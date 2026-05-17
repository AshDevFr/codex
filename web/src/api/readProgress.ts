import {
  enqueueOfflineWrite,
  isOfflineError,
  OfflineQueuedError,
} from "@/lib/offline/outbox";
import type { components } from "@/types";
import { api } from "./client";

export type ReadProgressResponse =
  components["schemas"]["ReadProgressResponse"];
export type UpdateProgressRequest =
  components["schemas"]["UpdateProgressRequest"];

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
