import { type RefObject, useCallback, useEffect } from "react";
import {
  selectEffectiveReadingDirection,
  useReaderStore,
} from "@/store/readerStore";

/** Fraction of the container height to scroll for Arrow keys */
const ARROW_SCROLL_STEP = 0.75;

/**
 * scrollBy wrapper that detects when the container is at a scroll limit
 * (top or bottom) and the position didn't change.  In that case the browser
 * won't emit a native "scroll" event, so we dispatch one synthetically so
 * the debounced handler in ContinuousScrollReader still runs (boundary
 * detection depends on it).
 */
function scrollByWithBoundaryCheck(
  container: HTMLDivElement,
  options: ScrollToOptions,
) {
  const before = container.scrollTop;
  container.scrollBy(options);
  // For instant scrolls the position updates synchronously.  For smooth
  // scrolls we use requestAnimationFrame to check after the first frame.
  if (options.behavior === "smooth") {
    requestAnimationFrame(() => {
      if (container.scrollTop === before) {
        container.dispatchEvent(new Event("scroll"));
      }
    });
  } else if (container.scrollTop === before) {
    container.dispatchEvent(new Event("scroll"));
  }
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
            scrollByWithBoundaryCheck(container, {
              top: container.clientHeight * ARROW_SCROLL_STEP,
              behavior: "smooth",
            });
          } else {
            nextPage();
          }
          break;

        case "ArrowUp":
          event.preventDefault();
          if (container) {
            scrollByWithBoundaryCheck(container, {
              top: -container.clientHeight * ARROW_SCROLL_STEP,
              behavior: "smooth",
            });
          } else {
            prevPage();
          }
          break;

        case "PageDown":
        case " ": // Space
          event.preventDefault();
          if (container) {
            scrollByWithBoundaryCheck(container, {
              top: container.clientHeight,
              behavior: "smooth",
            });
          } else {
            nextPage();
          }
          break;

        case "PageUp":
          event.preventDefault();
          if (container) {
            scrollByWithBoundaryCheck(container, {
              top: -container.clientHeight,
              behavior: "smooth",
            });
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
