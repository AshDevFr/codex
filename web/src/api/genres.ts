import type { PaginatedResponse } from "@/types";
import type { components } from "@/types/api.generated";
import { api } from "./client";

export type Genre = components["schemas"]["GenreDto"];
export type GenreListResponse = components["schemas"]["GenreListResponse"];

const MAX_PAGE_SIZE = 500;

export const genresApi = {
	/**
	 * Get all genres across all libraries.
	 * Fetches all pages if results are paginated.
	 */
	getAll: async (): Promise<Genre[]> => {
		const allGenres: Genre[] = [];
		let page = 1;
		let hasMore = true;

		while (hasMore) {
			const response = await api.get<PaginatedResponse<Genre>>("/genres", {
				params: { page, pageSize: MAX_PAGE_SIZE },
			});
			const data = response.data.data ?? [];
			allGenres.push(...data);

			hasMore = page < response.data.totalPages;
			page++;
		}

		return allGenres;
	},

	/**
	 * Get genres for a specific series
	 */
	getForSeries: async (seriesId: string): Promise<Genre[]> => {
		const response = await api.get<GenreListResponse>(
			`/series/${seriesId}/genres`,
		);
		return response.data.genres;
	},

	/**
	 * Set all genres for a series (replaces existing)
	 */
	setForSeries: async (
		seriesId: string,
		genres: string[],
	): Promise<Genre[]> => {
		const response = await api.put<GenreListResponse>(
			`/series/${seriesId}/genres`,
			{ genres },
		);
		return response.data.genres;
	},

	/**
	 * Add a single genre to a series
	 */
	addToSeries: async (seriesId: string, name: string): Promise<Genre> => {
		const response = await api.post<Genre>(`/series/${seriesId}/genres`, {
			name,
		});
		return response.data;
	},

	/**
	 * Remove a genre from a series
	 */
	removeFromSeries: async (
		seriesId: string,
		genreId: string,
	): Promise<void> => {
		await api.delete(`/series/${seriesId}/genres/${genreId}`);
	},

	/**
	 * Delete a genre globally
	 */
	delete: async (genreId: string): Promise<void> => {
		await api.delete(`/genres/${genreId}`);
	},

	/**
	 * Clean up unused genres (admin only)
	 */
	cleanup: async (): Promise<{ deleted_count: number }> => {
		const response = await api.post<{ deleted_count: number }>(
			"/genres/cleanup",
		);
		return response.data;
	},
};
