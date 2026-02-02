import type { components } from "@/types/api.generated";
import type { PreferenceKey, TypedPreferences } from "@/types/preferences";
import { api } from "./client";

// Re-export generated types for convenience
export type UserPreferenceDto = components["schemas"]["UserPreferenceDto"];
export type UserPreferencesResponse =
  components["schemas"]["UserPreferencesResponse"];
export type SetPreferenceRequest =
  components["schemas"]["SetPreferenceRequest"];
export type BulkSetPreferencesRequest =
  components["schemas"]["BulkSetPreferencesRequest"];
export type SetPreferencesResponse =
  components["schemas"]["SetPreferencesResponse"];

export const userPreferencesApi = {
  /**
   * Get all user preferences
   */
  getAll: async (): Promise<UserPreferenceDto[]> => {
    const response =
      await api.get<UserPreferencesResponse>("/user/preferences");
    return response.data.preferences;
  },

  /**
   * Get a single preference by key
   */
  get: async <K extends PreferenceKey>(
    key: K,
  ): Promise<UserPreferenceDto | null> => {
    try {
      const response = await api.get<UserPreferenceDto>(
        `/user/preferences/${encodeURIComponent(key)}`,
      );
      return response.data;
    } catch (error) {
      // Return null if preference not found (404)
      if (
        error &&
        typeof error === "object" &&
        "response" in error &&
        (error as { response?: { status?: number } }).response?.status === 404
      ) {
        return null;
      }
      throw error;
    }
  },

  /**
   * Set a single preference
   */
  set: async <K extends PreferenceKey>(
    key: K,
    value: TypedPreferences[K],
  ): Promise<UserPreferenceDto> => {
    const response = await api.put<UserPreferenceDto>(
      `/user/preferences/${encodeURIComponent(key)}`,
      { value } as SetPreferenceRequest,
    );
    return response.data;
  },

  /**
   * Bulk set multiple preferences at once
   */
  bulkSet: async (
    preferences: Partial<TypedPreferences>,
  ): Promise<SetPreferencesResponse> => {
    const response = await api.put<SetPreferencesResponse>(
      "/user/preferences",
      { preferences } as BulkSetPreferencesRequest,
    );
    return response.data;
  },

  /**
   * Delete (reset) a preference to its default value
   */
  delete: async (key: PreferenceKey): Promise<void> => {
    await api.delete(`/user/preferences/${encodeURIComponent(key)}`);
  },
};
