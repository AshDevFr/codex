import type { components } from "@/types/api.generated";
import { api } from "./client";

export type ReleaseLedgerEntry = components["schemas"]["ReleaseLedgerEntryDto"];
export type ReleaseSource = components["schemas"]["ReleaseSourceDto"];
export type UpdateReleaseLedgerEntryRequest =
  components["schemas"]["UpdateReleaseLedgerEntryRequest"];
export type UpdateReleaseSourceRequest =
  components["schemas"]["UpdateReleaseSourceRequest"];
export type PaginatedReleases =
  components["schemas"]["PaginatedResponse_ReleaseLedgerEntryDto"];
export type ReleaseTrackingApplicability =
  components["schemas"]["ApplicabilityResponse"];
export type ResetReleaseSourceResponse =
  components["schemas"]["ResetReleaseSourceResponse"];
export type ReleaseFacets = components["schemas"]["ReleaseFacetsResponse"];
export type ReleaseSeriesFacet = components["schemas"]["ReleaseSeriesFacetDto"];
export type ReleaseLibraryFacet =
  components["schemas"]["ReleaseLibraryFacetDto"];
export type ReleaseLanguageFacet =
  components["schemas"]["ReleaseLanguageFacetDto"];
export type BulkReleaseAction = components["schemas"]["BulkReleaseAction"];
export type BulkReleaseActionRequest =
  components["schemas"]["BulkReleaseActionRequest"];
export type BulkReleaseActionResponse =
  components["schemas"]["BulkReleaseActionResponse"];
export type DeleteReleaseResponse =
  components["schemas"]["DeleteReleaseResponse"];

export interface ReleaseInboxParams {
  /** State filter. Use `"all"` for no state restriction; defaults to `"announced"` server-side. */
  state?: string;
  seriesId?: string;
  sourceId?: string;
  language?: string;
  libraryId?: string;
  page?: number;
  pageSize?: number;
}

export interface ReleaseFacetsParams {
  state?: string;
  seriesId?: string;
  sourceId?: string;
  language?: string;
  libraryId?: string;
}

export interface SeriesReleaseListParams {
  state?: string;
  page?: number;
  pageSize?: number;
}

function buildQuery(params: object) {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && value !== "") {
      search.append(key, String(value));
    }
  }
  const qs = search.toString();
  return qs ? `?${qs}` : "";
}

export const releasesApi = {
  listInbox: async (
    params: ReleaseInboxParams = {},
  ): Promise<PaginatedReleases> => {
    const response = await api.get<PaginatedReleases>(
      `/releases${buildQuery(params)}`,
    );
    return response.data;
  },

  listForSeries: async (
    seriesId: string,
    params: SeriesReleaseListParams = {},
  ): Promise<PaginatedReleases> => {
    const response = await api.get<PaginatedReleases>(
      `/series/${seriesId}/releases${buildQuery(params)}`,
    );
    return response.data;
  },

  patchEntry: async (
    releaseId: string,
    update: UpdateReleaseLedgerEntryRequest,
  ): Promise<ReleaseLedgerEntry> => {
    const response = await api.patch<ReleaseLedgerEntry>(
      `/releases/${releaseId}`,
      update,
    );
    return response.data;
  },

  dismiss: async (releaseId: string): Promise<ReleaseLedgerEntry> => {
    const response = await api.post<ReleaseLedgerEntry>(
      `/releases/${releaseId}/dismiss`,
    );
    return response.data;
  },

  markAcquired: async (releaseId: string): Promise<ReleaseLedgerEntry> => {
    const response = await api.post<ReleaseLedgerEntry>(
      `/releases/${releaseId}/mark-acquired`,
    );
    return response.data;
  },

  /**
   * Hard-delete a single ledger row. The affected source's `etag` is
   * cleared so the next poll re-fetches without `If-None-Match` and
   * re-announces the row.
   */
  delete: async (releaseId: string): Promise<DeleteReleaseResponse> => {
    const response = await api.delete<DeleteReleaseResponse>(
      `/releases/${releaseId}`,
    );
    return response.data;
  },

  /**
   * Apply an action (`dismiss`, `mark-acquired`, `delete`) to a batch
   * of ledger rows in a single request. Server caps at 500 ids; clients
   * should batch larger selections.
   */
  bulk: async (
    request: BulkReleaseActionRequest,
  ): Promise<BulkReleaseActionResponse> => {
    const response = await api.post<BulkReleaseActionResponse>(
      `/releases/bulk`,
      request,
    );
    return response.data;
  },

  /**
   * Distinct values present in the inbox under a given filter set.
   * Each facet excludes its own dimension so dropdowns never collapse
   * to the active selection. Used by the inbox UI to populate cascading
   * filter Selects without forcing UUID input.
   */
  facets: async (params: ReleaseFacetsParams = {}): Promise<ReleaseFacets> => {
    const response = await api.get<ReleaseFacets>(
      `/releases/facets${buildQuery(params)}`,
    );
    return response.data;
  },
};

export const releaseSourcesApi = {
  list: async (): Promise<ReleaseSource[]> => {
    const response = await api.get<{ sources: ReleaseSource[] }>(
      `/release-sources`,
    );
    return response.data.sources;
  },

  update: async (
    sourceId: string,
    update: UpdateReleaseSourceRequest,
  ): Promise<ReleaseSource> => {
    const response = await api.patch<ReleaseSource>(
      `/release-sources/${sourceId}`,
      update,
    );
    return response.data;
  },

  pollNow: async (
    sourceId: string,
  ): Promise<{ status: string; message: string }> => {
    const response = await api.post<{ status: string; message: string }>(
      `/release-sources/${sourceId}/poll-now`,
    );
    return response.data;
  },

  /**
   * Drop every ledger row for this source and clear its transient poll
   * state (etag, last_polled_at, last_error, last_summary). User-managed
   * fields (enabled, cronSchedule, displayName, config) are preserved.
   *
   * Used as a "force re-emit" lever for testing: after a reset, the next
   * poll fetches the upstream feed without `If-None-Match` (no 304
   * short-circuit) and re-records every release as `announced`.
   */
  reset: async (sourceId: string): Promise<ResetReleaseSourceResponse> => {
    const response = await api.post<ResetReleaseSourceResponse>(
      `/release-sources/${sourceId}/reset`,
    );
    return response.data;
  },

  /**
   * Whether release tracking is available for a given library scope.
   *
   * Returns `applicable: true` when at least one enabled release-source
   * plugin applies to `libraryId` (or, with `libraryId` omitted, to any
   * library). The frontend uses this to hide the per-series Tracking panel
   * and Releases tab on libraries that aren't covered, and to gate the
   * bulk-track menu entry.
   */
  applicability: async (
    libraryId?: string,
  ): Promise<ReleaseTrackingApplicability> => {
    const params = new URLSearchParams();
    if (libraryId) {
      params.set("libraryId", libraryId);
    }
    const qs = params.toString();
    const response = await api.get<ReleaseTrackingApplicability>(
      `/release-sources/applicability${qs ? `?${qs}` : ""}`,
    );
    return response.data;
  },
};
