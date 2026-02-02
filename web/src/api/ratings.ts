import type { components } from "@/types/api.generated";
import { api } from "./client";

export type UserSeriesRating = components["schemas"]["UserSeriesRatingDto"];
export type UserRatingsListResponse =
  components["schemas"]["UserRatingsListResponse"];
export type SetUserRatingRequest =
  components["schemas"]["SetUserRatingRequest"];
export type SeriesAverageRatingResponse =
  components["schemas"]["SeriesAverageRatingResponse"];

export const ratingsApi = {
  /**
   * Get the current user's rating for a series
   * Returns null if no rating exists
   */
  getUserRating: async (seriesId: string): Promise<UserSeriesRating | null> => {
    const response = await api.get<UserSeriesRating | null>(
      `/series/${seriesId}/rating`,
    );
    return response.data;
  },

  /**
   * Set or update the current user's rating for a series
   * @param seriesId - Series ID
   * @param rating - Rating value (1-100, displayed as 0.1-10.0 in UI)
   * @param notes - Optional notes/review
   */
  setUserRating: async (
    seriesId: string,
    rating: number,
    notes?: string,
  ): Promise<UserSeriesRating> => {
    const response = await api.put<UserSeriesRating>(
      `/series/${seriesId}/rating`,
      { rating, notes } satisfies SetUserRatingRequest,
    );
    return response.data;
  },

  /**
   * Delete the current user's rating for a series
   */
  deleteUserRating: async (seriesId: string): Promise<void> => {
    await api.delete(`/series/${seriesId}/rating`);
  },

  /**
   * Get all ratings for the current user
   */
  getAllUserRatings: async (): Promise<UserSeriesRating[]> => {
    const response = await api.get<UserRatingsListResponse>("/user/ratings");
    return response.data.ratings;
  },

  /**
   * Get the average rating for a series from all users
   * @param seriesId - Series ID
   * @returns Average rating (0-100 scale) and count, or null average if no ratings
   */
  getSeriesAverageRating: async (
    seriesId: string,
  ): Promise<SeriesAverageRatingResponse> => {
    const response = await api.get<SeriesAverageRatingResponse>(
      `/series/${seriesId}/ratings/average`,
    );
    return response.data;
  },
};

/**
 * Convert display rating (1.0-10.0) to storage rating (10-100)
 */
export function displayToStorageRating(displayRating: number): number {
  return Math.round(displayRating * 10);
}

/**
 * Convert storage rating (10-100) to display rating (1.0-10.0)
 */
export function storageToDisplayRating(storageRating: number): number {
  return storageRating / 10;
}
