import type {
	Book,
	BookCondition,
	BookListRequest,
	components,
	PaginatedResponse,
} from "@/types";
import { api } from "./client";

// Book error types (from generated API types)
export type BookErrorTypeDto = components["schemas"]["BookErrorTypeDto"];
export type BookErrorDto = components["schemas"]["BookErrorDto"];
export type BookWithErrorsDto = components["schemas"]["BookWithErrorsDto"];
export type ErrorGroupDto = components["schemas"]["ErrorGroupDto"];
export type BooksWithErrorsResponse =
	components["schemas"]["BooksWithErrorsResponse"];
export type RetryBookErrorsRequest =
	components["schemas"]["RetryBookErrorsRequest"];
export type RetryAllErrorsRequest =
	components["schemas"]["RetryAllErrorsRequest"];
export type RetryErrorsResponse = components["schemas"]["RetryErrorsResponse"];

export type BookDetailResponse = components["schemas"]["BookDetailResponse"];
export type BookMetadata = components["schemas"]["BookMetadataDto"];
export type AdjacentBooksResponse =
	components["schemas"]["AdjacentBooksResponse"];

// Book metadata lock types
export interface BookMetadataLocks {
	summaryLock: boolean;
	writerLock: boolean;
	pencillerLock: boolean;
	inkerLock: boolean;
	coloristLock: boolean;
	lettererLock: boolean;
	coverArtistLock: boolean;
	editorLock: boolean;
	publisherLock: boolean;
	imprintLock: boolean;
	genreLock: boolean;
	webLock: boolean;
	languageIsoLock: boolean;
	formatDetailLock: boolean;
	blackAndWhiteLock: boolean;
	mangaLock: boolean;
	yearLock: boolean;
	monthLock: boolean;
	dayLock: boolean;
	volumeLock: boolean;
	countLock: boolean;
	isbnsLock: boolean;
}

