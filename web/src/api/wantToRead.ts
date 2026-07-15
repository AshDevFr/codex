import type { components } from "@/types/api.generated";
import { api } from "./client";

export type WantToReadEntry = components["schemas"]["WantToReadEntryDto"];
export type WantToReadListResponse =
  components["schemas"]["WantToReadListResponse"];
export type WantToReadItemType = components["schemas"]["WantToReadItemType"];

/** Queue sort: by add time, or the user's manual (`custom`) order. */
export type WantToReadSort = "newest" | "oldest" | "custom";

export type BulkAddWantToReadResponse =
  components["schemas"]["BulkAddWantToReadResponse"];

export const wantToReadApi = {
  /**
   * List the current user's want-to-read queue.
   * `newest` (default) returns most-recently-added first.
   */
  list: async (sort: WantToReadSort = "newest"): Promise<WantToReadEntry[]> => {
    const query = sort === "newest" ? "" : `?sort=${sort}`;
    const response = await api.get<WantToReadListResponse>(
      `/want-to-read${query}`,
    );
    return response.data.items;
  },

  /** Set the manual (`custom`) order of the queue by entry IDs. */
  reorder: async (entryIds: string[]): Promise<void> => {
    await api.put("/want-to-read/order", { entryIds });
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

  /**
   * Add many series to the queue in one call. Idempotent: items already queued
   * are reported via `alreadyPresent`; unknown ids are skipped.
   */
  bulkAddSeries: async (
    seriesIds: string[],
  ): Promise<BulkAddWantToReadResponse> => {
    const response = await api.post<BulkAddWantToReadResponse>(
      "/want-to-read/bulk",
      { seriesIds },
    );
    return response.data;
  },

  /** Add many books to the queue in one call. Idempotent. */
  bulkAddBooks: async (
    bookIds: string[],
  ): Promise<BulkAddWantToReadResponse> => {
    const response = await api.post<BulkAddWantToReadResponse>(
      "/want-to-read/bulk",
      { bookIds },
    );
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
