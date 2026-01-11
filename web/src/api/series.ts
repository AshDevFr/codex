import type { PaginatedResponse, Series } from "@/types";
import { api } from "./client";

export interface SeriesFilters {
	page?: number;
	pageSize?: number;
	sort?: string;
	genre?: string;
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

		if (filters?.page) params.set("page", filters.page.toString());
		if (filters?.pageSize)
			params.set("page_size", filters.pageSize.toString());
		if (filters?.sort) params.set("sort", filters.sort);
		if (filters?.genre) params.set("genre", filters.genre);
		if (filters?.status) params.set("status", filters.status);
		if (filters?.publisher) params.set("publisher", filters.publisher);
		if (filters?.year) params.set("year", filters.year.toString());

		const queryString = params.toString();
		const url =
			libraryId === "all"
				? `/series${queryString ? `?${queryString}` : ""}`
				: `/libraries/${libraryId}/series${queryString ? `?${queryString}` : ""}`;

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
		const url =
			libraryId === "all"
				? "/series/in-progress"
				: `/libraries/${libraryId}/series/in-progress`;

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

	// Mark all books in a series as read
	markAsRead: async (seriesId: string): Promise<{ count: number; message: string }> => {
		const response = await api.post<{ count: number; message: string }>(
			`/series/${seriesId}/read`,
		);
		return response.data;
	},

	// Mark all books in a series as unread
	markAsUnread: async (seriesId: string): Promise<{ count: number; message: string }> => {
		const response = await api.post<{ count: number; message: string }>(
			`/series/${seriesId}/unread`,
		);
		return response.data;
	},

	// Get recently added series
	getRecentlyAdded: async (libraryId: string, limit = 50): Promise<Series[]> => {
		const url =
			libraryId === "all"
				? `/series/recently-added?limit=${limit}`
				: `/libraries/${libraryId}/series/recently-added?limit=${limit}`;

		const response = await api.get<Series[]>(url);
		return response.data;
	},

	// Get recently updated series
	getRecentlyUpdated: async (libraryId: string, limit = 50): Promise<Series[]> => {
		const url =
			libraryId === "all"
				? `/series/recently-updated?limit=${limit}`
				: `/libraries/${libraryId}/series/recently-updated?limit=${limit}`;

		const response = await api.get<Series[]>(url);
		return response.data;
	},
};
