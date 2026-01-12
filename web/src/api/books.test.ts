import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { booksApi } from "./books";
import { api } from "./client";

// Mock the api client
vi.mock("./client", () => ({
	api: {
		get: vi.fn(),
		post: vi.fn(),
	},
}));

describe("booksApi", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	describe("getByLibrary", () => {
		it("should fetch books for a specific library", async () => {
			const mockResponse = {
				items: [
					{ id: "book-1", title: "Book 1", fileFormat: "cbz" },
					{ id: "book-2", title: "Book 2", fileFormat: "epub" },
				],
				total: 2,
				page: 1,
				pageSize: 20,
				totalPages: 1,
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await booksApi.getByLibrary("library-123");

			expect(api.get).toHaveBeenCalledWith("/books?library_id=library-123");
			expect(result).toEqual(mockResponse);
		});

		it("should fetch all books when libraryId is 'all'", async () => {
			const mockResponse = {
				items: [{ id: "book-1", title: "Book 1" }],
				total: 1,
				page: 1,
				pageSize: 20,
				totalPages: 1,
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await booksApi.getByLibrary("all");

			expect(api.get).toHaveBeenCalledWith("/books");
			expect(result).toEqual(mockResponse);
		});

		it("should include filter parameters", async () => {
			const mockResponse = { items: [], total: 0, page: 1, pageSize: 10, totalPages: 0 };

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			await booksApi.getByLibrary("library-123", {
				page: 2,
				pageSize: 10,
				sort: "title",
				series_id: "series-1",
				genre: "Action",
				status: "unread",
			});

			expect(api.get).toHaveBeenCalledWith(
				"/books?library_id=library-123&page=2&page_size=10&sort=title&series_id=series-1&genre=Action&status=unread",
			);
		});
	});

	describe("getById", () => {
		it("should fetch a book by ID (basic info)", async () => {
			const mockResponse = {
				book: {
					id: "book-123",
					title: "Test Book",
					fileFormat: "cbz",
					pageCount: 200,
				},
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await booksApi.getById("book-123");

			expect(api.get).toHaveBeenCalledWith("/books/book-123");
			expect(result).toEqual(mockResponse.book);
		});
	});

	describe("getDetail", () => {
		it("should fetch book detail with metadata", async () => {
			const mockResponse = {
				book: {
					id: "book-123",
					title: "Test Book",
					fileFormat: "cbz",
					pageCount: 200,
					seriesId: "series-1",
					seriesName: "Test Series",
				},
				metadata: {
					title: "Test Book",
					writer: "Author Name",
					penciller: "Artist Name",
				},
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await booksApi.getDetail("book-123");

			expect(api.get).toHaveBeenCalledWith("/books/book-123");
			expect(result).toEqual(mockResponse);
			expect(result.book).toEqual(mockResponse.book);
			expect(result.metadata).toEqual(mockResponse.metadata);
		});
	});

	describe("getAdjacent", () => {
		it("should fetch adjacent books in a series", async () => {
			const mockResponse = {
				prev: {
					id: "book-1",
					title: "Book 1",
					number: 1,
				},
				next: {
					id: "book-3",
					title: "Book 3",
					number: 3,
				},
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await booksApi.getAdjacent("book-2");

			expect(api.get).toHaveBeenCalledWith("/books/book-2/adjacent");
			expect(result).toEqual(mockResponse);
			expect(result.prev?.title).toBe("Book 1");
			expect(result.next?.title).toBe("Book 3");
		});

		it("should return null for prev when at start of series", async () => {
			const mockResponse = {
				prev: null,
				next: {
					id: "book-2",
					title: "Book 2",
					number: 2,
				},
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await booksApi.getAdjacent("book-1");

			expect(result.prev).toBeNull();
			expect(result.next).toBeDefined();
		});

		it("should return null for next when at end of series", async () => {
			const mockResponse = {
				prev: {
					id: "book-4",
					title: "Book 4",
					number: 4,
				},
				next: null,
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await booksApi.getAdjacent("book-5");

			expect(result.prev).toBeDefined();
			expect(result.next).toBeNull();
		});

		it("should return both null for single-book series", async () => {
			const mockResponse = {
				prev: null,
				next: null,
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await booksApi.getAdjacent("only-book");

			expect(result.prev).toBeNull();
			expect(result.next).toBeNull();
		});
	});

	describe("getInProgress", () => {
		it("should fetch in-progress books for a library", async () => {
			const mockResponse = {
				data: [
					{ id: "book-1", title: "Reading Book 1", readProgress: { currentPage: 50, totalPages: 200 } },
				],
				total: 1,
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await booksApi.getInProgress("library-123");

			expect(api.get).toHaveBeenCalledWith("/books/in-progress?library_id=library-123");
			expect(result).toEqual(mockResponse);
		});

		it("should fetch all in-progress books when libraryId is 'all'", async () => {
			vi.mocked(api.get).mockResolvedValueOnce({ data: { data: [], total: 0 } });

			await booksApi.getInProgress("all");

			expect(api.get).toHaveBeenCalledWith("/books/in-progress");
		});
	});

	describe("getOnDeck", () => {
		it("should fetch on-deck books for a library", async () => {
			const mockResponse = {
				items: [{ id: "book-next", title: "Next in Series" }],
				total: 1,
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await booksApi.getOnDeck("library-123");

			expect(api.get).toHaveBeenCalledWith("/books/on-deck?library_id=library-123");
			expect(result).toEqual(mockResponse);
		});
	});

	describe("getRecentlyAdded", () => {
		it("should fetch recently added books with default limit", async () => {
			const mockResponse = {
				data: [{ id: "book-new", title: "New Book" }],
				total: 1,
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await booksApi.getRecentlyAdded("library-123");

			expect(api.get).toHaveBeenCalledWith("/books/recently-added?library_id=library-123&page_size=50");
			expect(result).toEqual(mockResponse);
		});

		it("should fetch recently added books with custom limit", async () => {
			vi.mocked(api.get).mockResolvedValueOnce({ data: { data: [], total: 0 } });

			await booksApi.getRecentlyAdded("all", 10);

			expect(api.get).toHaveBeenCalledWith("/books/recently-added?page_size=10");
		});
	});

	describe("getRecentlyRead", () => {
		it("should fetch recently read books", async () => {
			const mockBooks = [{ id: "book-1", title: "Recently Read" }];

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockBooks });

			const result = await booksApi.getRecentlyRead("library-123", 20);

			expect(api.get).toHaveBeenCalledWith("/books/recently-read?library_id=library-123&limit=20");
			expect(result).toEqual(mockBooks);
		});
	});

	describe("analyze", () => {
		it("should trigger book analysis", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: { message: "Analysis queued" },
			});

			const result = await booksApi.analyze("book-123");

			expect(api.post).toHaveBeenCalledWith("/books/book-123/analyze");
			expect(result).toEqual({ message: "Analysis queued" });
		});
	});

	describe("analyzeUnanalyzed", () => {
		it("should trigger analysis for unanalyzed book", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: { message: "Analysis queued" },
			});

			const result = await booksApi.analyzeUnanalyzed("book-123");

			expect(api.post).toHaveBeenCalledWith("/books/book-123/analyze-unanalyzed");
			expect(result).toEqual({ message: "Analysis queued" });
		});
	});

	describe("markAsRead", () => {
		it("should mark a book as read", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({ data: {} });

			await booksApi.markAsRead("book-123");

			expect(api.post).toHaveBeenCalledWith("/books/book-123/read");
		});
	});

	describe("markAsUnread", () => {
		it("should mark a book as unread", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({ data: {} });

			await booksApi.markAsUnread("book-123");

			expect(api.post).toHaveBeenCalledWith("/books/book-123/unread");
		});
	});
});
