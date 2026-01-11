import type { Book, PaginatedResponse } from "@/types";
import { api } from "./client";

export interface BookFilters {
	page?: number;
	pageSize?: number;
	sort?: string;
	series_id?: string;
	genre?: string;
	status?: string;
}

export const booksApi = {
	// Get books by library ID with filters
	getByLibrary: async (
		libraryId: string,
		filters?: BookFilters,
	): Promise<PaginatedResponse<Book>> => {
		const params = new URLSearchParams();

		if (filters?.page) params.set("page", filters.page.toString());
		if (filters?.pageSize)
			params.set("page_size", filters.pageSize.toString());
		if (filters?.sort) params.set("sort", filters.sort);
		if (filters?.series_id) params.set("series_id", filters.series_id);
		if (filters?.genre) params.set("genre", filters.genre);
		if (filters?.status) params.set("status", filters.status);

		const queryString = params.toString();
		const url =
			libraryId === "all"
				? `/books${queryString ? `?${queryString}` : ""}`
				: `/libraries/${libraryId}/books${queryString ? `?${queryString}` : ""}`;

		const response = await api.get<PaginatedResponse<Book>>(url);
		return response.data;
	},

	// Get a single book by ID
	getById: async (id: string): Promise<Book> => {
		const response = await api.get<Book>(`/books/${id}`);
		return response.data;
	},

	// Get books with reading progress (incomplete reads)
	getInProgress: async (libraryId: string): Promise<Book[]> => {
		const url =
			libraryId === "all"
				? "/books/in-progress"
				: `/libraries/${libraryId}/books/in-progress`;

		const response = await api.get<Book[]>(url);
		return response.data;
	},

	// Get on-deck books (next unread book in series where user has completed at least one book)
	getOnDeck: async (libraryId: string): Promise<PaginatedResponse<Book>> => {
		const url =
			libraryId === "all"
				? "/books/on-deck"
				: `/libraries/${libraryId}/books/on-deck`;

		const response = await api.get<PaginatedResponse<Book>>(url);
		return response.data;
	},

	// Get recently added books
	getRecentlyAdded: async (
		libraryId: string,
		limit = 50,
	): Promise<Book[]> => {
		const url =
			libraryId === "all"
				? `/books/recently-added?limit=${limit}`
				: `/libraries/${libraryId}/books/recently-added?limit=${limit}`;

		const response = await api.get<Book[]>(url);
		return response.data;
	},

	// Trigger book analysis (force)
	analyze: async (bookId: string): Promise<{ message: string }> => {
		const response = await api.post<{ message: string }>(
			`/books/${bookId}/analyze`,
		);
		return response.data;
	},

	// Trigger book analysis if not already analyzed
	analyzeUnanalyzed: async (bookId: string): Promise<{ message: string }> => {
		const response = await api.post<{ message: string }>(
			`/books/${bookId}/analyze-unanalyzed`,
		);
		return response.data;
	},

	// Mark a book as read
	markAsRead: async (bookId: string): Promise<void> => {
		const response = await api.post(`/books/${bookId}/read`);
		return response.data;
	},

	// Mark a book as unread
	markAsUnread: async (bookId: string): Promise<void> => {
		const response = await api.post(`/books/${bookId}/unread`);
		return response.data;
	},

	// Get recently read books (ordered by last read activity)
	getRecentlyRead: async (libraryId: string, limit = 50): Promise<Book[]> => {
		const url =
			libraryId === "all"
				? `/books/recently-read?limit=${limit}`
				: `/libraries/${libraryId}/books/recently-read?limit=${limit}`;

		const response = await api.get<Book[]>(url);
		return response.data;
	},
};
