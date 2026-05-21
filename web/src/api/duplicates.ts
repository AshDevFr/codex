import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type DuplicateGroup = components["schemas"]["DuplicateGroup"];
export type ListDuplicatesResponse =
  components["schemas"]["ListDuplicatesResponse"];
export type TriggerDuplicateScanResponse =
  components["schemas"]["TriggerDuplicateScanResponse"];
export type SeriesDuplicateGroup =
  components["schemas"]["SeriesDuplicateGroup"];
export type SeriesDuplicateMember =
  components["schemas"]["SeriesDuplicateMember"];
export type ListSeriesDuplicatesResponse =
  components["schemas"]["ListSeriesDuplicatesResponse"];

export type SeriesDuplicateMatchType = "external_id" | "title";

export const duplicatesApi = {
  /**
   * List all duplicate book groups
   */
  list: async (): Promise<DuplicateGroup[]> => {
    const response = await api.get<ListDuplicatesResponse>("/duplicates");
    return response.data.duplicates;
  },

  /**
   * Trigger a duplicate scan (covers both books and series).
   */
  scan: async (): Promise<TriggerDuplicateScanResponse> => {
    const response =
      await api.post<TriggerDuplicateScanResponse>("/duplicates/scan");
    return response.data;
  },

  /**
   * Delete a duplicate group
   */
  delete: async (duplicateId: string): Promise<void> => {
    await api.delete(`/duplicates/${duplicateId}`);
  },
};

export const seriesDuplicatesApi = {
  /**
   * List series duplicate groups. Pass `matchType` to filter by detection signal.
   */
  list: async (
    matchType?: SeriesDuplicateMatchType,
  ): Promise<ListSeriesDuplicatesResponse> => {
    const params = matchType ? { matchType } : undefined;
    const response = await api.get<ListSeriesDuplicatesResponse>(
      "/duplicates/series",
      { params },
    );
    return response.data;
  },

  /**
   * Delete a series duplicate group (tracking record only; series are kept).
   */
  delete: async (duplicateId: string): Promise<void> => {
    await api.delete(`/duplicates/series/${duplicateId}`);
  },
};