export type UpdateBookMetadataLocksRequest = Partial<BookMetadataLocks>;

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

		// Add library filter if not "all"
		if (libraryId !== "all") {
			params.set("library_id", libraryId);
		}

		if (filters?.page) params.set("page", filters.page.toString());
		if (filters?.pageSize) params.set("page_size", filters.pageSize.toString());
		if (filters?.sort) params.set("sort", filters.sort);
		if (filters?.series_id) params.set("series_id", filters.series_id);
		if (filters?.genre) params.set("genre", filters.genre);
		if (filters?.status) params.set("status", filters.status);

		const queryString = params.toString();
		const url = `/books${queryString ? `?${queryString}` : ""}`;

		const response = await api.get<PaginatedResponse<Book>>(url);
		return response.data;
	},

	// Get a single book by ID (basic info only)
	getById: async (id: string): Promise<Book> => {
		const response = await api.get<BookDetailResponse>(`/books/${id}`);
		return response.data.book;
	},

	// Get a single book with full details including metadata
	getDetail: async (id: string): Promise<BookDetailResponse> => {
		const response = await api.get<BookDetailResponse>(`/books/${id}`);
		return response.data;
	},

	// Get books with reading progress (incomplete reads)
	getInProgress: async (
		libraryId: string,
	): Promise<PaginatedResponse<Book>> => {
		const params = new URLSearchParams();
		if (libraryId !== "all") {
			params.set("library_id", libraryId);
		}
		const queryString = params.toString();
		const url = `/books/in-progress${queryString ? `?${queryString}` : ""}`;

		const response = await api.get<PaginatedResponse<Book>>(url);
		return response.data;
	},

	// Get on-deck books (next unread book in series where user has completed at least one book)
	getOnDeck: async (libraryId: string): Promise<PaginatedResponse<Book>> => {
		const params = new URLSearchParams();
		if (libraryId !== "all") {
			params.set("library_id", libraryId);
		}
		const queryString = params.toString();
		const url = `/books/on-deck${queryString ? `?${queryString}` : ""}`;

		const response = await api.get<PaginatedResponse<Book>>(url);
		return response.data;
	},

	// Get recently added books
	getRecentlyAdded: async (
		libraryId: string,
		limit = 50,
	): Promise<PaginatedResponse<Book>> => {
		const params = new URLSearchParams();
		if (libraryId !== "all") {
			params.set("library_id", libraryId);
		}
		params.set("page_size", limit.toString());
		const queryString = params.toString();
		const url = `/books/recently-added?${queryString}`;

		const response = await api.get<PaginatedResponse<Book>>(url);
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
		const params = new URLSearchParams();
		if (libraryId !== "all") {
			params.set("library_id", libraryId);
		}
		params.set("limit", limit.toString());
		const queryString = params.toString();
		const url = `/books/recently-read?${queryString}`;

		const response = await api.get<Book[]>(url);
		return response.data;
	},

	// Get adjacent books (previous and next) in the same series
	getAdjacent: async (bookId: string): Promise<AdjacentBooksResponse> => {
		const response = await api.get<AdjacentBooksResponse>(
			`/books/${bookId}/adjacent`,
		);
		return response.data;
	},

	// Generate thumbnail for a book (queues a background task)
	generateThumbnail: async (bookId: string): Promise<{ task_id: string }> => {
		const response = await api.post<{ task_id: string }>(
			`/books/${bookId}/thumbnail/generate`,
			{ force: true },
		);
		return response.data;
	},

	/**
	 * Search/filter books with advanced condition-based filtering.
	 *
	 * Uses POST /books/list endpoint which supports:
	 * - Nested AllOf/AnyOf conditions
	 * - Include/exclude filtering for genres, tags, read status, etc.
	 * - Full-text search (optional)
	 * - Pagination and sorting
	 * - Include deleted books (optional)
	 *
	 * @param libraryId - Library to filter by, or "all" for all libraries
	 * @param request - The search request with condition, pagination, and sort options
	 */
	search: async (
		libraryId: string,
		request: Omit<BookListRequest, "condition"> & { condition?: BookCondition },
	): Promise<PaginatedResponse<Book>> => {
		// Build the full condition including library filter
		let finalCondition: BookCondition | undefined = request.condition;

		// Add library filter if not "all"
		if (libraryId !== "all") {
			const libraryCondition: BookCondition = {
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

		const body: BookListRequest = {
			condition: finalCondition,
			search: request.search,
			page: request.page,
			pageSize: request.pageSize,
			sort: request.sort,
			includeDeleted: request.includeDeleted,
		};

		const response = await api.post<PaginatedResponse<Book>>(
			"/books/list",
			body,
		);
		return response.data;
	},

	// Get book metadata locks
	getMetadataLocks: async (bookId: string): Promise<BookMetadataLocks> => {
		const response = await api.get<BookMetadataLocks>(
			`/books/${bookId}/metadata/locks`,
		);
		return response.data;
	},

	// Update book metadata locks
	updateMetadataLocks: async (
		bookId: string,
		locks: UpdateBookMetadataLocksRequest,
	): Promise<BookMetadataLocks> => {
		const response = await api.put<BookMetadataLocks>(
			`/books/${bookId}/metadata/locks`,
			locks,
		);
		return response.data;
	},

	// Patch book metadata
	patchMetadata: async (
		bookId: string,
		metadata: components["schemas"]["PatchBookMetadataRequest"],
	): Promise<components["schemas"]["BookMetadataResponse"]> => {
		const response = await api.patch<
			components["schemas"]["BookMetadataResponse"]
		>(`/books/${bookId}/metadata`, metadata);
		return response.data;
	},

	/**
	 * Upload a custom cover image for a book
	 * The cover will be stored and used as the book's thumbnail
	 */
	uploadCover: async (bookId: string, file: File): Promise<void> => {
		const formData = new FormData();
		formData.append("cover", file);

		await api.post(`/books/${bookId}/cover`, formData, {
			headers: {
				"Content-Type": undefined,
			},
		});
	},

	/**
	 * Update book core fields (title, number)
	 * @param bookId - Book ID
	 * @param data - Fields to update (title and/or number)
	 */
	patch: async (
		bookId: string,
		data: { title?: string | null; number?: number | null },
	): Promise<{
		id: string;
		title: string | null;
		number: number | null;
		updatedAt: string;
	}> => {
		const response = await api.patch<{
			id: string;
			title: string | null;
			number: number | null;
			updatedAt: string;
		}>(`/books/${bookId}`, data);
		return response.data;
	},

	// ==================== Book Errors API ====================

	/**
	 * Get books with errors, grouped by error type
	 * @param options - Filter and pagination options
	 */
	getBooksWithErrors: async (options?: {
		page?: number;
		pageSize?: number;
		errorType?: BookErrorTypeDto;
		libraryId?: string;
		seriesId?: string;
	}): Promise<BooksWithErrorsResponse> => {
		const params = new URLSearchParams();

		if (options?.page !== undefined)
			params.set("page", options.page.toString());
		if (options?.pageSize !== undefined)
			params.set("page_size", options.pageSize.toString());
		if (options?.errorType) params.set("error_type", options.errorType);
		if (options?.libraryId) params.set("library_id", options.libraryId);
		if (options?.seriesId) params.set("series_id", options.seriesId);

		const queryString = params.toString();
		const url = `/books/errors${queryString ? `?${queryString}` : ""}`;

		const response = await api.get<BooksWithErrorsResponse>(url);
		return response.data;
	},

	/**
	 * Retry analysis or thumbnail generation for a specific book
	 * @param bookId - Book ID to retry
	 * @param errorTypes - Optional specific error types to retry (if not provided, retries all error types)
	 */
	retryBookErrors: async (
		bookId: string,
		errorTypes?: BookErrorTypeDto[],
	): Promise<RetryErrorsResponse> => {
		const body: RetryBookErrorsRequest = {
			errorTypes: errorTypes ?? null,
		};

		const response = await api.post<RetryErrorsResponse>(
			`/books/${bookId}/retry`,
			body,
		);
		return response.data;
	},

	/**
	 * Retry all books with errors (bulk operation)
	 * @param options - Optional filters for which errors to retry
	 */
	retryAllErrors: async (options?: {
		errorType?: BookErrorTypeDto;
		libraryId?: string;
	}): Promise<RetryErrorsResponse> => {
		const body: RetryAllErrorsRequest = {
			errorType: options?.errorType ?? null,
			libraryId: options?.libraryId ?? null,
		};

		const response = await api.post<RetryErrorsResponse>(
			"/books/retry-all-errors",
			body,
		);
		return response.data;
	},
};
