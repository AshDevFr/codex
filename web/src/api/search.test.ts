import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./client";
import { searchApi } from "./search";

// Mock the api client
vi.mock("./client", () => ({
	api: {
		get: vi.fn(),
		post: vi.fn(),
	},
}));

describe("searchApi", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	describe("searchSeries", () => {
		it("should search series by query", async () => {
			const mockSeries = [
				{ id: "series-1", name: "Batman", bookCount: 10 },
				{ id: "series-2", name: "Batman Beyond", bookCount: 5 },
			];

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockSeries });

			const result = await searchApi.searchSeries("batman");

			expect(api.post).toHaveBeenCalledWith("/series/search", {
				query: "batman",
			});
			expect(result).toEqual(mockSeries);
		});

		it("should include libraryId when specified", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({ data: [] });

			await searchApi.searchSeries("batman", "library-123");

			expect(api.post).toHaveBeenCalledWith("/series/search", {
				query: "batman",
				libraryId: "library-123",
			});
		});

		it("should not include libraryId when 'all' is specified", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({ data: [] });

			await searchApi.searchSeries("batman", "all");

			expect(api.post).toHaveBeenCalledWith("/series/search", {
				query: "batman",
			});
		});
	});

	describe("searchBooks", () => {
		it("should search books by title using fullTextSearch", async () => {
			const mockBooks = [
				{ id: "book-1", title: "Batman Year One", pageCount: 200 },
				{ id: "book-2", title: "Batman: The Long Halloween", pageCount: 350 },
			];
			const mockResponse = {
				data: mockBooks,
				total: 2,
				page: 1,
				pageSize: 10,
			};

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockResponse });

			const result = await searchApi.searchBooks("batman");

			expect(api.post).toHaveBeenCalledWith("/books/list?page=1&pageSize=10", {
				fullTextSearch: "batman",
			});
			expect(result).toEqual(mockBooks);
		});

		it("should include library filter when libraryId is specified", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: { data: [], total: 0, page: 1, pageSize: 10 },
			});

			await searchApi.searchBooks("batman", "library-123");

			expect(api.post).toHaveBeenCalledWith("/books/list?page=1&pageSize=10", {
				fullTextSearch: "batman",
				condition: {
					libraryId: { operator: "is", value: "library-123" },
				},
			});
		});

		it("should not include library filter when 'all' is specified", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: { data: [], total: 0, page: 1, pageSize: 10 },
			});

			await searchApi.searchBooks("batman", "all");

			expect(api.post).toHaveBeenCalledWith("/books/list?page=1&pageSize=10", {
				fullTextSearch: "batman",
			});
		});

		it("should respect custom limit parameter", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: { data: [], total: 0, page: 1, pageSize: 20 },
			});

			await searchApi.searchBooks("batman", undefined, 20);

			expect(api.post).toHaveBeenCalledWith("/books/list?page=1&pageSize=20", {
				fullTextSearch: "batman",
			});
		});
	});

	describe("search", () => {
		it("should search both series and books in parallel", async () => {
			const mockSeries = [{ id: "series-1", title: "Batman", bookCount: 10 }];
			const mockBooks = [{ id: "book-1", title: "Batman Year One" }];
			const mockBooksResponse = {
				data: mockBooks,
				total: 1,
				page: 1,
				pageSize: 10,
			};

			// First call is searchSeries, second is searchBooks
			vi.mocked(api.post)
				.mockResolvedValueOnce({ data: mockSeries })
				.mockResolvedValueOnce({ data: mockBooksResponse });

			const result = await searchApi.search({ query: "batman" });

			expect(api.post).toHaveBeenCalledTimes(2);
			expect(result.series).toEqual(mockSeries);
			expect(result.books).toEqual(mockBooks);
		});

		it("should return empty arrays when searches fail", async () => {
			const consoleSpy = vi
				.spyOn(console, "error")
				.mockImplementation(() => {});

			vi.mocked(api.post)
				.mockRejectedValueOnce(new Error("Network error"))
				.mockRejectedValueOnce(new Error("Network error"));

			const result = await searchApi.search({ query: "batman" });

			expect(result.series).toEqual([]);
			expect(result.books).toEqual([]);
			expect(consoleSpy).toHaveBeenCalledTimes(2);
			expect(consoleSpy).toHaveBeenCalledWith(
				"Series search failed:",
				expect.any(Error),
			);
			expect(consoleSpy).toHaveBeenCalledWith(
				"Books search failed:",
				expect.any(Error),
			);
		});

		it("should limit results to specified limit", async () => {
			const mockSeries = Array.from({ length: 20 }, (_, i) => ({
				id: `series-${i}`,
				title: `Series ${i}`,
				bookCount: i,
			}));

			vi.mocked(api.post)
				.mockResolvedValueOnce({ data: mockSeries })
				.mockResolvedValueOnce({
					data: { data: [], total: 0, page: 1, pageSize: 5 },
				});

			const result = await searchApi.search({ query: "test", limit: 5 });

			expect(result.series).toHaveLength(5);
		});

		it("should pass libraryId to both searches", async () => {
			vi.mocked(api.post)
				.mockResolvedValueOnce({ data: [] })
				.mockResolvedValueOnce({
					data: { data: [], total: 0, page: 1, pageSize: 10 },
				});

			await searchApi.search({ query: "batman", libraryId: "library-123" });

			// Check series search
			expect(api.post).toHaveBeenNthCalledWith(1, "/series/search", {
				query: "batman",
				libraryId: "library-123",
			});

			// Check books search
			expect(api.post).toHaveBeenNthCalledWith(
				2,
				"/books/list?page=1&pageSize=10",
				{
					fullTextSearch: "batman",
					condition: {
						libraryId: { operator: "is", value: "library-123" },
					},
				},
			);
		});
	});
});
