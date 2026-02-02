import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { readProgressApi } from "@/api/readProgress";
import { useReaderStore } from "@/store/readerStore";

import { useReadProgress } from "./useReadProgress";

// Mock the API
vi.mock("@/api/readProgress", () => ({
  readProgressApi: {
    get: vi.fn(),
    update: vi.fn(),
  },
}));

const mockGet = vi.mocked(readProgressApi.get);
const mockUpdate = vi.mocked(readProgressApi.update);

// Helper to create a complete ReadProgressResponse
const createProgress = (
  overrides: Partial<{
    id: string;
    book_id: string;
    user_id: string;
    current_page: number;
    completed: boolean;
    completed_at: string | null;
    started_at: string;
    updated_at: string;
  }>,
) => ({
  id: "progress-123",
  book_id: "test-book",
  user_id: "user-123",
  current_page: 1,
  completed: false,
  started_at: "2024-01-01T00:00:00Z",
  updated_at: "2024-01-01T00:00:00Z",
  ...overrides,
});

describe("useReadProgress", () => {
  let queryClient: QueryClient;

  const createWrapper = () => {
    return ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );
  };

  beforeEach(() => {
    vi.clearAllMocks();

    // Reset reader store
    useReaderStore.setState({
      currentPage: 0,
      totalPages: 0,
    });

    // Create fresh query client for each test
    queryClient = new QueryClient({
      defaultOptions: {
        queries: { retry: false, gcTime: 0 },
        mutations: { retry: false },
      },
    });

    // Default mock implementations
    mockGet.mockResolvedValue(createProgress({}));

    mockUpdate.mockResolvedValue(createProgress({}));
  });

  describe("initial state", () => {
    it("should return loading state initially", () => {
      const { result } = renderHook(
        () =>
          useReadProgress({
            bookId: "test-book",
            totalPages: 100,
          }),
        { wrapper: createWrapper() },
      );

      expect(result.current.isLoading).toBe(true);
    });

    it("should fetch progress from API", async () => {
      mockGet.mockResolvedValue(createProgress({ current_page: 42 }));

      const { result } = renderHook(
        () =>
          useReadProgress({
            bookId: "test-book",
            totalPages: 100,
          }),
        { wrapper: createWrapper() },
      );

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      expect(mockGet).toHaveBeenCalledWith("test-book");
      expect(result.current.initialPage).toBe(42);
    });

    it("should return initialPage of 1 when no saved progress", async () => {
      mockGet.mockResolvedValue(null);

      const { result } = renderHook(
        () =>
          useReadProgress({
            bookId: "test-book",
            totalPages: 100,
          }),
        { wrapper: createWrapper() },
      );

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      expect(result.current.initialPage).toBe(1);
    });

    it("should clamp initialPage to totalPages", async () => {
      mockGet.mockResolvedValue(createProgress({ current_page: 150 }));

      const { result } = renderHook(
        () =>
          useReadProgress({
            bookId: "test-book",
            totalPages: 100,
          }),
        { wrapper: createWrapper() },
      );

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      expect(result.current.initialPage).toBe(100);
    });

    it("should return completed status from API", async () => {
      mockGet.mockResolvedValue(
        createProgress({ current_page: 100, completed: true }),
      );

      const { result } = renderHook(
        () =>
          useReadProgress({
            bookId: "test-book",
            totalPages: 100,
          }),
        { wrapper: createWrapper() },
      );

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      expect(result.current.isCompleted).toBe(true);
    });
  });

  describe("debounced save", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it("should debounce progress updates", async () => {
      renderHook(
        () =>
          useReadProgress({
            bookId: "test-book",
            totalPages: 100,
            debounceMs: 500,
          }),
        { wrapper: createWrapper() },
      );

      // Manually set loading to false since fake timers interfere with react-query
      await act(async () => {
        await vi.runAllTimersAsync();
      });

      // Update the page in the store
      act(() => {
        useReaderStore.setState({ currentPage: 5 });
      });

      // Should not have called update yet
      expect(mockUpdate).not.toHaveBeenCalled();

      // Advance timers past debounce
      await act(async () => {
        await vi.advanceTimersByTimeAsync(600);
      });

      // Now it should have been called
      expect(mockUpdate).toHaveBeenCalledWith("test-book", {
        current_page: 5,
        completed: false,
      });
    });

    it("should not save if page hasn't changed from last saved", async () => {
      renderHook(
        () =>
          useReadProgress({
            bookId: "test-book",
            totalPages: 100,
            debounceMs: 100,
          }),
        { wrapper: createWrapper() },
      );

      await act(async () => {
        await vi.runAllTimersAsync();
      });

      // Set page
      act(() => {
        useReaderStore.setState({ currentPage: 5 });
      });

      await act(async () => {
        await vi.advanceTimersByTimeAsync(200);
      });

      expect(mockUpdate).toHaveBeenCalledTimes(1);

      // Trigger debounce again without changing page value
      // (The hook checks if page === lastSavedPage)
      act(() => {
        useReaderStore.setState({ currentPage: 5 });
      });

      await act(async () => {
        await vi.advanceTimersByTimeAsync(200);
      });

      // Should not call again for same page
      expect(mockUpdate).toHaveBeenCalledTimes(1);
    });
  });

  describe("saveProgress (immediate)", () => {
    it("should save progress immediately without debounce", async () => {
      const { result } = renderHook(
        () =>
          useReadProgress({
            bookId: "test-book",
            totalPages: 100,
            debounceMs: 5000,
          }),
        { wrapper: createWrapper() },
      );

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      // Call saveProgress directly
      act(() => {
        result.current.saveProgress(25);
      });

      // Should be called immediately without waiting
      expect(mockUpdate).toHaveBeenCalledWith("test-book", {
        current_page: 25,
        completed: false,
      });
    });

    it("should mark as completed when saving last page", async () => {
      const { result } = renderHook(
        () =>
          useReadProgress({
            bookId: "test-book",
            totalPages: 100,
          }),
        { wrapper: createWrapper() },
      );

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      act(() => {
        result.current.saveProgress(100);
      });

      expect(mockUpdate).toHaveBeenCalledWith("test-book", {
        current_page: 100,
        completed: true,
      });
    });

    it("should not save if same page as last saved", async () => {
      const { result } = renderHook(
        () =>
          useReadProgress({
            bookId: "test-book",
            totalPages: 100,
          }),
        { wrapper: createWrapper() },
      );

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      // Save page 25
      act(() => {
        result.current.saveProgress(25);
      });

      expect(mockUpdate).toHaveBeenCalledTimes(1);

      // Try to save same page again
      act(() => {
        result.current.saveProgress(25);
      });

      // Should not call again
      expect(mockUpdate).toHaveBeenCalledTimes(1);
    });
  });

  describe("enabled option", () => {
    it("should not fetch when disabled", async () => {
      renderHook(
        () =>
          useReadProgress({
            bookId: "test-book",
            totalPages: 100,
            enabled: false,
          }),
        { wrapper: createWrapper() },
      );

      // Give it a moment
      await new Promise((r) => setTimeout(r, 50));

      expect(mockGet).not.toHaveBeenCalled();
    });

    it("should not save when disabled", async () => {
      vi.useFakeTimers();

      renderHook(
        () =>
          useReadProgress({
            bookId: "test-book",
            totalPages: 100,
            enabled: false,
          }),
        { wrapper: createWrapper() },
      );

      // Update page
      act(() => {
        useReaderStore.setState({ currentPage: 50 });
      });

      await act(async () => {
        await vi.advanceTimersByTimeAsync(2000);
      });

      expect(mockUpdate).not.toHaveBeenCalled();

      vi.useRealTimers();
    });
  });

  describe("query cache", () => {
    it("should update cache on successful save", async () => {
      mockUpdate.mockResolvedValue(createProgress({ current_page: 50 }));

      const { result } = renderHook(
        () =>
          useReadProgress({
            bookId: "test-book",
            totalPages: 100,
          }),
        { wrapper: createWrapper() },
      );

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      act(() => {
        result.current.saveProgress(50);
      });

      // Wait for the async update
      await waitFor(() => {
        const cachedData = queryClient.getQueryData([
          "readProgress",
          "test-book",
        ]);
        expect(cachedData).toEqual(createProgress({ current_page: 50 }));
      });
    });
  });

  describe("cleanup", () => {
    it("should save final progress on unmount", async () => {
      vi.useFakeTimers();

      const { unmount } = renderHook(
        () =>
          useReadProgress({
            bookId: "test-book",
            totalPages: 100,
            debounceMs: 5000,
          }),
        { wrapper: createWrapper() },
      );

      // Run timers to complete initial load
      await act(async () => {
        await vi.runAllTimersAsync();
      });

      // Set a page change that would trigger debounced save
      act(() => {
        useReaderStore.setState({ currentPage: 30 });
      });

      // Unmount before debounce fires
      unmount();

      // The save should happen on unmount with final page
      expect(mockUpdate).toHaveBeenCalledWith("test-book", {
        current_page: 30,
        completed: false,
      });

      vi.useRealTimers();
    });

    it("should not save on unmount when disabled (incognito mode)", async () => {
      vi.useFakeTimers();

      const { unmount } = renderHook(
        () =>
          useReadProgress({
            bookId: "test-book",
            totalPages: 100,
            debounceMs: 5000,
            enabled: false,
          }),
        { wrapper: createWrapper() },
      );

      // Run timers to complete initial load
      await act(async () => {
        await vi.runAllTimersAsync();
      });

      // Set a page change
      act(() => {
        useReaderStore.setState({ currentPage: 30 });
      });

      // Unmount
      unmount();

      // Should NOT have called update because enabled=false
      expect(mockUpdate).not.toHaveBeenCalled();

      vi.useRealTimers();
    });
  });
});
