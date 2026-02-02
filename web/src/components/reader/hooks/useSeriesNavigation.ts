import { useCallback } from "react";
import { useNavigate } from "react-router-dom";
import {
  selectIsFirstPage,
  selectIsLastPage,
  useReaderStore,
} from "@/store/readerStore";

export interface SeriesNavigationResult {
  /** Whether we can navigate to a previous book in the series */
  canGoPrevBook: boolean;
  /** Whether we can navigate to a next book in the series */
  canGoNextBook: boolean;
  /** Navigate to the previous book at its last page */
  goToPrevBook: () => void;
  /** Navigate to the next book at page 1 */
  goToNextBook: () => void;
  /** Attempt to go to the next page, handling boundary state */
  handleNextPage: () => void;
  /** Attempt to go to the previous page, handling boundary state */
  handlePrevPage: () => void;
  /** Current boundary state */
  boundaryState: "none" | "at-start" | "at-end";
  /** Message to display when at boundary (if any) */
  boundaryMessage: string | null;
}

export interface UseSeriesNavigationOptions {
  /** Callback when boundary state changes (for showing notifications) */
  onBoundaryChange?: (
    state: "none" | "at-start" | "at-end",
    message: string | null,
  ) => void;
}

/**
 * Hook that provides series navigation functionality with boundary detection.
 *
 * When at the end of a book and the user tries to go forward:
 * - If auto-advance is enabled: Navigates directly to next book
 * - Otherwise, first attempt: Sets boundary state to 'at-end' and returns a message
 * - Second attempt: Navigates to next book at page 1
 *
 * Same logic applies to the beginning (at-start -> previous book at last page)
 */
export function useSeriesNavigation(
  options: UseSeriesNavigationOptions = {},
): SeriesNavigationResult {
  const { onBoundaryChange } = options;
  const navigate = useNavigate();

  // Store state
  const adjacentBooks = useReaderStore((state) => state.adjacentBooks);
  const boundaryState = useReaderStore((state) => state.boundaryState);
  const autoAdvance = useReaderStore(
    (state) => state.settings.autoAdvanceToNextBook,
  );
  const isFirstPage = useReaderStore(selectIsFirstPage);
  const isLastPage = useReaderStore(selectIsLastPage);

  // Store actions
  const nextPage = useReaderStore((state) => state.nextPage);
  const prevPage = useReaderStore((state) => state.prevPage);
  const setBoundaryState = useReaderStore((state) => state.setBoundaryState);
  const clearBoundaryState = useReaderStore(
    (state) => state.clearBoundaryState,
  );

  const canGoPrevBook = adjacentBooks?.prev != null;
  const canGoNextBook = adjacentBooks?.next != null;

  // Navigate to previous book at its last page
  const goToPrevBook = useCallback(() => {
    const prevBook = adjacentBooks?.prev;
    if (prevBook) {
      clearBoundaryState();
      navigate(`/reader/${prevBook.id}?page=${prevBook.pageCount}`);
    }
  }, [adjacentBooks?.prev, navigate, clearBoundaryState]);

  // Navigate to next book at page 1
  const goToNextBook = useCallback(() => {
    const nextBook = adjacentBooks?.next;
    if (nextBook) {
      clearBoundaryState();
      navigate(`/reader/${nextBook.id}?page=1`);
    }
  }, [adjacentBooks?.next, navigate, clearBoundaryState]);

  // Handle next page with boundary detection
  const handleNextPage = useCallback(() => {
    if (!isLastPage) {
      // Not at boundary, just go to next page
      if (boundaryState !== "none") {
        clearBoundaryState();
      }
      nextPage();
      return;
    }

    // At the last page
    if (autoAdvance && canGoNextBook) {
      // Auto-advance is enabled - navigate directly to next book
      const message = `Continuing to "${adjacentBooks?.next?.title}"...`;
      onBoundaryChange?.("at-end", message);
      goToNextBook();
    } else if (boundaryState === "at-end" && canGoNextBook) {
      // User pressed again at end - navigate to next book
      goToNextBook();
    } else if (canGoNextBook) {
      // First press at end - show message
      setBoundaryState("at-end");
      const message = `End of book. Press again to continue to "${adjacentBooks?.next?.title}"`;
      onBoundaryChange?.("at-end", message);
    } else {
      // No next book
      const message = "You have reached the end of the series";
      onBoundaryChange?.("at-end", message);
    }
  }, [
    isLastPage,
    boundaryState,
    autoAdvance,
    canGoNextBook,
    adjacentBooks?.next?.title,
    nextPage,
    goToNextBook,
    setBoundaryState,
    clearBoundaryState,
    onBoundaryChange,
  ]);

  // Handle previous page with boundary detection
  const handlePrevPage = useCallback(() => {
    if (!isFirstPage) {
      // Not at boundary, just go to previous page
      if (boundaryState !== "none") {
        clearBoundaryState();
      }
      prevPage();
      return;
    }

    // At the first page
    if (autoAdvance && canGoPrevBook) {
      // Auto-advance is enabled - navigate directly to prev book at last page
      const message = `Going back to "${adjacentBooks?.prev?.title}"...`;
      onBoundaryChange?.("at-start", message);
      goToPrevBook();
    } else if (boundaryState === "at-start" && canGoPrevBook) {
      // User pressed again at start - navigate to prev book at last page
      goToPrevBook();
    } else if (canGoPrevBook) {
      // First press at start - show message
      setBoundaryState("at-start");
      const message = `Beginning of book. Press again to go to "${adjacentBooks?.prev?.title}"`;
      onBoundaryChange?.("at-start", message);
    } else {
      // No prev book
      const message = "You are at the beginning of the series";
      onBoundaryChange?.("at-start", message);
    }
  }, [
    isFirstPage,
    boundaryState,
    autoAdvance,
    canGoPrevBook,
    adjacentBooks?.prev?.title,
    prevPage,
    goToPrevBook,
    setBoundaryState,
    clearBoundaryState,
    onBoundaryChange,
  ]);

  // Determine boundary message
  let boundaryMessage: string | null = null;
  if (boundaryState === "at-end" && canGoNextBook) {
    boundaryMessage = `End of book. Press again to continue to "${adjacentBooks?.next?.title}"`;
  } else if (boundaryState === "at-start" && canGoPrevBook) {
    boundaryMessage = `Beginning of book. Press again to go to "${adjacentBooks?.prev?.title}"`;
  }

  return {
    canGoPrevBook,
    canGoNextBook,
    goToPrevBook,
    goToNextBook,
    handleNextPage,
    handlePrevPage,
    boundaryState,
    boundaryMessage,
  };
}
