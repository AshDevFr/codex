import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useEpubProgress } from "./useEpubProgress";

// Mock the readProgressApi
vi.mock("@/api/readProgress", () => ({
  readProgressApi: {
    update: vi.fn().mockResolvedValue({}),
    updateProgression: vi.fn().mockResolvedValue(undefined),
    get: vi.fn().mockResolvedValue(null),
    getProgression: vi.fn().mockResolvedValue(null),
  },
}));

describe("useEpubProgress", () => {
  const mockBookId = "test-book-123";
  const mockCfi = "epubcfi(/6/4[chap01]!/4/2/8/1:0)";
  const mockCfi2 = "epubcfi(/6/6[chap02]!/4/2/10/1:0)";
  const mockPercentage = 0.25;
  const mockPercentage2 = 0.5;

  let queryClient: QueryClient;

  // Wrapper with QueryClientProvider
  const createWrapper = () => {
    return ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );
  };

  beforeEach(() => {
    localStorage.clear();
    vi.useFakeTimers();
    queryClient = new QueryClient({
      defaultOptions: {
        queries: { retry: false },
        mutations: { retry: false },
      },
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    queryClient.clear();
  });

  describe("getSavedLocation", () => {
    it("returns null when no progress is saved", () => {
      const { result } = renderHook(
        () => useEpubProgress({ bookId: mockBookId, totalPages: 100 }),
        { wrapper: createWrapper() },
      );

      expect(result.current.getSavedLocation()).toBeNull();
    });

    it("returns saved CFI from localStorage", () => {
      localStorage.setItem(`epub-cfi-${mockBookId}`, mockCfi);

      const { result } = renderHook(
        () => useEpubProgress({ bookId: mockBookId, totalPages: 100 }),
        { wrapper: createWrapper() },
      );

      expect(result.current.getSavedLocation()).toBe(mockCfi);
    });

    it("returns null when disabled", () => {
      localStorage.setItem(`epub-cfi-${mockBookId}`, mockCfi);

      const { result } = renderHook(
        () =>
          useEpubProgress({
            bookId: mockBookId,
            totalPages: 100,
            enabled: false,
          }),
        { wrapper: createWrapper() },
      );

      expect(result.current.getSavedLocation()).toBeNull();
    });
  });

  describe("saveLocation", () => {
    it("saves CFI to localStorage after debounce", () => {
      const { result } = renderHook(
        () =>
          useEpubProgress({
            bookId: mockBookId,
            totalPages: 100,
            debounceMs: 1000,
          }),
        { wrapper: createWrapper() },
      );

      act(() => {
        result.current.saveLocation(mockCfi, mockPercentage, "chapter1.xhtml");
      });

      // Should not be saved yet (debounce)
      expect(localStorage.getItem(`epub-cfi-${mockBookId}`)).toBeNull();

      // Fast-forward past debounce
      act(() => {
        vi.advanceTimersByTime(1000);
      });

      // Now it should be saved
      expect(localStorage.getItem(`epub-cfi-${mockBookId}`)).toBe(mockCfi);
    });

    it("does not save when disabled", () => {
      const { result } = renderHook(
        () =>
          useEpubProgress({
            bookId: mockBookId,
            totalPages: 100,
            enabled: false,
          }),
        { wrapper: createWrapper() },
      );

      act(() => {
        result.current.saveLocation(mockCfi, mockPercentage, "chapter1.xhtml");
        vi.advanceTimersByTime(2000);
      });

      expect(localStorage.getItem(`epub-cfi-${mockBookId}`)).toBeNull();
    });

    it("debounces multiple rapid saves", () => {
      const { result } = renderHook(
        () =>
          useEpubProgress({
            bookId: mockBookId,
            totalPages: 100,
            debounceMs: 1000,
          }),
        { wrapper: createWrapper() },
      );

      act(() => {
        result.current.saveLocation(mockCfi, mockPercentage, "chapter1.xhtml");
      });

      act(() => {
        vi.advanceTimersByTime(500);
      });

      act(() => {
        result.current.saveLocation(
          mockCfi2,
          mockPercentage2,
          "chapter2.xhtml",
        );
      });

      act(() => {
        vi.advanceTimersByTime(500);
      });

      // First save should have been cancelled
      expect(localStorage.getItem(`epub-cfi-${mockBookId}`)).toBeNull();

      act(() => {
        vi.advanceTimersByTime(500);
      });

      // Second save should complete
      expect(localStorage.getItem(`epub-cfi-${mockBookId}`)).toBe(mockCfi2);
    });

    it("skips saving if CFI and percentage are same as last saved", () => {
      const { result } = renderHook(
        () =>
          useEpubProgress({
            bookId: mockBookId,
            totalPages: 100,
            debounceMs: 1000,
          }),
        { wrapper: createWrapper() },
      );

      // Save first CFI
      act(() => {
        result.current.saveLocation(mockCfi, mockPercentage, "chapter1.xhtml");
        vi.advanceTimersByTime(1000);
      });

      expect(localStorage.getItem(`epub-cfi-${mockBookId}`)).toBe(mockCfi);

      // Try to save same CFI and percentage again
      const setItemSpy = vi.spyOn(Storage.prototype, "setItem");
      setItemSpy.mockClear();

      act(() => {
        result.current.saveLocation(mockCfi, mockPercentage, "chapter1.xhtml");
        vi.advanceTimersByTime(1000);
      });

      // Should not have called setItem again
      expect(setItemSpy).not.toHaveBeenCalled();

      setItemSpy.mockRestore();
    });

    it("skips saving if percentage change is below threshold", () => {
      const { result } = renderHook(
        () =>
          useEpubProgress({
            bookId: mockBookId,
            totalPages: 100,
            debounceMs: 1000,
          }),
        { wrapper: createWrapper() },
      );

      // Save first location
      act(() => {
        result.current.saveLocation(mockCfi, 0.25, "chapter1.xhtml");
        vi.advanceTimersByTime(1000);
      });

      expect(localStorage.getItem(`epub-cfi-${mockBookId}`)).toBe(mockCfi);

      // Try to save with tiny percentage change (below 0.5% threshold)
      const setItemSpy = vi.spyOn(Storage.prototype, "setItem");
      setItemSpy.mockClear();

      act(() => {
        // Same CFI, tiny percentage change (0.001 = 0.1%, below 0.5% threshold)
        result.current.saveLocation(mockCfi, 0.251, "chapter1.xhtml");
        vi.advanceTimersByTime(1000);
      });

      // Should not have called setItem again (no meaningful change)
      expect(setItemSpy).not.toHaveBeenCalled();

      setItemSpy.mockRestore();
    });
  });

  describe("clearProgress", () => {
    it("removes saved progress from localStorage", () => {
      localStorage.setItem(`epub-cfi-${mockBookId}`, mockCfi);

      const { result } = renderHook(
        () => useEpubProgress({ bookId: mockBookId, totalPages: 100 }),
        { wrapper: createWrapper() },
      );

      act(() => {
        result.current.clearProgress();
      });

      expect(localStorage.getItem(`epub-cfi-${mockBookId}`)).toBeNull();
    });
  });

  describe("unmount behavior", () => {
    it("saves pending progress on unmount", () => {
      const { result, unmount } = renderHook(
        () =>
          useEpubProgress({
            bookId: mockBookId,
            totalPages: 100,
            debounceMs: 5000,
          }),
        { wrapper: createWrapper() },
      );

      act(() => {
        result.current.saveLocation(mockCfi, mockPercentage, "chapter1.xhtml");
      });

      // Progress should not be saved yet (debounce is 5s)
      expect(localStorage.getItem(`epub-cfi-${mockBookId}`)).toBeNull();

      // Unmount should flush pending save
      unmount();

      expect(localStorage.getItem(`epub-cfi-${mockBookId}`)).toBe(mockCfi);
    });

    it("does not save on unmount if no pending changes", () => {
      localStorage.setItem(`epub-cfi-${mockBookId}`, mockCfi);

      const { result, unmount } = renderHook(
        () => useEpubProgress({ bookId: mockBookId, totalPages: 100 }),
        { wrapper: createWrapper() },
      );

      // Read the saved location but don't save anything new
      result.current.getSavedLocation();

      const setItemSpy = vi.spyOn(Storage.prototype, "setItem");

      unmount();

      expect(setItemSpy).not.toHaveBeenCalled();

      setItemSpy.mockRestore();
    });

    it("does not save on unmount when disabled (incognito mode)", () => {
      const { result, unmount } = renderHook(
        () =>
          useEpubProgress({
            bookId: mockBookId,
            totalPages: 100,
            debounceMs: 5000,
            enabled: false,
          }),
        { wrapper: createWrapper() },
      );

      // Try to save a location (will be ignored due to enabled=false)
      act(() => {
        result.current.saveLocation(mockCfi, mockPercentage, "chapter1.xhtml");
      });

      const setItemSpy = vi.spyOn(Storage.prototype, "setItem");

      // Unmount - should NOT flush any pending saves when disabled
      unmount();

      expect(setItemSpy).not.toHaveBeenCalled();
      expect(localStorage.getItem(`epub-cfi-${mockBookId}`)).toBeNull();

      setItemSpy.mockRestore();
    });
  });

  describe("different book IDs", () => {
    it("uses separate storage keys for different books", () => {
      const bookId1 = "book-1";
      const bookId2 = "book-2";

      const { result: result1 } = renderHook(
        () => useEpubProgress({ bookId: bookId1, totalPages: 100 }),
        { wrapper: createWrapper() },
      );
      const { result: result2 } = renderHook(
        () => useEpubProgress({ bookId: bookId2, totalPages: 100 }),
        { wrapper: createWrapper() },
      );

      act(() => {
        result1.current.saveLocation(mockCfi, mockPercentage, "chapter1.xhtml");
        result2.current.saveLocation(
          mockCfi2,
          mockPercentage2,
          "chapter2.xhtml",
        );
        vi.advanceTimersByTime(2000);
      });

      expect(localStorage.getItem(`epub-cfi-${bookId1}`)).toBe(mockCfi);
      expect(localStorage.getItem(`epub-cfi-${bookId2}`)).toBe(mockCfi2);
    });
  });
});
