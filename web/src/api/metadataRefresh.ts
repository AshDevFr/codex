import type { components } from "@/types/api.generated";
import { api } from "./client";

export type MetadataRefreshConfig =
  components["schemas"]["MetadataRefreshConfigDto"];
export type MetadataRefreshConfigPatch =
  components["schemas"]["MetadataRefreshConfigPatchDto"];
export type ProviderOverride = components["schemas"]["ProviderOverrideDto"];
export type FieldGroup = components["schemas"]["FieldGroupDto"];
export type RunNowResponse = components["schemas"]["RunNowResponse"];
export type DryRunRequest = components["schemas"]["DryRunRequest"];
export type DryRunResponse = components["schemas"]["DryRunResponse"];
export type DryRunSeriesDelta = components["schemas"]["DryRunSeriesDelta"];
export type DryRunSkippedField = components["schemas"]["DryRunSkippedFieldDto"];
export type FieldChange = components["schemas"]["FieldChangeDto"];

/**
 * Hand-written PATCH input. The generated PATCH DTO uses `T | null` everywhere
 * with `default: null`, but in practice clients should omit fields they don't
 * want to change. This narrower type lets callers just pass the fields they
 * care about.
 */
export type MetadataRefreshConfigPatchInput = Partial<{
  enabled: boolean;
  cronSchedule: string;
  timezone: string | null;
  fieldGroups: string[];
  extraFields: string[];
  providers: string[];
  existingSourceIdsOnly: boolean;
  skipRecentlySyncedWithinS: number;
  maxConcurrency: number;
  perProviderOverrides: Record<string, ProviderOverride> | null;
}>;

export const metadataRefreshApi = {
  /** Read the current saved config (server returns defaults if none stored). */
  get: async (libraryId: string): Promise<MetadataRefreshConfig> => {
    const response = await api.get<MetadataRefreshConfig>(
      `/libraries/${libraryId}/metadata-refresh`,
    );
    return response.data;
  },

  /** Partial update; omitted fields are left alone server-side. */
  update: async (
    libraryId: string,
    patch: MetadataRefreshConfigPatchInput,
  ): Promise<MetadataRefreshConfig> => {
    const response = await api.patch<MetadataRefreshConfig>(
      `/libraries/${libraryId}/metadata-refresh`,
      patch,
    );
    return response.data;
  },

  /** Enqueue an immediate refresh task. */
  runNow: async (libraryId: string): Promise<RunNowResponse> => {
    const response = await api.post<RunNowResponse>(
      `/libraries/${libraryId}/metadata-refresh/run-now`,
    );
    return response.data;
  },

  /**
   * Preview what a refresh would change. When `configOverride` is set, the
   * preview uses that unsaved config instead of the persisted one.
   */
  dryRun: async (
    libraryId: string,
    request: DryRunRequest = { configOverride: null, sampleSize: null },
  ): Promise<DryRunResponse> => {
    const response = await api.post<DryRunResponse>(
      `/libraries/${libraryId}/metadata-refresh/dry-run`,
      request,
    );
    return response.data;
  },

  /** Catalog of field groups exposed by the server. */
  listFieldGroups: async (): Promise<FieldGroup[]> => {
    const response = await api.get<FieldGroup[]>(
      "/metadata-refresh/field-groups",
    );
    return response.data;
  },
};
