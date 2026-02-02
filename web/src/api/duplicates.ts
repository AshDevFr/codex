import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type DuplicateGroup = components["schemas"]["DuplicateGroup"];
export type ListDuplicatesResponse =
  components["schemas"]["ListDuplicatesResponse"];
export type TriggerDuplicateScanResponse =
  components["schemas"]["TriggerDuplicateScanResponse"];

export const duplicatesApi = {
  /**
   * List all duplicate groups
   */
  list: async (): Promise<DuplicateGroup[]> => {
    const response = await api.get<ListDuplicatesResponse>("/duplicates");
    return response.data.duplicates;
  },

  /**
   * Trigger a duplicate scan
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
