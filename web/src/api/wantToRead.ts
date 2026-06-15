import type { components } from "@/types/api.generated";
import { api } from "./client";

export type WantToReadEntry = components["schemas"]["WantToReadEntryDto"];
export type WantToReadListResponse =
  components["schemas"]["WantToReadListResponse"];
export type WantToReadItemType = components["schemas"]["WantToReadItemType"];

/** Sort direction for the queue, by add time. */
export type WantToReadSort = "newest" | "oldest";

export const wantToReadApi = {
  /**
   * List the current user's want-to-read queue.
   * `newest` (default) returns most-recently-added first.
   */
  list: async (sort: WantToReadSort = "newest"): Promise<WantToReadEntry[]> => {
    const query = sort === "oldest" ? "?sort=added_at:asc" : "";
    const response = await api.get<WantToReadListResponse>(
      `/want-to-read${query}`,
    );
    return response.data.items;
  },

  /** Add a series to the queue. Idempotent. */
  addSeries: async (seriesId: string): Promise<WantToReadEntry> => {
    const response = await api.post<WantToReadEntry>("/want-to-read", {
      seriesId,
    });
    return response.data;
  },

  /** Add a book to the queue. Idempotent. */
  addBook: async (bookId: string): Promise<WantToReadEntry> => {
    const response = await api.post<WantToReadEntry>("/want-to-read", {
      bookId,
    });
    return response.data;
  },

  /** Remove a series from the queue. */
  removeSeries: async (seriesId: string): Promise<void> => {
    await api.delete(`/want-to-read/series/${seriesId}`);
  },

  /** Remove a book from the queue. */
  removeBook: async (bookId: string): Promise<void> => {
    await api.delete(`/want-to-read/books/${bookId}`);
  },
};
