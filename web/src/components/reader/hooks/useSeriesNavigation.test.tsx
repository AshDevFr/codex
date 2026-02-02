import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useReaderStore } from "@/store/readerStore";
import { useSeriesNavigation } from "./useSeriesNavigation";

// Mock useNavigate
const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return {
    ...actual,
    useNavigate: () => mockNavigate,
  };
});

function wrapper({ children }: { children: ReactNode }) {
  return <MemoryRouter>{children}</MemoryRouter>;
}

describe("useSeriesNavigation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useReaderStore.getState().resetSession();
  });

  describe("canGoPrevBook and canGoNextBook", () => {
    it("should return false when no adjacent books", () => {
      const { result } = renderHook(() => useSeriesNavigation(), { wrapper });

      expect(result.current.canGoPrevBook).toBe(false);
      expect(result.current.canGoNextBook).toBe(false);
    });

    it("should return true when adjacent books exist", () => {
      // Set up adjacent books in store
      act(() => {
        useReaderStore.getState().setAdjacentBooks({
          prev: { id: "book-0", title: "Prev Book", pageCount: 50 },
          next: { id: "book-2", title: "Next Book", pageCount: 100 },
        });
      });

      const { result } = renderHook(() => useSeriesNavigation(), { wrapper });

      expect(result.current.canGoPrevBook).toBe(true);
      expect(result.current.canGoNextBook).toBe(true);
    });

    it("should return true for prev only when next is null", () => {
      act(() => {
        useReaderStore.getState().setAdjacentBooks({
          prev: { id: "book-0", title: "Prev Book", pageCount: 50 },
          next: null,
        });
      });

      const { result } = renderHook(() => useSeriesNavigation(), { wrapper });

      expect(result.current.canGoPrevBook).toBe(true);
      expect(result.current.canGoNextBook).toBe(false);
    });
  });

  describe("goToPrevBook", () => {
    it("should navigate to previous book at its last page", () => {
      act(() => {
        useReaderStore.getState().setAdjacentBooks({
          prev: { id: "book-0", title: "Prev Book", pageCount: 50 },
          next: null,
        });
      });

      const { result } = renderHook(() => useSeriesNavigation(), { wrapper });

      act(() => {
        result.current.goToPrevBook();
      });

      expect(mockNavigate).toHaveBeenCalledWith("/reader/book-0?page=50");
    });

    it("should clear boundary state when navigating", () => {
      act(() => {
        useReaderStore.getState().setAdjacentBooks({
          prev: { id: "book-0", title: "Prev Book", pageCount: 50 },
          next: null,
        });
        useReaderStore.getState().setBoundaryState("at-start");
      });

      const { result } = renderHook(() => useSeriesNavigation(), { wrapper });

      act(() => {
        result.current.goToPrevBook();
      });

      expect(useReaderStore.getState().boundaryState).toBe("none");
    });
  });

  describe("goToNextBook", () => {
    it("should navigate to next book at page 1", () => {
      act(() => {
        useReaderStore.getState().setAdjacentBooks({
          prev: null,
          next: { id: "book-2", title: "Next Book", pageCount: 100 },
        });
      });

      const { result } = renderHook(() => useSeriesNavigation(), { wrapper });

      act(() => {
        result.current.goToNextBook();
      });

      expect(mockNavigate).toHaveBeenCalledWith("/reader/book-2?page=1");
    });
  });

  describe("handleNextPage", () => {
    it("should go to next page when not at last page", () => {
      act(() => {
        useReaderStore.getState().initializeReader("book-1", 10, 5);
      });

      const { result } = renderHook(() => useSeriesNavigation(), { wrapper });

      act(() => {
        result.current.handleNextPage();
      });

      expect(useReaderStore.getState().currentPage).toBe(6);
      expect(useReaderStore.getState().boundaryState).toBe("none");
    });

    it("should set boundary state on first press at last page with next book", () => {
      const onBoundaryChange = vi.fn();

      act(() => {
        useReaderStore.getState().initializeReader("book-1", 10, 10);
        useReaderStore.getState().setAdjacentBooks({
          prev: null,
          next: { id: "book-2", title: "Next Book", pageCount: 100 },
        });
      });

      const { result } = renderHook(
        () => useSeriesNavigation({ onBoundaryChange }),
        { wrapper },
      );

      act(() => {
        result.current.handleNextPage();
      });

      expect(useReaderStore.getState().boundaryState).toBe("at-end");
      expect(onBoundaryChange).toHaveBeenCalledWith(
        "at-end",
        'End of book. Press again to continue to "Next Book"',
      );
      expect(mockNavigate).not.toHaveBeenCalled();
    });

    it("should navigate to next book on second press at last page", () => {
      act(() => {
        useReaderStore.getState().initializeReader("book-1", 10, 10);
        useReaderStore.getState().setAdjacentBooks({
          prev: null,
          next: { id: "book-2", title: "Next Book", pageCount: 100 },
        });
        useReaderStore.getState().setBoundaryState("at-end");
      });

      const { result } = renderHook(() => useSeriesNavigation(), { wrapper });

      act(() => {
        result.current.handleNextPage();
      });

      expect(mockNavigate).toHaveBeenCalledWith("/reader/book-2?page=1");
    });

    it("should show 'end of series' message when no next book", () => {
      const onBoundaryChange = vi.fn();

      act(() => {
        useReaderStore.getState().initializeReader("book-1", 10, 10);
        useReaderStore.getState().setAdjacentBooks({
          prev: { id: "book-0", title: "Prev Book", pageCount: 50 },
          next: null,
        });
      });

      const { result } = renderHook(
        () => useSeriesNavigation({ onBoundaryChange }),
        { wrapper },
      );

      act(() => {
        result.current.handleNextPage();
      });

      expect(onBoundaryChange).toHaveBeenCalledWith(
        "at-end",
        "You have reached the end of the series",
      );
    });
  });

  describe("handlePrevPage", () => {
    it("should go to previous page when not at first page", () => {
      act(() => {
        useReaderStore.getState().initializeReader("book-1", 10, 5);
      });

      const { result } = renderHook(() => useSeriesNavigation(), { wrapper });

      act(() => {
        result.current.handlePrevPage();
      });

      expect(useReaderStore.getState().currentPage).toBe(4);
      expect(useReaderStore.getState().boundaryState).toBe("none");
    });

    it("should set boundary state on first press at first page with prev book", () => {
      const onBoundaryChange = vi.fn();

      act(() => {
        useReaderStore.getState().initializeReader("book-1", 10, 1);
        useReaderStore.getState().setAdjacentBooks({
          prev: { id: "book-0", title: "Prev Book", pageCount: 50 },
          next: null,
        });
      });

      const { result } = renderHook(
        () => useSeriesNavigation({ onBoundaryChange }),
        { wrapper },
      );

      act(() => {
        result.current.handlePrevPage();
      });

      expect(useReaderStore.getState().boundaryState).toBe("at-start");
      expect(onBoundaryChange).toHaveBeenCalledWith(
        "at-start",
        'Beginning of book. Press again to go to "Prev Book"',
      );
      expect(mockNavigate).not.toHaveBeenCalled();
    });

    it("should navigate to prev book at last page on second press", () => {
      act(() => {
        useReaderStore.getState().initializeReader("book-1", 10, 1);
        useReaderStore.getState().setAdjacentBooks({
          prev: { id: "book-0", title: "Prev Book", pageCount: 50 },
          next: null,
        });
        useReaderStore.getState().setBoundaryState("at-start");
      });

      const { result } = renderHook(() => useSeriesNavigation(), { wrapper });

      act(() => {
        result.current.handlePrevPage();
      });

      expect(mockNavigate).toHaveBeenCalledWith("/reader/book-0?page=50");
    });

    it("should show 'beginning of series' message when no prev book", () => {
      const onBoundaryChange = vi.fn();

      act(() => {
        useReaderStore.getState().initializeReader("book-1", 10, 1);
        useReaderStore.getState().setAdjacentBooks({
          prev: null,
          next: { id: "book-2", title: "Next Book", pageCount: 100 },
        });
      });

      const { result } = renderHook(
        () => useSeriesNavigation({ onBoundaryChange }),
        { wrapper },
      );

      act(() => {
        result.current.handlePrevPage();
      });

      expect(onBoundaryChange).toHaveBeenCalledWith(
        "at-start",
        "You are at the beginning of the series",
      );
    });
  });

  describe("boundaryMessage", () => {
    it("should return null when boundary state is none", () => {
      const { result } = renderHook(() => useSeriesNavigation(), { wrapper });
      expect(result.current.boundaryMessage).toBeNull();
    });

    it("should return message when at-end with next book", () => {
      act(() => {
        useReaderStore.getState().setAdjacentBooks({
          prev: null,
          next: { id: "book-2", title: "Next Book", pageCount: 100 },
        });
        useReaderStore.getState().setBoundaryState("at-end");
      });

      const { result } = renderHook(() => useSeriesNavigation(), { wrapper });
      expect(result.current.boundaryMessage).toBe(
        'End of book. Press again to continue to "Next Book"',
      );
    });

    it("should return message when at-start with prev book", () => {
      act(() => {
        useReaderStore.getState().setAdjacentBooks({
          prev: { id: "book-0", title: "Prev Book", pageCount: 50 },
          next: null,
        });
        useReaderStore.getState().setBoundaryState("at-start");
      });

      const { result } = renderHook(() => useSeriesNavigation(), { wrapper });
      expect(result.current.boundaryMessage).toBe(
        'Beginning of book. Press again to go to "Prev Book"',
      );
    });
  });

  describe("clearing boundary state", () => {
    it("should clear boundary state when navigating away from boundary", () => {
      act(() => {
        useReaderStore.getState().initializeReader("book-1", 10, 2);
        useReaderStore.getState().setBoundaryState("at-start");
      });

      const { result } = renderHook(() => useSeriesNavigation(), { wrapper });

      act(() => {
        result.current.handlePrevPage();
      });

      expect(useReaderStore.getState().currentPage).toBe(1);
      expect(useReaderStore.getState().boundaryState).toBe("none");
    });
  });

  describe("auto-advance to next book", () => {
    it("should immediately navigate to next book when auto-advance is enabled", () => {
      const onBoundaryChange = vi.fn();

      act(() => {
        useReaderStore.getState().initializeReader("book-1", 10, 10);
        useReaderStore.getState().setAdjacentBooks({
          prev: null,
          next: { id: "book-2", title: "Next Book", pageCount: 100 },
        });
        useReaderStore.getState().setAutoAdvanceToNextBook(true);
      });

      const { result } = renderHook(
        () => useSeriesNavigation({ onBoundaryChange }),
        { wrapper },
      );

      act(() => {
        result.current.handleNextPage();
      });

      // Should navigate immediately without requiring second press
      expect(mockNavigate).toHaveBeenCalledWith("/reader/book-2?page=1");
      expect(onBoundaryChange).toHaveBeenCalledWith(
        "at-end",
        'Continuing to "Next Book"...',
      );
    });

    it("should immediately navigate to prev book when auto-advance is enabled", () => {
      const onBoundaryChange = vi.fn();

      act(() => {
        useReaderStore.getState().initializeReader("book-1", 10, 1);
        useReaderStore.getState().setAdjacentBooks({
          prev: { id: "book-0", title: "Prev Book", pageCount: 50 },
          next: null,
        });
        useReaderStore.getState().setAutoAdvanceToNextBook(true);
      });

      const { result } = renderHook(
        () => useSeriesNavigation({ onBoundaryChange }),
        { wrapper },
      );

      act(() => {
        result.current.handlePrevPage();
      });

      // Should navigate immediately to prev book at last page
      expect(mockNavigate).toHaveBeenCalledWith("/reader/book-0?page=50");
      expect(onBoundaryChange).toHaveBeenCalledWith(
        "at-start",
        'Going back to "Prev Book"...',
      );
    });

    it("should not auto-advance when there is no next book", () => {
      const onBoundaryChange = vi.fn();

      act(() => {
        useReaderStore.getState().initializeReader("book-1", 10, 10);
        useReaderStore.getState().setAdjacentBooks({
          prev: { id: "book-0", title: "Prev Book", pageCount: 50 },
          next: null,
        });
        useReaderStore.getState().setAutoAdvanceToNextBook(true);
      });

      const { result } = renderHook(
        () => useSeriesNavigation({ onBoundaryChange }),
        { wrapper },
      );

      act(() => {
        result.current.handleNextPage();
      });

      // Should show end of series message, not navigate
      expect(mockNavigate).not.toHaveBeenCalled();
      expect(onBoundaryChange).toHaveBeenCalledWith(
        "at-end",
        "You have reached the end of the series",
      );
    });

    it("should require two presses when auto-advance is disabled", () => {
      const onBoundaryChange = vi.fn();

      act(() => {
        useReaderStore.getState().initializeReader("book-1", 10, 10);
        useReaderStore.getState().setAdjacentBooks({
          prev: null,
          next: { id: "book-2", title: "Next Book", pageCount: 100 },
        });
        useReaderStore.getState().setAutoAdvanceToNextBook(false);
      });

      const { result } = renderHook(
        () => useSeriesNavigation({ onBoundaryChange }),
        { wrapper },
      );

      // First press - should set boundary state
      act(() => {
        result.current.handleNextPage();
      });

      expect(mockNavigate).not.toHaveBeenCalled();
      expect(useReaderStore.getState().boundaryState).toBe("at-end");

      // Second press - should navigate
      act(() => {
        result.current.handleNextPage();
      });

      expect(mockNavigate).toHaveBeenCalledWith("/reader/book-2?page=1");
    });
  });
});
