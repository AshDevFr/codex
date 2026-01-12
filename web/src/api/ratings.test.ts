import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
	displayToStorageRating,
	ratingsApi,
	storageToDisplayRating,
} from "./ratings";
import { api } from "./client";

// Mock the api client
vi.mock("./client", () => ({
	api: {
		get: vi.fn(),
		put: vi.fn(),
		delete: vi.fn(),
	},
}));

describe("Rating conversion utilities", () => {
	describe("displayToStorageRating", () => {
		it("should convert display rating 5.0 to storage 50", () => {
			expect(displayToStorageRating(5.0)).toBe(50);
		});

		it("should convert display rating 10.0 to storage 100", () => {
			expect(displayToStorageRating(10.0)).toBe(100);
		});

		it("should convert display rating 1.0 to storage 10", () => {
			expect(displayToStorageRating(1.0)).toBe(10);
		});

		it("should handle decimal values with rounding", () => {
			expect(displayToStorageRating(7.5)).toBe(75);
			expect(displayToStorageRating(8.3)).toBe(83);
			expect(displayToStorageRating(8.35)).toBe(84); // rounds to 84
		});

		it("should handle edge case 0", () => {
			expect(displayToStorageRating(0)).toBe(0);
		});
	});

	describe("storageToDisplayRating", () => {
		it("should convert storage 50 to display 5.0", () => {
			expect(storageToDisplayRating(50)).toBe(5.0);
		});

		it("should convert storage 100 to display 10.0", () => {
			expect(storageToDisplayRating(100)).toBe(10.0);
		});

		it("should convert storage 10 to display 1.0", () => {
			expect(storageToDisplayRating(10)).toBe(1.0);
		});

		it("should handle intermediate values", () => {
			expect(storageToDisplayRating(75)).toBe(7.5);
			expect(storageToDisplayRating(83)).toBe(8.3);
		});

		it("should handle edge case 0", () => {
			expect(storageToDisplayRating(0)).toBe(0);
		});
	});

	describe("round-trip conversion", () => {
		it("should preserve value through round-trip", () => {
			const original = 7.5;
			const stored = displayToStorageRating(original);
			const recovered = storageToDisplayRating(stored);
			expect(recovered).toBe(original);
		});
	});
});

describe("ratingsApi", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	describe("getUserRating", () => {
		it("should fetch rating for a series", async () => {
			const mockRating = {
				id: "rating-1",
				seriesId: "series-123",
				userId: "user-1",
				rating: 85,
				notes: "Great series!",
				createdAt: "2024-01-01T00:00:00Z",
				updatedAt: "2024-01-01T00:00:00Z",
			};

			vi.mocked(api.get).mockResolvedValueOnce({
				data: mockRating,
				status: 200,
			});

			const result = await ratingsApi.getUserRating("series-123");

			expect(api.get).toHaveBeenCalledWith("/series/series-123/rating", {
				validateStatus: expect.any(Function),
			});
			expect(result).toEqual(mockRating);
		});

		it("should return null when no rating exists (404)", async () => {
			vi.mocked(api.get).mockResolvedValueOnce({
				data: { error: "Not found" },
				status: 404,
			});

			const result = await ratingsApi.getUserRating("series-123");

			expect(result).toBeNull();
		});

		it("should throw error for non-404 errors", async () => {
			const error = { response: { status: 500 } };
			vi.mocked(api.get).mockRejectedValueOnce(error);

			await expect(ratingsApi.getUserRating("series-123")).rejects.toEqual(
				error,
			);
		});
	});

	describe("setUserRating", () => {
		it("should set rating with notes", async () => {
			const mockRating = {
				id: "rating-1",
				seriesId: "series-123",
				userId: "user-1",
				rating: 85,
				notes: "Great series!",
				createdAt: "2024-01-01T00:00:00Z",
				updatedAt: "2024-01-01T00:00:00Z",
			};

			vi.mocked(api.put).mockResolvedValueOnce({ data: mockRating });

			const result = await ratingsApi.setUserRating("series-123", 85, "Great series!");

			expect(api.put).toHaveBeenCalledWith("/series/series-123/rating", {
				rating: 85,
				notes: "Great series!",
			});
			expect(result).toEqual(mockRating);
		});

		it("should set rating without notes", async () => {
			const mockRating = {
				id: "rating-1",
				seriesId: "series-123",
				userId: "user-1",
				rating: 75,
				notes: null,
				createdAt: "2024-01-01T00:00:00Z",
				updatedAt: "2024-01-01T00:00:00Z",
			};

			vi.mocked(api.put).mockResolvedValueOnce({ data: mockRating });

			const result = await ratingsApi.setUserRating("series-123", 75);

			expect(api.put).toHaveBeenCalledWith("/series/series-123/rating", {
				rating: 75,
				notes: undefined,
			});
			expect(result).toEqual(mockRating);
		});
	});

	describe("deleteUserRating", () => {
		it("should delete rating for a series", async () => {
			vi.mocked(api.delete).mockResolvedValueOnce({ data: {} });

			await ratingsApi.deleteUserRating("series-123");

			expect(api.delete).toHaveBeenCalledWith("/series/series-123/rating");
		});
	});

	describe("getAllUserRatings", () => {
		it("should fetch all user ratings", async () => {
			const mockResponse = {
				ratings: [
					{
						id: "rating-1",
						seriesId: "series-1",
						userId: "user-1",
						rating: 85,
						notes: null,
						createdAt: "2024-01-01T00:00:00Z",
						updatedAt: "2024-01-01T00:00:00Z",
					},
					{
						id: "rating-2",
						seriesId: "series-2",
						userId: "user-1",
						rating: 90,
						notes: "Excellent!",
						createdAt: "2024-01-02T00:00:00Z",
						updatedAt: "2024-01-02T00:00:00Z",
					},
				],
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await ratingsApi.getAllUserRatings();

			expect(api.get).toHaveBeenCalledWith("/user/ratings");
			expect(result).toEqual(mockResponse.ratings);
		});
	});
});
