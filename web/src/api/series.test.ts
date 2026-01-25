import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./client";
import { seriesApi } from "./series";

// Mock the api client
vi.mock("./client", () => ({
	api: {
		get: vi.fn(),
		post: vi.fn(),
	},
}));

describe("seriesApi", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	describe("getByLibrary", () => {
		it("should fetch series for a specific library", async () => {
			const mockResponse = {
				items: [
					{ id: "series-1", title: "Series 1", bookCount: 5 },
					{ id: "series-2", title: "Series 2", bookCount: 3 },
				],
				total: 2,
				page: 1,
				pageSize: 20,
				totalPages: 1,
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await seriesApi.getByLibrary("library-123");

			expect(api.get).toHaveBeenCalledWith("/series?libraryId=library-123");
			expect(result).toEqual(mockResponse);
		});

		it("should fetch all series when libraryId is 'all'", async () => {
			const mockResponse = {
				items: [{ id: "series-1", title: "Series 1" }],
				total: 1,
				page: 1,
				pageSize: 20,
				totalPages: 1,
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await seriesApi.getByLibrary("all");

			expect(api.get).toHaveBeenCalledWith("/series");
			expect(result).toEqual(mockResponse);
		});

		it("should include filter parameters", async () => {
			const mockResponse = {
				items: [],
				total: 0,
				page: 1,
				pageSize: 10,
				totalPages: 0,
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			await seriesApi.getByLibrary("library-123", {
				page: 2,
				pageSize: 10,
				sort: "name",
				genres: "Action,Comedy",
				tags: "Favorite",
				status: "ongoing",
				publisher: "Marvel",
				year: 2024,
			});

			expect(api.get).toHaveBeenCalledWith(
				"/series?libraryId=library-123&page=2&pageSize=10&sort=name&genres=Action%2CComedy&tags=Favorite&status=ongoing&publisher=Marvel&year=2024",
			);
		});
	});

	describe("getById", () => {
		it("should fetch a series by ID", async () => {
			const mockSeries = {
				id: "series-123",
				title: "Test Series",
				bookCount: 10,
				libraryId: "library-1",
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockSeries });

			const result = await seriesApi.getById("series-123");

			expect(api.get).toHaveBeenCalledWith("/series/series-123");
			expect(result).toEqual(mockSeries);
		});
	});

	describe("getInProgress", () => {
		it("should fetch in-progress series for a library", async () => {
			const mockSeries = [
				{ id: "series-1", title: "Reading Series 1", bookCount: 10 },
			];

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockSeries });

			const result = await seriesApi.getInProgress("library-123");

			expect(api.get).toHaveBeenCalledWith(
				"/series/in-progress?libraryId=library-123",
			);
			expect(result).toEqual(mockSeries);
		});

		it("should fetch all in-progress series when libraryId is 'all'", async () => {
			vi.mocked(api.get).mockResolvedValueOnce({ data: [] });

			await seriesApi.getInProgress("all");

			expect(api.get).toHaveBeenCalledWith("/series/in-progress");
		});
	});

	describe("analyze", () => {
		it("should trigger series analysis (force all books)", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: { message: "Analysis queued for all books" },
			});

			const result = await seriesApi.analyze("series-123");

			expect(api.post).toHaveBeenCalledWith("/series/series-123/analyze");
			expect(result).toEqual({ message: "Analysis queued for all books" });
		});
	});

	describe("analyzeUnanalyzed", () => {
		it("should trigger analysis for unanalyzed books only", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: { message: "Analysis queued for unanalyzed books" },
			});

			const result = await seriesApi.analyzeUnanalyzed("series-123");

			expect(api.post).toHaveBeenCalledWith(
				"/series/series-123/analyze-unanalyzed",
			);
			expect(result).toEqual({
				message: "Analysis queued for unanalyzed books",
			});
		});
	});

	describe("markAsRead", () => {
		it("should mark all books in series as read", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: { count: 10, message: "Marked 10 books as read" },
			});

			const result = await seriesApi.markAsRead("series-123");

			expect(api.post).toHaveBeenCalledWith("/series/series-123/read");
			expect(result).toEqual({ count: 10, message: "Marked 10 books as read" });
		});
	});

	describe("markAsUnread", () => {
		it("should mark all books in series as unread", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: { count: 10, message: "Marked 10 books as unread" },
			});

			const result = await seriesApi.markAsUnread("series-123");

			expect(api.post).toHaveBeenCalledWith("/series/series-123/unread");
			expect(result).toEqual({
				count: 10,
				message: "Marked 10 books as unread",
			});
		});
	});

	describe("getRecentlyAdded", () => {
		it("should fetch recently added series with default limit", async () => {
			const mockSeries = [{ id: "series-new", title: "New Series" }];

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockSeries });

			const result = await seriesApi.getRecentlyAdded("library-123");

			expect(api.get).toHaveBeenCalledWith(
				"/series/recently-added?libraryId=library-123&limit=50",
			);
			expect(result).toEqual(mockSeries);
		});

		it("should fetch recently added series with custom limit", async () => {
			vi.mocked(api.get).mockResolvedValueOnce({ data: [] });

			await seriesApi.getRecentlyAdded("all", 10);

			expect(api.get).toHaveBeenCalledWith("/series/recently-added?limit=10");
		});
	});

	describe("getRecentlyUpdated", () => {
		it("should fetch recently updated series with default limit", async () => {
			const mockSeries = [{ id: "series-1", title: "Updated Series" }];

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockSeries });

			const result = await seriesApi.getRecentlyUpdated("library-123");

			expect(api.get).toHaveBeenCalledWith(
				"/series/recently-updated?libraryId=library-123&limit=50",
			);
			expect(result).toEqual(mockSeries);
		});

		it("should fetch recently updated series with custom limit", async () => {
			vi.mocked(api.get).mockResolvedValueOnce({ data: [] });

			await seriesApi.getRecentlyUpdated("all", 25);

			expect(api.get).toHaveBeenCalledWith("/series/recently-updated?limit=25");
		});
	});

	describe("search", () => {
		const mockPaginatedResponse = {
			data: [
				{ id: "series-1", title: "Action Series", bookCount: 5 },
				{ id: "series-2", title: "Comedy Series", bookCount: 3 },
			],
			total: 2,
			page: 1,
			pageSize: 50,
		};

		it("should search series without any condition for 'all' library", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: mockPaginatedResponse,
			});

			const result = await seriesApi.search("all", {
				page: 1,
				pageSize: 50,
			});

			// Pagination params are now in URL, body contains only filter
			expect(api.post).toHaveBeenCalledWith("/series/list?page=1&pageSize=50", {
				condition: undefined,
				fullTextSearch: undefined,
			});
			expect(result).toEqual(mockPaginatedResponse);
		});

		it("should add library condition when libraryId is specified", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: mockPaginatedResponse,
			});

			await seriesApi.search("library-123", {
				page: 1,
				pageSize: 50,
			});

			expect(api.post).toHaveBeenCalledWith("/series/list?page=1&pageSize=50", {
				condition: {
					libraryId: { operator: "is", value: "library-123" },
				},
				fullTextSearch: undefined,
			});
		});

		it("should combine library condition with existing condition", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: mockPaginatedResponse,
			});

			const genreCondition = {
				genre: { operator: "is" as const, value: "Action" },
			};

			await seriesApi.search("library-123", {
				condition: genreCondition,
				page: 1,
				pageSize: 50,
			});

			expect(api.post).toHaveBeenCalledWith("/series/list?page=1&pageSize=50", {
				condition: {
					allOf: [
						{ libraryId: { operator: "is", value: "library-123" } },
						genreCondition,
					],
				},
				fullTextSearch: undefined,
			});
		});

		it("should pass through search and sort parameters", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: mockPaginatedResponse,
			});

			await seriesApi.search("all", {
				search: "naruto",
				page: 1,
				pageSize: 10,
				sort: "name,asc",
			});

			expect(api.post).toHaveBeenCalledWith(
				"/series/list?page=1&pageSize=10&sort=name%2Casc",
				{
					condition: undefined,
					fullTextSearch: "naruto",
				},
			);
		});

		it("should handle complex nested conditions", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: mockPaginatedResponse,
			});

			const complexCondition = {
				allOf: [
					{
						anyOf: [
							{ genre: { operator: "is" as const, value: "Action" } },
							{ genre: { operator: "is" as const, value: "Comedy" } },
						],
					},
					{ genre: { operator: "isNot" as const, value: "Horror" } },
				],
			};

			await seriesApi.search("all", {
				condition: complexCondition,
				page: 1,
				pageSize: 50,
			});

			expect(api.post).toHaveBeenCalledWith("/series/list?page=1&pageSize=50", {
				condition: complexCondition,
				fullTextSearch: undefined,
			});
		});
	});
});
