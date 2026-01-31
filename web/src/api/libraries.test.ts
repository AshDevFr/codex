import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./client";
import { librariesApi } from "./libraries";

// Mock the api client
vi.mock("./client", () => ({
	api: {
		get: vi.fn(),
		post: vi.fn(),
		patch: vi.fn(),
		delete: vi.fn(),
	},
}));

describe("librariesApi", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	describe("getAll", () => {
		it("should fetch all libraries", async () => {
			const mockLibraries = [
				{ id: "lib-1", name: "Library 1" },
				{ id: "lib-2", name: "Library 2" },
			];

			vi.mocked(api.get).mockResolvedValueOnce({
				data: { data: mockLibraries },
			});

			const result = await librariesApi.getAll();

			expect(api.get).toHaveBeenCalledWith("/libraries");
			expect(result).toEqual(mockLibraries);
		});
	});

	describe("getById", () => {
		it("should fetch a library by ID", async () => {
			const mockLibrary = { id: "lib-123", name: "Test Library" };

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockLibrary });

			const result = await librariesApi.getById("lib-123");

			expect(api.get).toHaveBeenCalledWith("/libraries/lib-123");
			expect(result).toEqual(mockLibrary);
		});
	});

	describe("scan", () => {
		it("should trigger a normal scan by default", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({});

			await librariesApi.scan("lib-123");

			expect(api.post).toHaveBeenCalledWith(
				"/libraries/lib-123/scan?mode=normal",
			);
		});

		it("should trigger a deep scan when specified", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({});

			await librariesApi.scan("lib-123", "deep");

			expect(api.post).toHaveBeenCalledWith(
				"/libraries/lib-123/scan?mode=deep",
			);
		});
	});

	describe("purgeDeleted", () => {
		it("should purge deleted books from a library", async () => {
			vi.mocked(api.delete).mockResolvedValueOnce({ data: 5 });

			const result = await librariesApi.purgeDeleted("lib-123");

			expect(api.delete).toHaveBeenCalledWith(
				"/libraries/lib-123/purge-deleted",
			);
			expect(result).toBe(5);
		});
	});

	describe("generateMissingThumbnails", () => {
		it("should generate missing thumbnails for a library", async () => {
			const mockResponse = { task_id: "task-456" };

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockResponse });

			const result = await librariesApi.generateMissingThumbnails("lib-123");

			expect(api.post).toHaveBeenCalledWith(
				"/libraries/lib-123/books/thumbnails/generate",
				{ force: false },
			);
			expect(result).toEqual(mockResponse);
		});
	});

	describe("regenerateAllThumbnails", () => {
		it("should regenerate all thumbnails for a library with force flag", async () => {
			const mockResponse = { task_id: "task-789" };

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockResponse });

			const result = await librariesApi.regenerateAllThumbnails("lib-123");

			expect(api.post).toHaveBeenCalledWith(
				"/libraries/lib-123/books/thumbnails/generate",
				{ force: true },
			);
			expect(result).toEqual(mockResponse);
		});
	});

	describe("generateMissingSeriesThumbnails", () => {
		it("should generate missing series thumbnails for a library", async () => {
			const mockResponse = { task_id: "task-101" };

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockResponse });

			const result =
				await librariesApi.generateMissingSeriesThumbnails("lib-123");

			expect(api.post).toHaveBeenCalledWith(
				"/libraries/lib-123/series/thumbnails/generate",
				{ force: false },
			);
			expect(result).toEqual(mockResponse);
		});
	});

	describe("regenerateAllSeriesThumbnails", () => {
		it("should regenerate all series thumbnails for a library with force flag", async () => {
			const mockResponse = { task_id: "task-202" };

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockResponse });

			const result =
				await librariesApi.regenerateAllSeriesThumbnails("lib-123");

			expect(api.post).toHaveBeenCalledWith(
				"/libraries/lib-123/series/thumbnails/generate",
				{ force: true },
			);
			expect(result).toEqual(mockResponse);
		});
	});
});
