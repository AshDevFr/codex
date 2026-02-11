import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type RecommendationDto = components["schemas"]["RecommendationDto"];
export type RecommendationsResponse =
  components["schemas"]["RecommendationsResponse"] & {
    /** Status of a running/pending background task ("pending" | "running"), if any */
    taskStatus?: "pending" | "running";
    /** ID of the running/pending background task, if any */
    taskId?: string;
  };
export type RecommendationsRefreshResponse =
  components["schemas"]["RecommendationsRefreshResponse"];
export type DismissRecommendationRequest =
  components["schemas"]["DismissRecommendationRequest"];
export type DismissRecommendationResponse =
  components["schemas"]["DismissRecommendationResponse"];

// =============================================================================
// API Client
// =============================================================================

export const recommendationsApi = {
  /**
   * Get personalized recommendations from the user's recommendation plugin
   */
  get: async (): Promise<RecommendationsResponse> => {
    const response = await api.get<RecommendationsResponse>(
      "/user/recommendations",
    );
    return response.data;
  },

  /**
   * Refresh recommendations (clears cache and regenerates)
   */
  refresh: async (): Promise<RecommendationsRefreshResponse> => {
    const response = await api.post<RecommendationsRefreshResponse>(
      "/user/recommendations/refresh",
    );
    return response.data;
  },

  /**
   * Dismiss a recommendation (user not interested)
   */
  dismiss: async (
    externalId: string,
    reason?: string,
  ): Promise<DismissRecommendationResponse> => {
    const response = await api.post<DismissRecommendationResponse>(
      `/user/recommendations/${encodeURIComponent(externalId)}/dismiss`,
      { reason } satisfies DismissRecommendationRequest,
    );
    return response.data;
  },
};
