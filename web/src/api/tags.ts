import type { PaginatedResponse } from "@/types";
import type { components } from "@/types/api.generated";
import { api } from "./client";

export type Tag = components["schemas"]["TagDto"];
export type TagListResponse = components["schemas"]["TagListResponse"];

const MAX_PAGE_SIZE = 500;

export const tagsApi = {
  /**
   * Get all tags across all libraries.
   * Fetches all pages if results are paginated.
   */
  getAll: async (): Promise<Tag[]> => {
    const allTags: Tag[] = [];
    let page = 1;
    let hasMore = true;

    while (hasMore) {
      const response = await api.get<PaginatedResponse<Tag>>("/tags", {
        params: { page, pageSize: MAX_PAGE_SIZE },
      });
      const data = response.data.data ?? [];
      allTags.push(...data);

      hasMore = page < response.data.totalPages;
      page++;
    }

    return allTags;
  },

  /**
   * Get tags for a specific series
   */
  getForSeries: async (seriesId: string): Promise<Tag[]> => {
    const response = await api.get<TagListResponse>(`/series/${seriesId}/tags`);
    return response.data.tags;
  },

  /**
   * Set all tags for a series (replaces existing)
   */
  setForSeries: async (seriesId: string, tags: string[]): Promise<Tag[]> => {
    const response = await api.put<TagListResponse>(
      `/series/${seriesId}/tags`,
      { tags },
    );
    return response.data.tags;
  },

  /**
   * Add a single tag to a series
   */
  addToSeries: async (seriesId: string, name: string): Promise<Tag> => {
    const response = await api.post<Tag>(`/series/${seriesId}/tags`, { name });
    return response.data;
  },

  /**
   * Remove a tag from a series
   */
  removeFromSeries: async (seriesId: string, tagId: string): Promise<void> => {
    await api.delete(`/series/${seriesId}/tags/${tagId}`);
  },

  /**
   * Get tags for a specific book
   */
  getForBook: async (bookId: string): Promise<Tag[]> => {
    const response = await api.get<TagListResponse>(`/books/${bookId}/tags`);
    return response.data.tags;
  },

  /**
   * Set all tags for a book (replaces existing)
   */
  setForBook: async (bookId: string, tags: string[]): Promise<Tag[]> => {
    const response = await api.put<TagListResponse>(`/books/${bookId}/tags`, {
      tags,
    });
    return response.data.tags;
  },

  /**
   * Add a single tag to a book
   */
  addToBook: async (bookId: string, name: string): Promise<Tag> => {
    const response = await api.post<Tag>(`/books/${bookId}/tags`, { name });
    return response.data;
  },

  /**
   * Remove a tag from a book
   */
  removeFromBook: async (bookId: string, tagId: string): Promise<void> => {
    await api.delete(`/books/${bookId}/tags/${tagId}`);
  },

  /**
   * Delete a tag globally
   */
  delete: async (tagId: string): Promise<void> => {
    await api.delete(`/tags/${tagId}`);
  },

  /**
   * Clean up unused tags (admin only)
   */
  cleanup: async (): Promise<{ deleted_count: number }> => {
    const response = await api.post<{ deleted_count: number }>("/tags/cleanup");
    return response.data;
  },
};
