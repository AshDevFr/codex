import type {
	Book,
	FullBook,
	FullSeries,
	PaginatedResponse,
	Series,
	SeriesCondition,
	SeriesListRequest,
} from "@/types";
import { api } from "./client";

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
};

/** Alphabetical group with count */
export interface AlphabeticalGroup {
	/** The first character (lowercase letter, digit, or special character) */
	group: string;
	/** Number of series starting with this character */
	count: number;
}
