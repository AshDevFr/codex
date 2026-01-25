import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./client";
import { genresApi } from "./genres";

// Mock the api client
vi.mock("./client", () => ({
	api: {
		get: vi.fn(),
		put: vi.fn(),
		post: vi.fn(),
		delete: vi.fn(),
	},
}));

describe("genresApi", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	describe("getAll", () => {
		it("should fetch all genres from a single page", async () => {
			const mockGenres = [
				{ id: "genre-1", name: "Action", seriesCount: 10 },
				{ id: "genre-2", name: "Comedy", seriesCount: 5 },
				{ id: "genre-3", name: "Drama", seriesCount: 8 },
			];
			const mockResponse = {
				data: mockGenres,
				page: 1,
				pageSize: 500,
				total: 3,
				totalPages: 1,
				links: {
					self: "/api/v1/genres?page=1&pageSize=500",
				},
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await genresApi.getAll();

			expect(api.get).toHaveBeenCalledWith("/genres", {
				params: { page: 1, pageSize: 500 },
			});
			expect(result).toEqual(mockGenres);
		});

		it("should fetch all genres across multiple pages", async () => {
			const page1Genres = [
				{ id: "genre-1", name: "Action", seriesCount: 10 },
				{ id: "genre-2", name: "Comedy", seriesCount: 5 },
			];
			const page2Genres = [
				{ id: "genre-3", name: "Drama", seriesCount: 8 },
				{ id: "genre-4", name: "Horror", seriesCount: 3 },
			];

			vi.mocked(api.get)
				.mockResolvedValueOnce({
					data: {
						data: page1Genres,
						page: 1,
						pageSize: 500,
						total: 4,
						totalPages: 2,
						links: { self: "/api/v1/genres?page=1&pageSize=500" },
					},
				})
				.mockResolvedValueOnce({
					data: {
						data: page2Genres,
						page: 2,
						pageSize: 500,
						total: 4,
						totalPages: 2,
						links: { self: "/api/v1/genres?page=2&pageSize=500" },
					},
				});

			const result = await genresApi.getAll();

			expect(api.get).toHaveBeenCalledTimes(2);
			expect(api.get).toHaveBeenNthCalledWith(1, "/genres", {
				params: { page: 1, pageSize: 500 },
			});
			expect(api.get).toHaveBeenNthCalledWith(2, "/genres", {
				params: { page: 2, pageSize: 500 },
			});
			expect(result).toEqual([...page1Genres, ...page2Genres]);
		});

		it("should return empty array when no genres exist", async () => {
			vi.mocked(api.get).mockResolvedValueOnce({
				data: {
					data: [],
					page: 1,
					pageSize: 500,
					total: 0,
					totalPages: 0,
					links: {
						self: "/api/v1/genres?page=1&pageSize=500",
					},
				},
			});

			const result = await genresApi.getAll();

			expect(result).toEqual([]);
		});
	});

	describe("getForSeries", () => {
		it("should fetch genres for a specific series", async () => {
			const mockResponse = {
				genres: [
					{ id: "genre-1", name: "Action", seriesCount: 10 },
					{ id: "genre-2", name: "Adventure", seriesCount: 7 },
				],
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await genresApi.getForSeries("series-123");

			expect(api.get).toHaveBeenCalledWith("/series/series-123/genres");
			expect(result).toEqual(mockResponse.genres);
		});
	});

	describe("setForSeries", () => {
		it("should set genres for a series (replacing existing)", async () => {
			const mockResponse = {
				genres: [
					{ id: "genre-1", name: "Action", seriesCount: 11 },
					{ id: "genre-3", name: "Sci-Fi", seriesCount: 1 },
				],
			};

			vi.mocked(api.put).mockResolvedValueOnce({ data: mockResponse });

			const result = await genresApi.setForSeries("series-123", [
				"Action",
				"Sci-Fi",
			]);

			expect(api.put).toHaveBeenCalledWith("/series/series-123/genres", {
				genres: ["Action", "Sci-Fi"],
			});
			expect(result).toEqual(mockResponse.genres);
		});

		it("should clear all genres when given empty array", async () => {
			vi.mocked(api.put).mockResolvedValueOnce({ data: { genres: [] } });

			const result = await genresApi.setForSeries("series-123", []);

			expect(api.put).toHaveBeenCalledWith("/series/series-123/genres", {
				genres: [],
			});
			expect(result).toEqual([]);
		});
	});

	describe("addToSeries", () => {
		it("should add a single genre to a series", async () => {
			const mockGenre = { id: "genre-new", name: "Horror", seriesCount: 1 };

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockGenre });

			const result = await genresApi.addToSeries("series-123", "Horror");

			expect(api.post).toHaveBeenCalledWith("/series/series-123/genres", {
				name: "Horror",
			});
			expect(result).toEqual(mockGenre);
		});
	});

	describe("removeFromSeries", () => {
		it("should remove a genre from a series", async () => {
			vi.mocked(api.delete).mockResolvedValueOnce({ data: {} });

			await genresApi.removeFromSeries("series-123", "genre-1");

			expect(api.delete).toHaveBeenCalledWith(
				"/series/series-123/genres/genre-1",
			);
		});
	});

	describe("delete", () => {
		it("should delete a genre globally", async () => {
			vi.mocked(api.delete).mockResolvedValueOnce({ data: {} });

			await genresApi.delete("genre-1");

			expect(api.delete).toHaveBeenCalledWith("/genres/genre-1");
		});
	});

	describe("cleanup", () => {
		it("should cleanup unused genres", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: { deleted_count: 5 },
			});

			const result = await genresApi.cleanup();

			expect(api.post).toHaveBeenCalledWith("/genres/cleanup");
			expect(result).toEqual({ deleted_count: 5 });
		});

		it("should return 0 when no genres to cleanup", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: { deleted_count: 0 },
			});

			const result = await genresApi.cleanup();

			expect(result).toEqual({ deleted_count: 0 });
		});
	});
});
