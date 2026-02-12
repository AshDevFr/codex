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
  /** True when at the end of the last book in the series (no next book) */
  isSeriesEnd: boolean;
  /** True when at the start of the first book in the series (no prev book) */
  isSeriesStart: boolean;
}

export interface UseSeriesNavigationOptions {
  /** Callback when boundary state changes (for showing notifications) */
  onBoundaryChange?: (
    state: "none" | "at-start" | "at-end",
    message: string | null,
  ) => void;
  /** Callback to clear both notification and boundary state (cancels auto-hide timeout) */
  clearNotification?: () => void;
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
  const { onBoundaryChange, clearNotification } = options;
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

  // Boundary logic for end-of-book
  const handleEndBoundary = useCallback(
    (currentBoundaryState: "none" | "at-start" | "at-end") => {
      if (autoAdvance && canGoNextBook) {
        const message = `Continuing to next book\n${adjacentBooks?.next?.title}`;
        onBoundaryChange?.("at-end", message);
        goToNextBook();
      } else if (currentBoundaryState === "at-end" && canGoNextBook) {
        goToNextBook();
      } else if (canGoNextBook) {
        setBoundaryState("at-end");
        const message = `End of book\nPress again for "${adjacentBooks?.next?.title}"`;
        onBoundaryChange?.("at-end", message);
      } else {
        setBoundaryState("at-end");
        const message = "End of series\nYou have reached the last book";
        onBoundaryChange?.("at-end", message);
      }
    },
    [
      autoAdvance,
      canGoNextBook,
      adjacentBooks?.next?.title,
      goToNextBook,
      setBoundaryState,
      onBoundaryChange,
    ],
  );

  // Boundary logic for start-of-book
  const handleStartBoundary = useCallback(
    (currentBoundaryState: "none" | "at-start" | "at-end") => {
      if (autoAdvance && canGoPrevBook) {
        const message = `Going back to previous book\n${adjacentBooks?.prev?.title}`;
        onBoundaryChange?.("at-start", message);
        goToPrevBook();
      } else if (currentBoundaryState === "at-start" && canGoPrevBook) {
        goToPrevBook();
      } else if (canGoPrevBook) {
        setBoundaryState("at-start");
        const message = `Beginning of book\nPress again for "${adjacentBooks?.prev?.title}"`;
        onBoundaryChange?.("at-start", message);
      } else {
        setBoundaryState("at-start");
        const message = "Beginning of series\nYou are at the first book";
        onBoundaryChange?.("at-start", message);
      }
    },
    [
      autoAdvance,
      canGoPrevBook,
      adjacentBooks?.prev?.title,
      goToPrevBook,
      setBoundaryState,
      onBoundaryChange,
    ],
  );

  // Clears boundary state and cancels any pending auto-hide timeout.
  // Falls back to store-only clear if clearNotification isn't provided.
  const dismissBoundary = useCallback(() => {
    if (clearNotification) {
      clearNotification();
    } else {
      clearBoundaryState();
    }
  }, [clearNotification, clearBoundaryState]);

  // Handle next page with boundary detection.
  // Reads boundaryState from getState() to always get the freshest value,
  // avoiding race conditions between React re-renders and the auto-hide
  // timeout in useBoundaryNotification that clears boundaryState.
  const handleNextPage = useCallback(() => {
    const currentBoundary = useReaderStore.getState().boundaryState;

    if (!isLastPage) {
      if (currentBoundary !== "none") {
        dismissBoundary();
      }
      const pageBefore = useReaderStore.getState().currentPage;
      nextPage();
      // If the page didn't change, we hit the effective end
      if (useReaderStore.getState().currentPage === pageBefore) {
        handleEndBoundary(currentBoundary);
      }
      return;
    }

    handleEndBoundary(currentBoundary);
  }, [isLastPage, nextPage, dismissBoundary, handleEndBoundary]);

  // Handle previous page with boundary detection.
  // Same getState() approach as handleNextPage.
  const handlePrevPage = useCallback(() => {
    const currentBoundary = useReaderStore.getState().boundaryState;

    if (!isFirstPage) {
      if (currentBoundary !== "none") {
        dismissBoundary();
      }
      const pageBefore = useReaderStore.getState().currentPage;
      prevPage();
      // If the page didn't change, we hit the effective start
      if (useReaderStore.getState().currentPage === pageBefore) {
        handleStartBoundary(currentBoundary);
      }
      return;
    }

    handleStartBoundary(currentBoundary);
  }, [isFirstPage, prevPage, dismissBoundary, handleStartBoundary]);

  // Determine boundary message
  const isSeriesEnd = boundaryState === "at-end" && !canGoNextBook;
  const isSeriesStart = boundaryState === "at-start" && !canGoPrevBook;

  let boundaryMessage: string | null = null;
  if (boundaryState === "at-end" && canGoNextBook) {
    boundaryMessage = `End of book\nPress again for "${adjacentBooks?.next?.title}"`;
  } else if (boundaryState === "at-start" && canGoPrevBook) {
    boundaryMessage = `Beginning of book\nPress again for "${adjacentBooks?.prev?.title}"`;
  } else if (isSeriesEnd) {
    boundaryMessage = "End of series\nYou have reached the last book";
  } else if (isSeriesStart) {
    boundaryMessage = "Beginning of series\nYou are at the first book";
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
    isSeriesEnd,
    isSeriesStart,
  };
}
