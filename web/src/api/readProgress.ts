import type { components } from "@/types";
import { api } from "./client";

export type ReadProgressResponse =
  components["schemas"]["ReadProgressResponse"];
export type UpdateProgressRequest =
  components["schemas"]["UpdateProgressRequest"];

/** Readium R2Progression format for EPUB position sync */
export interface R2Progression {
  device: { id: string; name: string };
  locator: {
    href: string;
    locations: {
      position?: number;
      progression?: number;
      totalProgression: number;
      /** Codex extension: epub.js CFI for precise position restoration */
      cfi?: string;
    };
    type: string;
  };
  modified: string;
}

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

  /**
   * Get R2Progression for a book (Readium standard)
   * Returns null if no progression exists (204 response)
   */
  getProgression: async (bookId: string): Promise<R2Progression | null> => {
    const response = await api.get<R2Progression>(
      `/books/${bookId}/progression`,
      {
        validateStatus: (status) => status === 200 || status === 204,
      },
    );
    return response.status === 204 ? null : response.data;
  },

  /**
   * Update R2Progression for a book (Readium standard)
   */
  updateProgression: async (
    bookId: string,
    progression: R2Progression,
  ): Promise<void> => {
    await api.put(`/books/${bookId}/progression`, progression);
  },
};
