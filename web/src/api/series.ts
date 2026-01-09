import type { PaginatedResponse, Series } from "@/types/api";
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

	// Get series with started books (for "On Deck" section)
	getStarted: async (libraryId: string): Promise<Series[]> => {
		const url =
			libraryId === "all"
				? "/series/started"
				: `/libraries/${libraryId}/series/started`;

		const response = await api.get<Series[]>(url);
		return response.data;
	},
};
