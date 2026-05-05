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

export interface ReleaseInboxParams {
  state?: string;
  seriesId?: string;
  sourceId?: string;
  language?: string;
  page?: number;
  pageSize?: number;
}

export interface SeriesReleaseListParams {
  state?: string;
  page?: number;
  pageSize?: number;
}

function buildQuery(params: Record<string, string | number | undefined>) {
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
