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
		it("should fetch all genres", async () => {
			const mockGenres = [
				{ id: "genre-1", name: "Action", seriesCount: 10 },
				{ id: "genre-2", name: "Comedy", seriesCount: 5 },
				{ id: "genre-3", name: "Drama", seriesCount: 8 },
			];
			const mockResponse = {
				data: mockGenres,
				page: 1,
				pageSize: 50,
				total: 3,
				totalPages: 1,
				links: {
					self: "/api/v1/genres?page=1&page_size=50",
					first: "/api/v1/genres?page=1&page_size=50",
					last: "/api/v1/genres?page=1&page_size=50",
				},
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await genresApi.getAll();

			expect(api.get).toHaveBeenCalledWith("/genres");
			expect(result).toEqual(mockGenres);
		});

		it("should return empty array when no genres exist", async () => {
			vi.mocked(api.get).mockResolvedValueOnce({
				data: {
					data: [],
					page: 1,
					pageSize: 50,
					total: 0,
					totalPages: 0,
					links: {
						self: "/api/v1/genres?page=1&page_size=50",
						first: "/api/v1/genres?page=1&page_size=50",
						last: "/api/v1/genres?page=1&page_size=50",
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
