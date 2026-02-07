import { api } from "./client";

// =============================================================================
// Types
// =============================================================================

// Types defined manually until OpenAPI types are regenerated.
// These match the backend DTOs in src/api/routes/v1/dto/recommendations.rs

/** A single recommendation */
export interface RecommendationDto {
  externalId: string;
  externalUrl?: string;
  title: string;
  coverUrl?: string;
  summary?: string;
  genres?: string[];
  score: number;
  reason: string;
  basedOn?: string[];
  codexSeriesId?: string;
  inLibrary: boolean;
}

/** Recommendations list response */
export interface RecommendationsResponse {
  recommendations: RecommendationDto[];
  pluginId: string;
  pluginName: string;
  generatedAt?: string;
  cached: boolean;
}

/** Refresh recommendations response */
export interface RecommendationsRefreshResponse {
  taskId: string;
  message: string;
}

/** Dismiss recommendation request */
export interface DismissRecommendationRequest {
  reason?: string;
}

/** Dismiss recommendation response */
export interface DismissRecommendationResponse {
  dismissed: boolean;
}

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
