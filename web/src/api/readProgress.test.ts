import { IDBFactory } from "fake-indexeddb";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { _resetForTests, getOutbox, setDbContext } from "@/lib/offline/db";
import { isOfflineQueuedError, OfflineQueuedError } from "@/lib/offline/outbox";
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
    setDbContext({ indexedDB: new IDBFactory() });
  });

  afterEach(() => {
    setDbContext(null);
    _resetForTests();
  });

  describe("get", () => {
    it("should fetch reading progress for a book", async () => {
      const mockProgress = {
        id: "progress-123",
        bookId: "book-123",
        currentPage: 42,
        completed: false,
        startedAt: "2024-01-01T00:00:00Z",
      };
      vi.mocked(api.get).mockResolvedValueOnce({ data: mockProgress });

      const result = await readProgressApi.get("book-123");

      expect(api.get).toHaveBeenCalledWith("/books/book-123/progress");
      expect(result).toEqual(mockProgress);
    });

    it("should return null when no progress exists", async () => {
      vi.mocked(api.get).mockResolvedValueOnce({ data: null });

      const result = await readProgressApi.get("book-123");

      expect(api.get).toHaveBeenCalledWith("/books/book-123/progress");
      expect(result).toBeNull();
    });
  });

  describe("update", () => {
    it("should update reading progress", async () => {
      const mockProgress = {
        id: "progress-123",
        bookId: "book-123",
        currentPage: 50,
        completed: false,
        startedAt: "2024-01-01T00:00:00Z",
      };
      vi.mocked(api.put).mockResolvedValueOnce({ data: mockProgress });

      const result = await readProgressApi.update("book-123", {
        currentPage: 50,
      });

      expect(api.put).toHaveBeenCalledWith("/books/book-123/progress", {
        currentPage: 50,
      });
      expect(result).toEqual(mockProgress);
    });

    it("should mark book as completed", async () => {
      const mockProgress = {
        id: "progress-123",
        bookId: "book-123",
        currentPage: 100,
        completed: true,
        completedAt: "2024-01-15T10:00:00Z",
        startedAt: "2024-01-01T00:00:00Z",
      };
      vi.mocked(api.put).mockResolvedValueOnce({ data: mockProgress });

      const result = await readProgressApi.update("book-123", {
        currentPage: 100,
        completed: true,
      });

      expect(api.put).toHaveBeenCalledWith("/books/book-123/progress", {
        currentPage: 100,
        completed: true,
      });
      expect(result).toEqual(mockProgress);
    });

    it("should update progress with percentage for EPUB books", async () => {
      const mockProgress = {
        id: "progress-123",
        bookId: "book-123",
        currentPage: 45,
        progressPercentage: 0.45,
        completed: false,
        startedAt: "2024-01-01T00:00:00Z",
      };
      vi.mocked(api.put).mockResolvedValueOnce({ data: mockProgress });

      const result = await readProgressApi.update("book-123", {
        currentPage: 45,
        progressPercentage: 0.45,
      });

      expect(api.put).toHaveBeenCalledWith("/books/book-123/progress", {
        currentPage: 45,
        progressPercentage: 0.45,
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

  describe("offline outbox integration", () => {
    it("update throws OfflineQueuedError and enqueues on network failure", async () => {
      vi.mocked(api.put).mockRejectedValueOnce({
        error: "Network Error",
        message: "offline",
      });

      const request = { currentPage: 42, completed: false };
      await expect(
        readProgressApi.update("book-123", request),
      ).rejects.toSatisfy(isOfflineQueuedError);

      const queued = await getOutbox();
      expect(queued).toHaveLength(1);
      expect(queued[0]?.request.url).toBe("/api/v1/books/book-123/progress");
      expect(queued[0]?.request.method).toBe("PUT");
      expect(queued[0]?.request.body).toBe(JSON.stringify(request));
    });

    it("update rethrows non-network errors without queueing", async () => {
      vi.mocked(api.put).mockRejectedValueOnce({
        error: "Internal Server Error",
        message: "server died",
      });

      await expect(
        readProgressApi.update("book-123", { currentPage: 1 }),
      ).rejects.not.toSatisfy(isOfflineQueuedError);

      expect(await getOutbox()).toEqual([]);
    });

    it("updateProgression enqueues on network failure", async () => {
      vi.mocked(api.put).mockRejectedValueOnce({ error: "Network Error" });
      await expect(
        readProgressApi.updateProgression("book-123", {
          device: { id: "d", name: "n" },
          locator: {
            href: "ch1",
            locations: { totalProgression: 0.5 },
            type: "application/xhtml+xml",
          },
          modified: "2024-01-01T00:00:00Z",
        }),
      ).rejects.toBeInstanceOf(OfflineQueuedError);
      const queued = await getOutbox();
      expect(queued[0]?.request.url).toBe("/api/v1/books/book-123/progression");
    });

    it("delete enqueues on network failure", async () => {
      vi.mocked(api.delete).mockRejectedValueOnce({ error: "Network Error" });
      await expect(readProgressApi.delete("book-123")).rejects.toBeInstanceOf(
        OfflineQueuedError,
      );
      const queued = await getOutbox();
      expect(queued[0]?.request.method).toBe("DELETE");
    });
  });
});
