import { type RefObject, useCallback, useEffect } from "react";
import {
  selectEffectiveReadingDirection,
  useReaderStore,
} from "@/store/readerStore";

/** Fraction of the container height to scroll for Arrow keys */
const ARROW_SCROLL_STEP = 0.75;

/**
 * scrollBy wrapper that detects when the container is already at a scroll
 * boundary before attempting to scroll.  If at the boundary, calls the
 * optional onBoundary callback (used for next/previous book navigation).
 *
 * We check the boundary *before* calling scrollBy rather than comparing
 * scrollTop before/after, because smooth scrolling is async and a single
 * requestAnimationFrame is not enough to detect whether the position will
 * change.
 */
function scrollByWithBoundaryCheck(
  container: HTMLDivElement,
  options: ScrollToOptions,
  onBoundary?: () => void,
) {
  const scrollingDown = (options.top ?? 0) > 0;

  if (scrollingDown) {
    // At the bottom when we can't scroll any further down.
    // Use a 1px tolerance to account for sub-pixel rounding.
    const atBottom =
      container.scrollTop + container.clientHeight >=
      container.scrollHeight - 1;
    if (atBottom) {
      onBoundary?.();
      return;
    }
  } else {
    // At the top when scrollTop is 0 (or effectively 0).
    const atTop = container.scrollTop <= 0;
    if (atTop) {
      onBoundary?.();
      return;
    }
  }

  container.scrollBy(options);
}

interface UseKeyboardNavOptions {
  /** Whether keyboard navigation is enabled */
  enabled?: boolean;
  /** Callback when escape is pressed */
  onEscape?: () => void;
  /** Custom handler for next page (overrides default store action) */
  onNextPage?: () => void;
  /** Custom handler for previous page (overrides default store action) */
  onPrevPage?: () => void;
  /**
   * Ref to the continuous-scroll container element.  When provided,
   * directional keys (arrows, Space, PageDown/Up) scroll the container
   * directly instead of going through the page store.
   * Arrow keys scroll by 75% of the viewport; Space/PageDown/Up scroll
   * by a full viewport height.
   */
  scrollContainerRef?: RefObject<HTMLDivElement | null>;
  /** Callback when a scroll-down key is pressed at the bottom of the scroll container */
  onBoundaryEnd?: () => void;
  /** Callback when a scroll-up key is pressed at the top of the scroll container */
  onBoundaryStart?: () => void;
}

/**
 * Hook for keyboard navigation in the reader.
 *
 * Supports:
 * - Arrow keys (left/right/up/down) for page navigation
 * - Page Up/Down for page navigation
 * - Space for next page
 * - Home/End for first/last page
 * - F for fullscreen toggle
 * - Escape for exit/close
 *
 * Reading direction is respected:
 * - LTR: Left = previous, Right = next, Up = previous, Down = next
 * - RTL: Left = next, Right = previous, Up = previous, Down = next
 * - TTB: Up = previous, Down = next, Left = previous, Right = next
 */
export function useKeyboardNav({
  enabled = true,
  onEscape,
  onNextPage,
  onPrevPage,
  scrollContainerRef,
  onBoundaryEnd,
  onBoundaryStart,
}: UseKeyboardNavOptions = {}) {
  const storeNextPage = useReaderStore((state) => state.nextPage);
  const storePrevPage = useReaderStore((state) => state.prevPage);
  const firstPage = useReaderStore((state) => state.firstPage);
  const lastPage = useReaderStore((state) => state.lastPage);
  const toggleFullscreen = useReaderStore((state) => state.toggleFullscreen);
  const toggleToolbar = useReaderStore((state) => state.toggleToolbar);
  const cycleFitMode = useReaderStore((state) => state.cycleFitMode);
  const readingDirection = useReaderStore(selectEffectiveReadingDirection);

  // Use custom handlers if provided, otherwise fall back to store actions
  const nextPage = onNextPage ?? storeNextPage;
  const prevPage = onPrevPage ?? storePrevPage;

  const handleKeyDown = useCallback(
    (event: KeyboardEvent) => {
      // Don't handle if focus is on an input element
      const target = event.target as HTMLElement;
      if (
        target.tagName === "INPUT" ||
        target.tagName === "TEXTAREA" ||
        target.isContentEditable
      ) {
        return;
      }

      const container = scrollContainerRef?.current;

      // Navigation keys based on reading direction
      // LTR: Right = next, Left = prev
      // RTL: Right = prev, Left = next (reversed horizontal)
      // TTB: Down = next, Up = prev, Left/Right act like LTR
      const isRtl = readingDirection === "rtl";

      switch (event.key) {
        case "ArrowRight":
          if (container) break; // No horizontal page nav in continuous scroll
          event.preventDefault();
          if (isRtl) {
            prevPage();
          } else {
            nextPage();
          }
          break;

        case "ArrowLeft":
          if (container) break;
          event.preventDefault();
          if (isRtl) {
            nextPage();
          } else {
            prevPage();
          }
          break;

        case "ArrowDown":
          event.preventDefault();
          if (container) {
            scrollByWithBoundaryCheck(
              container,
              {
                top: container.clientHeight * ARROW_SCROLL_STEP,
                behavior: "smooth",
              },
              onBoundaryEnd,
            );
          } else {
            nextPage();
          }
          break;

        case "ArrowUp":
          event.preventDefault();
          if (container) {
            scrollByWithBoundaryCheck(
              container,
              {
                top: -container.clientHeight * ARROW_SCROLL_STEP,
                behavior: "smooth",
              },
              onBoundaryStart,
            );
          } else {
            prevPage();
          }
          break;

        case "PageDown":
        case " ": // Space
          event.preventDefault();
          if (container) {
            scrollByWithBoundaryCheck(
              container,
              {
                top: container.clientHeight,
                behavior: "smooth",
              },
              onBoundaryEnd,
            );
          } else {
            nextPage();
          }
          break;

        case "PageUp":
          event.preventDefault();
          if (container) {
            scrollByWithBoundaryCheck(
              container,
              {
                top: -container.clientHeight,
                behavior: "smooth",
              },
              onBoundaryStart,
            );
          } else {
            prevPage();
          }
          break;

        case "Home":
          event.preventDefault();
          if (container) {
            container.scrollTo({ top: 0, behavior: "instant" });
          }
          firstPage();
          break;

        case "End":
          event.preventDefault();
          if (container) {
            container.scrollTo({
              top: container.scrollHeight,
              behavior: "instant",
            });
          }
          lastPage();
          break;

        case "f":
        case "F":
          event.preventDefault();
          toggleFullscreen();
          break;

        case "t":
        case "T":
          event.preventDefault();
          toggleToolbar();
          break;

        case "m":
        case "M":
          // Cycle through fit modes
          event.preventDefault();
          cycleFitMode();
          break;

        case "Escape":
          event.preventDefault();
          onEscape?.();
          break;

        default:
          break;
      }
    },
    [
      readingDirection,
      scrollContainerRef,
      nextPage,
      prevPage,
      firstPage,
      lastPage,
      toggleFullscreen,
      toggleToolbar,
      cycleFitMode,
      onEscape,
      onBoundaryEnd,
      onBoundaryStart,
    ],
  );

  useEffect(() => {
    if (!enabled) return;

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [enabled, handleKeyDown]);
}
