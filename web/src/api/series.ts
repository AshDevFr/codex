import type {
  Book,
  FullBook,
  FullSeries,
  PaginatedResponse,
  Series,
  SeriesCondition,
  SeriesListRequest,
} from "@/types";
import type { components } from "@/types/api.generated";
import { api } from "./client";

export type BulkTrackForReleasesResponse =
  components["schemas"]["BulkTrackForReleasesResponse"];

export interface SeriesFilters {
  page?: number;
  pageSize?: number;
  sort?: string;
  genres?: string; // Comma-separated genre names (AND logic)
  tags?: string; // Comma-separated tag names (AND logic)
  status?: string;
  publisher?: string;
  year?: number;
  /** When true, returns FullSeriesResponse with complete metadata, genres, tags, etc. */
  full?: boolean;
}

export const seriesApi = {
  // Get series by library ID with filters
  getByLibrary: async <T extends boolean = false>(
    libraryId: string,
    filters?: SeriesFilters & { full?: T },
  ): Promise<PaginatedResponse<T extends true ? FullSeries : Series>> => {
    const params = new URLSearchParams();

    // Add library filter if not "all"
    if (libraryId !== "all") {
      params.set("libraryId", libraryId);
    }

    if (filters?.page) params.set("page", filters.page.toString());
    if (filters?.pageSize) params.set("pageSize", filters.pageSize.toString());
    if (filters?.sort) params.set("sort", filters.sort);
    if (filters?.genres) params.set("genres", filters.genres);
    if (filters?.tags) params.set("tags", filters.tags);
    if (filters?.status) params.set("status", filters.status);
    if (filters?.publisher) params.set("publisher", filters.publisher);
    if (filters?.year) params.set("year", filters.year.toString());
    if (filters?.full) params.set("full", "true");

    const queryString = params.toString();
    const url = `/series${queryString ? `?${queryString}` : ""}`;

    const response =
      await api.get<PaginatedResponse<T extends true ? FullSeries : Series>>(
        url,
      );
    return response.data;
  },

  // Get a single series by ID
  getById: async <T extends boolean = false>(
    id: string,
    options?: { full?: T },
  ): Promise<T extends true ? FullSeries : Series> => {
    const params = new URLSearchParams();
    if (options?.full) params.set("full", "true");
    const queryString = params.toString();
    const url = `/series/${id}${queryString ? `?${queryString}` : ""}`;

    const response = await api.get<T extends true ? FullSeries : Series>(url);
    return response.data;
  },

  // Get series with in-progress books
  getInProgress: async <T extends boolean = false>(
    libraryId: string,
    options?: { full?: T },
  ): Promise<(T extends true ? FullSeries : Series)[]> => {
    const params = new URLSearchParams();
    if (libraryId !== "all") {
      params.set("libraryId", libraryId);
    }
    if (options?.full) params.set("full", "true");
    const queryString = params.toString();
    const url = `/series/in-progress${queryString ? `?${queryString}` : ""}`;

    const response =
      await api.get<(T extends true ? FullSeries : Series)[]>(url);
    return response.data;
  },

  // Trigger series analysis (force - all books)
  analyze: async (seriesId: string): Promise<{ message: string }> => {
    const response = await api.post<{ message: string }>(
      `/series/${seriesId}/analyze`,
    );
    return response.data;
  },

  // Trigger series analysis for unanalyzed books only
  analyzeUnanalyzed: async (seriesId: string): Promise<{ message: string }> => {
    const response = await api.post<{ message: string }>(
      `/series/${seriesId}/analyze-unanalyzed`,
    );
    return response.data;
  },

  // Generate missing thumbnails for books in series (queues a background task)
  generateMissingBookThumbnails: async (
    seriesId: string,
  ): Promise<{ task_id: string }> => {
    const response = await api.post<{ task_id: string }>(
      "/books/thumbnails/generate",
      { series_id: seriesId, force: false },
    );
    return response.data;
  },

  // Regenerate all thumbnails for books in series (queues a background task)
  regenerateBookThumbnails: async (
    seriesId: string,
  ): Promise<{ task_id: string }> => {
    const response = await api.post<{ task_id: string }>(
      "/books/thumbnails/generate",
      { series_id: seriesId, force: true },
    );
    return response.data;
  },

  // Generate series cover thumbnail if missing (from first book's cover)
  generateSeriesThumbnailIfMissing: async (
    seriesId: string,
  ): Promise<{ task_id: string }> => {
    const response = await api.post<{ task_id: string }>(
      `/series/${seriesId}/thumbnail/generate`,
      { force: false },
    );
    return response.data;
  },

  // Regenerate the series cover thumbnail (from first book's cover)
  regenerateSeriesThumbnail: async (
    seriesId: string,
  ): Promise<{ task_id: string }> => {
    const response = await api.post<{ task_id: string }>(
      `/series/${seriesId}/thumbnail/generate`,
      { force: true },
    );
    return response.data;
  },

  // Mark all books in a series as read
  markAsRead: async (
    seriesId: string,
  ): Promise<{ count: number; message: string }> => {
    const response = await api.post<{ count: number; message: string }>(
      `/series/${seriesId}/read`,
    );
    return response.data;
  },

  // Mark all books in a series as unread
  markAsUnread: async (
    seriesId: string,
  ): Promise<{ count: number; message: string }> => {
    const response = await api.post<{ count: number; message: string }>(
      `/series/${seriesId}/unread`,
    );
    return response.data;
  },

  // Get recently added series
  getRecentlyAdded: async <T extends boolean = false>(
    libraryId: string,
    options?: { limit?: number; full?: T },
  ): Promise<(T extends true ? FullSeries : Series)[]> => {
    const params = new URLSearchParams();
    if (libraryId !== "all") {
      params.set("libraryId", libraryId);
    }
    params.set("limit", (options?.limit ?? 50).toString());
    if (options?.full) params.set("full", "true");
    const queryString = params.toString();
    const url = `/series/recently-added?${queryString}`;

    const response =
      await api.get<(T extends true ? FullSeries : Series)[]>(url);
    return response.data;
  },

  // Get recently updated series
  getRecentlyUpdated: async <T extends boolean = false>(
    libraryId: string,
    options?: { limit?: number; full?: T },
  ): Promise<(T extends true ? FullSeries : Series)[]> => {
    const params = new URLSearchParams();
    if (libraryId !== "all") {
      params.set("libraryId", libraryId);
    }
    params.set("limit", (options?.limit ?? 50).toString());
    if (options?.full) params.set("full", "true");
    const queryString = params.toString();
    const url = `/series/recently-updated?${queryString}`;

    const response =
      await api.get<(T extends true ? FullSeries : Series)[]>(url);
    return response.data;
  },

  // Get books in a series
  getBooks: async <T extends boolean = false>(
    seriesId: string,
    options?: { includeDeleted?: boolean; full?: T },
  ): Promise<(T extends true ? FullBook : Book)[]> => {
    const params = new URLSearchParams();
    if (options?.includeDeleted) {
      params.set("includeDeleted", "true");
    }
    if (options?.full) params.set("full", "true");
    const queryString = params.toString();
    const url = `/series/${seriesId}/books${queryString ? `?${queryString}` : ""}`;

    const response = await api.get<(T extends true ? FullBook : Book)[]>(url);
    return response.data;
  },

  /**
   * Search/filter series with advanced condition-based filtering.
   *
   * Uses POST /series/list endpoint which supports:
   * - Nested AllOf/AnyOf conditions
   * - Include/exclude filtering for genres, tags, status, etc.
   * - Full-text search (optional)
   * - Pagination and sorting (via query params)
   *
   * @param libraryId - Library to filter by, or "all" for all libraries
   * @param request - The search request with condition, pagination, and sort options
   */
  search: async <T extends boolean = false>(
    libraryId: string,
    request: {
      condition?: SeriesCondition;
      search?: string;
      page?: number;
      pageSize?: number;
      sort?: string;
      full?: T;
    },
  ): Promise<PaginatedResponse<T extends true ? FullSeries : Series>> => {
    // Build the full condition including library filter
    let finalCondition: SeriesCondition | undefined = request.condition;

    // Add library filter if not "all"
    if (libraryId !== "all") {
      const libraryCondition: SeriesCondition = {
        libraryId: { operator: "is", value: libraryId },
      };

      if (finalCondition) {
        // Combine with existing condition using allOf
        finalCondition = {
          allOf: [libraryCondition, finalCondition],
        };
      } else {
        finalCondition = libraryCondition;
      }
    }

    // Build query params for pagination (moved from body)
    const params = new URLSearchParams();
    if (request.page !== undefined) params.set("page", String(request.page));
    if (request.pageSize !== undefined)
      params.set("pageSize", String(request.pageSize));
    if (request.sort) params.set("sort", request.sort);
    if (request.full) params.set("full", "true");

    // Body only contains filter condition and search
    const body: SeriesListRequest = {
      condition: finalCondition,
      fullTextSearch: request.search,
    };

    const queryString = params.toString();
    const url = queryString ? `/series/list?${queryString}` : "/series/list";

    const response = await api.post<
      PaginatedResponse<T extends true ? FullSeries : Series>
    >(url, body);
    return response.data;
  },

  /**
   * Update series core fields (title)
   * @param seriesId - Series ID
   * @param data - Fields to update (title)
   */
  patch: async (
    seriesId: string,
    data: { title?: string },
  ): Promise<{ id: string; title: string; updatedAt: string }> => {
    const response = await api.patch<{
      id: string;
      title: string;
      updatedAt: string;
    }>(`/series/${seriesId}`, data);
    return response.data;
  },

  /**
   * Get alphabetical groups with counts for series navigation.
   *
   * Returns a list of first characters (lowercase) with the count of series
   * starting with that character. Useful for A-Z navigation filters.
   *
   * @param libraryId - Library to filter by, or "all" for all libraries
   * @param condition - Optional additional filter condition
   */
  getAlphabeticalGroups: async (
    libraryId: string,
    condition?: SeriesCondition,
  ): Promise<AlphabeticalGroup[]> => {
    // Build the full condition including library filter
    let finalCondition: SeriesCondition | undefined = condition;

    // Add library filter if not "all"
    if (libraryId !== "all") {
      const libraryCondition: SeriesCondition = {
        libraryId: { operator: "is", value: libraryId },
      };

      if (finalCondition) {
        finalCondition = {
          allOf: [libraryCondition, finalCondition],
        };
      } else {
        finalCondition = libraryCondition;
      }
    }

    const body: SeriesListRequest = {
      condition: finalCondition,
    };

    const response = await api.post<AlphabeticalGroup[]>(
      "/series/list/alphabetical-groups",
      body,
    );
    return response.data;
  },

  // ==================== Bulk Operations API ====================

  /**
   * Mark all books in multiple series as read in bulk
   * @param seriesIds - Array of series IDs to mark as read
   */
  bulkMarkAsRead: async (
    seriesIds: string[],
  ): Promise<{ count: number; message: string }> => {
    const response = await api.post<{ count: number; message: string }>(
      "/series/bulk/read",
      { seriesIds },
    );
    return response.data;
  },

  /**
   * Mark all books in multiple series as unread in bulk
   * @param seriesIds - Array of series IDs to mark as unread
   */
  bulkMarkAsUnread: async (
    seriesIds: string[],
  ): Promise<{ count: number; message: string }> => {
    const response = await api.post<{ count: number; message: string }>(
      "/series/bulk/unread",
      { seriesIds },
    );
    return response.data;
  },

  /**
   * Bulk-enable release tracking. Flips `tracked: true` on each series and
   * runs the seed pass (auto-derives aliases, latest_known_*, track_*).
   * Series already tracked are reported as `outcome: skipped`.
   */
  bulkTrackForReleases: async (
    seriesIds: string[],
  ): Promise<BulkTrackForReleasesResponse> => {
    const response = await api.post<BulkTrackForReleasesResponse>(
      "/series/bulk/track-for-releases",
      { seriesIds },
    );
    return response.data;
  },

  /**
   * Bulk-disable release tracking. Flips `tracked: false` without deleting
   * aliases or other tracking config — re-tracking later still re-seeds
   * the auto-derived fields.
   */
  bulkUntrackForReleases: async (
    seriesIds: string[],
  ): Promise<BulkTrackForReleasesResponse> => {
    const response = await api.post<BulkTrackForReleasesResponse>(
      "/series/bulk/untrack-for-releases",
      { seriesIds },
    );
    return response.data;
  },

  /**
   * Queue analysis for all books in multiple series in bulk
   * @param seriesIds - Array of series IDs to analyze
   * @param force - Whether to force re-analysis of already analyzed books (default: true)
   */
  bulkAnalyze: async (
    seriesIds: string[],
    force = true,
  ): Promise<{ tasksEnqueued: number; message: string }> => {
    const response = await api.post<{ tasksEnqueued: number; message: string }>(
      "/series/bulk/analyze",
      { seriesIds, force },
    );
    return response.data;
  },

  /**
   * Reprocess series title using library preprocessing rules (queues a background task)
   * @param seriesId - Series ID to reprocess title for
   */
  reprocessTitle: async (seriesId: string): Promise<{ taskId: string }> => {
    const response = await api.post<{ taskId: string }>(
      `/series/${seriesId}/title/reprocess`,
      {},
    );
    return response.data;
  },

  /**
   * Queue thumbnail generation for series covers in bulk
   * @param seriesIds - Array of series IDs to generate thumbnails for
   * @param force - Whether to regenerate thumbnails even if they exist (default: false)
   */
  bulkGenerateSeriesThumbnails: async (
    seriesIds: string[],
    force = false,
  ): Promise<{ taskId: string; message: string }> => {
    const response = await api.post<{ taskId: string; message: string }>(
      "/series/bulk/thumbnails/generate",
      { seriesIds, force },
    );
    return response.data;
  },

  /**
   * Queue thumbnail generation for all books in multiple series
   * @param seriesIds - Array of series IDs whose books should have thumbnails generated
   * @param force - Whether to regenerate thumbnails even if they exist (default: false)
   */
  bulkGenerateBookThumbnails: async (
    seriesIds: string[],
    force = false,
  ): Promise<{ taskId: string; message: string }> => {
    const response = await api.post<{ taskId: string; message: string }>(
      "/series/bulk/thumbnails/books/generate",
      { seriesIds, force },
    );
    return response.data;
  },

  /**
   * Queue title reprocessing for multiple series in bulk
   * @param seriesIds - Array of series IDs to reprocess titles for
   */
  bulkReprocessTitles: async (
    seriesIds: string[],
  ): Promise<{ taskId: string; message: string }> => {
    const response = await api.post<{ taskId: string; message: string }>(
      "/series/bulk/titles/reprocess",
      { seriesIds },
    );
    return response.data;
  },

  /**
   * Reset metadata for multiple series back to filesystem-derived defaults in bulk.
   * Clears genres, tags, alternate titles, external IDs/ratings/links,
   * covers, metadata sources, and all lock states.
   * Preserves series records, book data, user ratings, and read progress.
   * @param seriesIds - Array of series IDs to reset metadata for
   */
  bulkResetMetadata: async (
    seriesIds: string[],
  ): Promise<{ count: number; message: string }> => {
    const response = await api.post<{ count: number; message: string }>(
      "/series/bulk/metadata/reset",
      { seriesIds },
    );
    return response.data;
  },

  /**
   * Renumber all books in a series based on the library's number strategy.
   * Enqueues a task and returns a task ID for tracking via SSE.
   * @param seriesId - Series ID to renumber books for
   */
  renumber: async (seriesId: string): Promise<{ taskId: string }> => {
    const response = await api.post<{ taskId: string }>(
      `/series/${seriesId}/renumber`,
    );
    return response.data;
  },

  /**
   * Renumber books in multiple series in bulk.
   * Enqueues a fan-out task and returns a task ID for tracking via SSE.
   * @param seriesIds - Array of series IDs to renumber books for
   */
  bulkRenumber: async (
    seriesIds: string[],
  ): Promise<{ taskId: string; message: string }> => {
    const response = await api.post<{ taskId: string; message: string }>(
      "/series/bulk/renumber",
      { seriesIds },
    );
    return response.data;
  },
};

/** Alphabetical group with count */
export interface AlphabeticalGroup {
  /** The first character (lowercase letter, digit, or special character) */
  group: string;
  /** Number of series starting with this character */
  count: number;
}
