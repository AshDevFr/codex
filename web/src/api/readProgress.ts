import type { components } from "@/types";
import { api } from "./client";

export type ReadProgressResponse =
  components["schemas"]["ReadProgressResponse"];
export type UpdateProgressRequest =
  components["schemas"]["UpdateProgressRequest"];

export const readProgressApi = {
  /**
   * Get reading progress for a book
   * Returns null if no progress exists for the book
   */
  get: async (bookId: string): Promise<ReadProgressResponse | null> => {
    const response = await api.get<ReadProgressResponse | null>(
      `/books/${bookId}/progress`,
    );
    return response.data;
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
