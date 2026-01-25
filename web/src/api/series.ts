import type {
	Book,
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
}

export const seriesApi = {
	// Get series by library ID with filters
	getByLibrary: async (
		libraryId: string,
		filters?: SeriesFilters,
	): Promise<PaginatedResponse<Series>> => {
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

		const queryString = params.toString();
		const url = `/series${queryString ? `?${queryString}` : ""}`;

		const response = await api.get<PaginatedResponse<Series>>(url);
		return response.data;
	},

	// Get a single series by ID
	getById: async (id: string): Promise<Series> => {
		const response = await api.get<Series>(`/series/${id}`);
		return response.data;
	},

	// Get series with in-progress books
	getInProgress: async (libraryId: string): Promise<Series[]> => {
		const params = new URLSearchParams();
		if (libraryId !== "all") {
			params.set("libraryId", libraryId);
		}
		const queryString = params.toString();
		const url = `/series/in-progress${queryString ? `?${queryString}` : ""}`;

		const response = await api.get<Series[]>(url);
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

	// Generate thumbnails for all books in series (queues a background task)
	generateThumbnails: async (
		seriesId: string,
	): Promise<{ task_id: string }> => {
		const response = await api.post<{ task_id: string }>(
			`/series/${seriesId}/thumbnails/generate`,
			{ force: true },
		);
		return response.data;
	},

	// Regenerate the series cover thumbnail (from first book's cover)
	regenerateSeriesThumbnail: async (
		seriesId: string,
		force = true,
	): Promise<{ task_id: string }> => {
		const response = await api.post<{ task_id: string }>(
			`/series/${seriesId}/thumbnail/generate`,
			{ force },
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
	getRecentlyAdded: async (
		libraryId: string,
		limit = 50,
	): Promise<Series[]> => {
		const params = new URLSearchParams();
		if (libraryId !== "all") {
			params.set("libraryId", libraryId);
		}
		params.set("limit", limit.toString());
		const queryString = params.toString();
		const url = `/series/recently-added?${queryString}`;

		const response = await api.get<Series[]>(url);
		return response.data;
	},

	// Get recently updated series
	getRecentlyUpdated: async (
		libraryId: string,
		limit = 50,
	): Promise<Series[]> => {
		const params = new URLSearchParams();
		if (libraryId !== "all") {
			params.set("libraryId", libraryId);
		}
		params.set("limit", limit.toString());
		const queryString = params.toString();
		const url = `/series/recently-updated?${queryString}`;

		const response = await api.get<Series[]>(url);
		return response.data;
	},

	// Get books in a series
	getBooks: async (
		seriesId: string,
		includeDeleted = false,
	): Promise<Book[]> => {
		const params = new URLSearchParams();
		if (includeDeleted) {
			params.set("includeDeleted", "true");
		}
		const queryString = params.toString();
		const url = `/series/${seriesId}/books${queryString ? `?${queryString}` : ""}`;

		const response = await api.get<Book[]>(url);
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
	search: async (
		libraryId: string,
		request: {
			condition?: SeriesCondition;
			search?: string;
			page?: number;
			pageSize?: number;
			sort?: string;
		},
	): Promise<PaginatedResponse<Series>> => {
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

		// Body only contains filter condition and search
		const body: SeriesListRequest = {
			condition: finalCondition,
			fullTextSearch: request.search,
		};

		const queryString = params.toString();
		const url = queryString ? `/series/list?${queryString}` : "/series/list";

		const response = await api.post<PaginatedResponse<Series>>(url, body);
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
