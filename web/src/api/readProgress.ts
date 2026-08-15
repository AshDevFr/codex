import {
  enqueueOfflineWrite,
  isOfflineError,
  OfflineQueuedError,
} from "@/lib/offline/outbox";
import { getDeviceId } from "@/lib/reading/deviceIdentity";
import type { components } from "@/types";
import { api } from "./client";
import { noContentAsNull } from "./noContent";

export type ReadProgressResponse =
  components["schemas"]["ReadProgressResponse"];
export type UpdateProgressRequest =
  components["schemas"]["UpdateProgressRequest"];
export type ReadHistoryResponse = components["schemas"]["ReadHistoryResponse"];
export type ReadCompletionDto = components["schemas"]["ReadCompletionDto"];

const API_BASE = "/api/v1";

/**
 * Build the auth + content-type headers the outbox needs to replay this
 * request later. Captures the JWT at enqueue time; if the user logs out
 * before the drain fires the replay will get a 401 (the drain marks the
 * record as failed-retry; the user re-authenticates and tries again).
 */
function captureWriteHeaders(): Record<string, string> {
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    // Declares which device these position writes belong to.
    //
    // The reader posts a measured session only when a sitting ends, so until
    // then these writes are the only thing keeping the stored position live.
    // Tagging them with the same device lets the measured session recognise
    // and absorb them when it arrives, instead of leaving one sitting looking
    // like a page turn's worth of separate sessions on a phantom device.
    "X-Codex-Device-Id": getDeviceId(),
  };
  const token =
    typeof localStorage !== "undefined"
      ? localStorage.getItem("jwt_token")
      : null;
  if (token) headers.Authorization = `Bearer ${token}`;
  return headers;
}

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
   * Get reading progress for a book.
   * Resolves to null when the book has not been started (the server answers
   * 204 No Content).
   */
  get: async (bookId: string): Promise<ReadProgressResponse | null> => {
    const response = await api.get<ReadProgressResponse | null>(
      `/books/${bookId}/progress`,
    );
    return noContentAsNull(response);
  },

  /**
   * Update reading progress for a book.
   *
   * On network failure (offline / server unreachable) the request is
   * serialised into the offline outbox and an {@link OfflineQueuedError}
   * is thrown. Callers should treat that error as "saved locally, will
   * sync when online" rather than a real failure.
   */
  update: async (
    bookId: string,
    request: UpdateProgressRequest,
  ): Promise<ReadProgressResponse> => {
    try {
      const response = await api.put<ReadProgressResponse>(
        `/books/${bookId}/progress`,
        request,
      );
      return response.data;
    } catch (err) {
      if (!isOfflineError(err)) throw err;
      const descriptor = {
        url: `${API_BASE}/books/${bookId}/progress`,
        method: "PUT",
        headers: captureWriteHeaders(),
        body: request,
      };
      await enqueueOfflineWrite(descriptor);
      throw new OfflineQueuedError(descriptor);
    }
  },

  /**
   * Delete reading progress for a book. Same offline semantics as `update`.
   */
  delete: async (bookId: string): Promise<void> => {
    try {
      await api.delete(`/books/${bookId}/progress`);
    } catch (err) {
      if (!isOfflineError(err)) throw err;
      const descriptor = {
        url: `${API_BASE}/books/${bookId}/progress`,
        method: "DELETE",
        headers: captureWriteHeaders(),
      };
      await enqueueOfflineWrite(descriptor);
      throw new OfflineQueuedError(descriptor);
    }
  },

  /**
   * A book's completion history for the current user.
   *
   * Independent of reading progress: this survives marking the book unread.
   */
  getBookHistory: async (bookId: string): Promise<ReadHistoryResponse> => {
    const response = await api.get<ReadHistoryResponse>(
      `/books/${bookId}/read-history`,
    );
    return response.data;
  },

  /** Clear a book's completion history. Leaves reading progress alone. */
  clearBookHistory: async (bookId: string): Promise<void> => {
    await api.delete(`/books/${bookId}/read-history`);
  },

  /**
   * A series' completion history for the current user.
   *
   * `readCount` is the minimum across the series' books, so it only advances
   * once every volume has been read again.
   */
  getSeriesHistory: async (seriesId: string): Promise<ReadHistoryResponse> => {
    const response = await api.get<ReadHistoryResponse>(
      `/series/${seriesId}/read-history`,
    );
    return response.data;
  },

  /** Clear every book's history in a series. Leaves reading progress alone. */
  clearSeriesHistory: async (seriesId: string): Promise<void> => {
    await api.delete(`/series/${seriesId}/read-history`);
  },

  /** Clear the current user's entire completion history. */
  clearMyHistory: async (): Promise<void> => {
    await api.delete("/user/read-history");
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
   * Update R2Progression for a book (Readium standard). Same offline
   * semantics as `update`.
   */
  updateProgression: async (
    bookId: string,
    progression: R2Progression,
  ): Promise<void> => {
    try {
      await api.put(`/books/${bookId}/progression`, progression);
    } catch (err) {
      if (!isOfflineError(err)) throw err;
      const descriptor = {
        url: `${API_BASE}/books/${bookId}/progression`,
        method: "PUT",
        headers: captureWriteHeaders(),
        body: progression,
      };
      await enqueueOfflineWrite(descriptor);
      throw new OfflineQueuedError(descriptor);
    }
  },
};
