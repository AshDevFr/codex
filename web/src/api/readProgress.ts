import type { components } from "@/types";
import { api } from "./client";

export type ReadProgressResponse =
  components["schemas"]["ReadProgressResponse"];
export type UpdateProgressRequest =
  components["schemas"]["UpdateProgressRequest"];

export const readProgressApi = {
  /**
   * Get reading progress for a book
   */
  get: async (bookId: string): Promise<ReadProgressResponse | null> => {
    try {
      const response = await api.get<ReadProgressResponse>(
        `/books/${bookId}/progress`,
      );
      return response.data;
    } catch (error) {
      // Return null if no progress exists (404)
      if (
        (error as { response?: { status?: number } })?.response?.status === 404
      ) {
        return null;
      }
      throw error;
    }
  },

  /**
   * Update reading progress for a book
   */
  update: async (
    bookId: string,
    request: UpdateProgressRequest,
  ): Promise<ReadProgressResponse> => {
    const response = await api.put<ReadProgressResponse>(
      `/books/${bookId}/progress`,
      request,
    );
    return response.data;
  },

  /**
   * Delete reading progress for a book
   */
  delete: async (bookId: string): Promise<void> => {
    await api.delete(`/books/${bookId}/progress`);
  },
};
