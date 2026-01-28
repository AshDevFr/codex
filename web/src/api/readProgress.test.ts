import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./client";
import { readProgressApi } from "./readProgress";

vi.mock("./client", () => ({
	api: {
		get: vi.fn(),
		put: vi.fn(),
		delete: vi.fn(),
	},
}));

describe("readProgressApi", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	describe("get", () => {
		it("should fetch reading progress for a book", async () => {
			const mockProgress = {
				id: "progress-123",
				book_id: "book-123",
				current_page: 42,
				completed: false,
				started_at: "2024-01-01T00:00:00Z",
			};
			vi.mocked(api.get).mockResolvedValueOnce({ data: mockProgress });

			const result = await readProgressApi.get("book-123");

			expect(api.get).toHaveBeenCalledWith("/books/book-123/progress");
			expect(result).toEqual(mockProgress);
		});

		it("should return null when no progress exists (404)", async () => {
			const error = { response: { status: 404 } };
			vi.mocked(api.get).mockRejectedValueOnce(error);

			const result = await readProgressApi.get("book-123");

			expect(result).toBeNull();
		});

		it("should throw on other errors", async () => {
			const error = { response: { status: 500 }, message: "Server error" };
			vi.mocked(api.get).mockRejectedValueOnce(error);

			await expect(readProgressApi.get("book-123")).rejects.toEqual(error);
		});
	});

	describe("update", () => {
		it("should update reading progress", async () => {
			const mockProgress = {
				id: "progress-123",
				book_id: "book-123",
				current_page: 50,
				completed: false,
				started_at: "2024-01-01T00:00:00Z",
			};
			vi.mocked(api.put).mockResolvedValueOnce({ data: mockProgress });

			const result = await readProgressApi.update("book-123", {
				current_page: 50,
			});

			expect(api.put).toHaveBeenCalledWith("/books/book-123/progress", {
				current_page: 50,
			});
			expect(result).toEqual(mockProgress);
		});

		it("should mark book as completed", async () => {
			const mockProgress = {
				id: "progress-123",
				book_id: "book-123",
				current_page: 100,
				completed: true,
				completed_at: "2024-01-15T10:00:00Z",
				started_at: "2024-01-01T00:00:00Z",
			};
			vi.mocked(api.put).mockResolvedValueOnce({ data: mockProgress });

			const result = await readProgressApi.update("book-123", {
				current_page: 100,
				completed: true,
			});

			expect(api.put).toHaveBeenCalledWith("/books/book-123/progress", {
				current_page: 100,
				completed: true,
			});
			expect(result).toEqual(mockProgress);
		});

		it("should update progress with percentage for EPUB books", async () => {
			const mockProgress = {
				id: "progress-123",
				book_id: "book-123",
				current_page: 45,
				progress_percentage: 0.45,
				completed: false,
				started_at: "2024-01-01T00:00:00Z",
			};
			vi.mocked(api.put).mockResolvedValueOnce({ data: mockProgress });

			const result = await readProgressApi.update("book-123", {
				current_page: 45,
				progress_percentage: 0.45,
			});

			expect(api.put).toHaveBeenCalledWith("/books/book-123/progress", {
				current_page: 45,
				progress_percentage: 0.45,
			});
			expect(result).toEqual(mockProgress);
		});
	});

	describe("delete", () => {
		it("should delete reading progress", async () => {
			vi.mocked(api.delete).mockResolvedValueOnce({});

			await readProgressApi.delete("book-123");

			expect(api.delete).toHaveBeenCalledWith("/books/book-123/progress");
		});
	});
});
