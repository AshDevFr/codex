import type { Book, PaginatedResponse, Series } from "@/types";
import { api } from "./client";

export interface SearchResults {
	series: Series[];
	books: Book[];
}

export interface SearchRequest {
	query: string;
	libraryId?: string;
	limit?: number;
}

export const searchApi = {
	/**
	 * Search series by name
	 * Uses the dedicated series search endpoint
	 */
	searchSeries: async (
		query: string,
		libraryId?: string,
	): Promise<Series[]> => {
		const body: { query: string; libraryId?: string } = { query };
		if (libraryId && libraryId !== "all") {
			body.libraryId = libraryId;
		}

		const response = await api.post<Series[]>("/series/search", body);
		return response.data;
	},

	/**
	 * Search books by title (case-insensitive)
	 * Uses the books list endpoint with fullTextSearch for case-insensitive matching
	 */
	searchBooks: async (
		query: string,
		libraryId?: string,
		limit = 10,
	): Promise<Book[]> => {
		interface BookCondition {
			libraryId?: { operator: "is"; value: string };
		}

		// Build query params for pagination
		const params = new URLSearchParams();
		params.set("page", "1");
		params.set("pageSize", String(limit));

		// Build request body with fullTextSearch for case-insensitive search
		const body: {
			fullTextSearch: string;
			condition?: BookCondition;
		} = {
			fullTextSearch: query,
		};

		// Add library filter if specified
		if (libraryId && libraryId !== "all") {
			body.condition = {
				libraryId: { operator: "is", value: libraryId },
			};
		}

		const response = await api.post<PaginatedResponse<Book>>(
			`/books/list?${params.toString()}`,
			body,
		);

		return response.data.data;
	},

	/**
	 * Combined search for both series and books
	 * Returns results grouped by type
	 */
	search: async (request: SearchRequest): Promise<SearchResults> => {
		const { query, libraryId, limit = 10 } = request;

		// Search both in parallel
		const [series, books] = await Promise.all([
			searchApi.searchSeries(query, libraryId).catch((err) => {
				console.error("Series search failed:", err);
				return [] as Series[];
			}),
			searchApi.searchBooks(query, libraryId, limit).catch((err) => {
				console.error("Books search failed:", err);
				return [] as Book[];
			}),
		]);

		return {
			series: series.slice(0, limit),
			books,
		};
	},
};
