import { Box, Center, Loader, Text } from "@mantine/core";
import {
  type RefObject,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  type BackgroundColor,
  type FitMode,
  useReaderStore,
} from "@/store/readerStore";

// =============================================================================
// Types
// =============================================================================

interface PageEntry {
  pageNumber: number;
  src: string;
  /** Whether the image has been loaded */
  isLoaded: boolean;
}

interface ContinuousScrollReaderProps {
  /** Book ID */
  bookId: string;
  /** Total number of pages */
  totalPages: number;
  /** Current page (used for initial scroll position) */
  initialPage?: number;
  /** Fit mode for images */
  fitMode: FitMode;
  /** Background color */
  backgroundColor: BackgroundColor;
  /** Gap between pages in pixels */
  pageGap?: number;
  /** Number of pages to preload above/below visible area */
  preloadBuffer?: number;
  /** Side padding as percentage (0-40) */
  sidePadding?: number;
  /** Callback when the visible page changes (for progress tracking) */
  onPageChange?: (page: number) => void;
  /** Callback when the user scrolls to the last page */
  onReachedEnd?: () => void;
  /** Callback when the user scrolls to the first page */
  onReachedStart?: () => void;
  /** External ref to the scroll container (for keyboard scrolling) */
  scrollContainerRef?: RefObject<HTMLDivElement | null>;
}

// =============================================================================
// Constants
// =============================================================================

const BACKGROUND_COLORS: Record<BackgroundColor, string> = {
  black: "#000000",
  gray: "#1a1a1a",
  white: "#ffffff",
};

const DEFAULT_PAGE_GAP = 0;
const SCROLL_DEBOUNCE_MS = 100;

// =============================================================================
// Component
// =============================================================================

/**
 * Continuous scroll reader for webtoon-style vertical reading.
 *
 * Features:
 * - Vertical scrolling with all pages in a single container
 * - Lazy loading: only loads images that are visible or near-visible
 * - Intersection Observer for efficient visibility tracking
 * - Scroll-based progress tracking
 * - Supports all fit modes
 */
export function ContinuousScrollReader({
  bookId,
  totalPages,
  initialPage = 1,
  fitMode,
  backgroundColor,
  pageGap,
  preloadBuffer,
  sidePadding = 0,
  onPageChange,
  onReachedEnd,
  onReachedStart,
  scrollContainerRef,
}: ContinuousScrollReaderProps) {
  // Use explicit undefined checks to allow 0 as a valid value
  const effectivePageGap = pageGap ?? DEFAULT_PAGE_GAP;
  const effectivePreloadBuffer = preloadBuffer ?? 0;
  const containerRef = useRef<HTMLDivElement>(null);

  // Ref callback that assigns the container to both internal and external refs
  const setContainerRef = useCallback(
    (el: HTMLDivElement | null) => {
      containerRef.current = el;
      if (scrollContainerRef) {
        (scrollContainerRef as { current: HTMLDivElement | null }).current = el;
      }
    },
    [scrollContainerRef],
  );
  const pageRefs = useRef<Map<number, HTMLDivElement>>(new Map());
  const observerRef = useRef<IntersectionObserver | null>(null);
  const hasScrolledToInitialRef = useRef(false);
  // Initialise to initialPage so the external-sync effect doesn't scroll on mount.
  const lastReportedPageRef = useRef<number>(initialPage);
  // Suppresses boundary detection until the user has scrolled at least once,
  // preventing false "start of book" notifications on initial mount.
  const hasUserScrolledRef = useRef(false);
  // When set to a page number, indicates that an external sync is in progress.
  // The observer skips updating currentVisiblePageRef, the flush skips page
  // reporting, and image loads re-scroll to the target page to compensate for
  // layout shifts.  Cleared by the next user-initiated scroll event.
  const syncTargetPageRef = useRef<number | null>(null);

  // Track which pages are visible (ref is source of truth; state triggers renders).
  // lastFlushedVisibleRef caches a serialised snapshot so we can skip no-op updates.
  const visiblePagesRef = useRef<Set<number>>(new Set());
  const lastFlushedVisibleRef = useRef("");
  const [visiblePages, setVisiblePages] = useState<Set<number>>(new Set());
  // Track which pages have been loaded
  const [loadedPages, setLoadedPages] = useState<Set<number>>(new Set());
  // Current page based on scroll position (topmost visible page).
  // Only stored as a ref; no state needed since nothing in the render path
  // depends on it.  The debounced scroll handler reads the ref and syncs
  // the value to the store via goToPage().
  const currentVisiblePageRef = useRef(initialPage);

  // Reader store actions
  const goToPage = useReaderStore((state) => state.goToPage);

  // Generate page entries.  Deliberately does NOT depend on visiblePages;
  // visibility only affects pagesToRender (below) which is a separate memo.
  const pages: PageEntry[] = useMemo(() => {
    return Array.from({ length: totalPages }, (_, i) => {
      const pageNumber = i + 1;
      return {
        pageNumber,
        src: `/api/v1/books/${bookId}/pages/${pageNumber}`,
        isLoaded: loadedPages.has(pageNumber),
      };
    });
  }, [bookId, totalPages, loadedPages]);

  // Determine which pages should be rendered (visible + buffer)
  const pagesToRender = useMemo(() => {
    const minVisible = Math.min(...visiblePages);
    const maxVisible = Math.max(...visiblePages);

    // If no pages visible yet, render around initial page
    if (visiblePages.size === 0) {
      const start = Math.max(1, initialPage - effectivePreloadBuffer);
      const end = Math.min(totalPages, initialPage + effectivePreloadBuffer);
      return new Set(
        Array.from({ length: end - start + 1 }, (_, i) => start + i),
      );
    }

    // Render visible pages plus buffer
    const start = Math.max(1, minVisible - effectivePreloadBuffer);
    const end = Math.min(totalPages, maxVisible + effectivePreloadBuffer);
    return new Set(
      Array.from({ length: end - start + 1 }, (_, i) => start + i),
    );
  }, [visiblePages, initialPage, totalPages, effectivePreloadBuffer]);

  // Stable refs for callbacks used inside the scroll/observer effects.
  // These let the effects read the latest prop values without re-running.
  const callbacksRef = useRef({
    goToPage,
    onPageChange,
    onReachedEnd,
    onReachedStart,
  });
  callbacksRef.current = {
    goToPage,
    onPageChange,
    onReachedEnd,
    onReachedStart,
  };
  const totalPagesRef = useRef(totalPages);
  totalPagesRef.current = totalPages;

  // Set up intersection observer.
  // The observer only updates refs and visiblePages state (for lazy loading).
  // It never sets currentVisiblePage state directly; that is flushed by the
  // debounced scroll handler below, so mid-animation frames cause zero
  // re-renders from page tracking.
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const options: IntersectionObserverInit = {
      root: container,
      rootMargin: "100px 0px",
      threshold: [0, 0.1, 0.5, 0.9, 1],
    };

    observerRef.current = new IntersectionObserver((entries) => {
      const currentVisible = visiblePagesRef.current;
      let topMostPage = currentVisiblePageRef.current;
      let topMostRatio = 0;

      for (const entry of entries) {
        const pageNum = Number(entry.target.getAttribute("data-page"));
        if (Number.isNaN(pageNum)) continue;

        if (entry.isIntersecting) {
          currentVisible.add(pageNum);
          const rect = entry.boundingClientRect;
          const containerRect = container.getBoundingClientRect();
          const visibleTop = Math.max(rect.top, containerRect.top);
          const visibleBottom = Math.min(rect.bottom, containerRect.bottom);
          const visibleHeight = visibleBottom - visibleTop;
          const ratio = visibleHeight / rect.height;

          if (rect.top <= containerRect.top + 100 && ratio > topMostRatio) {
            topMostPage = pageNum;
            topMostRatio = ratio;
          }
        } else {
          currentVisible.delete(pageNum);
        }
      }

      // Only update refs here; state is flushed by the debounced scroll
      // handler so that mid-animation frames cause zero re-renders.
      // Skip the update while an external sync is active, so that
      // layout shifts from unloaded images don't override the target page.
      if (syncTargetPageRef.current != null) return;
      currentVisiblePageRef.current = topMostPage;
    }, options);

    // Observe all page elements currently registered
    const currentObserver = observerRef.current;
    for (const [, element] of pageRefs.current) {
      currentObserver.observe(element);
    }

    return () => {
      currentObserver.disconnect();
    };
    // Only re-create the observer when the container element changes (i.e. on mount)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Debounced scroll handler: flushes currentVisiblePageRef to state and
  // fires page-change / boundary callbacks.  Runs on every scroll event but
  // debounces so it only acts once scrolling settles.
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    let timeout: NodeJS.Timeout | null = null;

    const flush = () => {
      const tp = totalPagesRef.current;
      const cbs = callbacksRef.current;

      // Sync visible pages to state only when the set contents actually changed,
      // avoiding unnecessary re-renders that cause visual flicker.
      const snapshot = Array.from(visiblePagesRef.current).sort().join(",");
      if (snapshot !== lastFlushedVisibleRef.current) {
        lastFlushedVisibleRef.current = snapshot;
        setVisiblePages(new Set(visiblePagesRef.current));
      }

      // While an external sync is active, skip page reporting and boundary
      // detection.  The observer is also locked, so currentVisiblePageRef
      // still holds the sync target.  We only flush visible-pages state
      // (above) so lazy loading keeps working.
      if (syncTargetPageRef.current != null) return;

      const page = currentVisiblePageRef.current;

      if (page !== lastReportedPageRef.current) {
        // The first distinct page change means the user has scrolled.
        // We don't arm on the very first report (which is the initial
        // observer firing on mount) since lastReportedPageRef starts at
        // initialPage and the first "change" is a real scroll.
        hasUserScrolledRef.current = true;
        lastReportedPageRef.current = page;
        cbs.goToPage(page);
        cbs.onPageChange?.(page);
      }

      // Boundary detection: fire every time the user scrolls/wheels/presses
      // a key while at the first or last page.  The receiving side
      // (useSeriesNavigation) manages its own two-press state machine, so
      // we intentionally do NOT de-duplicate here.
      if (!hasUserScrolledRef.current) return;

      if (page === tp && tp > 0) {
        cbs.onReachedEnd?.();
      } else if (page === 1) {
        cbs.onReachedStart?.();
      }
    };

    const scheduleFlush = () => {
      if (timeout) clearTimeout(timeout);
      timeout = setTimeout(flush, SCROLL_DEBOUNCE_MS);
    };

    // User-interaction events clear the external sync lock.  We listen for
    // wheel/pointerdown/keydown (not "scroll", which also fires for
    // programmatic scrollIntoView calls from image-load re-scrolls).
    const clearSyncLock = () => {
      syncTargetPageRef.current = null;
    };

    // Also listen for wheel events: when the container is at a scroll limit,
    // further wheel events don't produce "scroll" events, so boundary
    // detection would never fire without this.
    const onWheel = () => {
      clearSyncLock();
      scheduleFlush();
    };

    container.addEventListener("scroll", scheduleFlush, { passive: true });
    container.addEventListener("wheel", onWheel, { passive: true });
    // Clear sync lock on any user-initiated interaction
    window.addEventListener("pointerdown", clearSyncLock, { passive: true });
    window.addEventListener("keydown", clearSyncLock, { passive: true });

    return () => {
      container.removeEventListener("scroll", scheduleFlush);
      container.removeEventListener("wheel", onWheel);
      window.removeEventListener("pointerdown", clearSyncLock);
      window.removeEventListener("keydown", clearSyncLock);
      if (timeout) clearTimeout(timeout);
    };
    // Only bind once on mount
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // External page sync: when the store's currentPage changes from an external
  // source (toolbar slider, chevrons), scroll to that page.  The
  // lastReportedPageRef guard prevents reacting to our own goToPage() calls,
  // avoiding feedback loops.
  //
  // We also update lastReportedPageRef and currentVisiblePageRef immediately
  // so the debounced flush doesn't overwrite the externally-set page with a
  // nearby page the observer happens to detect during the scroll.
  const storeCurrentPage = useReaderStore((state) => state.currentPage);
  useEffect(() => {
    if (storeCurrentPage === lastReportedPageRef.current) return;
    const container = containerRef.current;
    if (!container) return;
    lastReportedPageRef.current = storeCurrentPage;
    currentVisiblePageRef.current = storeCurrentPage;
    // Activate the sync lock so observer updates and flush page-reporting
    // are suppressed until the user interacts.  Image loads will re-scroll
    // to the target page to compensate for layout shifts.
    syncTargetPageRef.current = storeCurrentPage;
    const el = container.querySelector(`[data-page="${storeCurrentPage}"]`);
    if (el) {
      el.scrollIntoView({ behavior: "instant", block: "start" });
    }
  }, [storeCurrentPage]);

  // Scroll to initial page on mount
  useEffect(() => {
    if (hasScrolledToInitialRef.current) return;
    if (initialPage <= 1) {
      hasScrolledToInitialRef.current = true;
      return;
    }

    // Wait for page refs to be set
    const targetRef = pageRefs.current.get(initialPage);
    if (targetRef && containerRef.current) {
      hasScrolledToInitialRef.current = true;
      targetRef.scrollIntoView({ behavior: "instant", block: "start" });
    }
  }, [initialPage]);

  // Handle image load.  When an external sync is active, re-scroll to the
  // target page after each image load to compensate for layout shifts caused
  // by images above the target changing from placeholder to actual height.
  const handleImageLoad = useCallback((pageNumber: number) => {
    setLoadedPages((prev) => new Set([...prev, pageNumber]));

    const syncTarget = syncTargetPageRef.current;
    if (syncTarget != null && pageNumber < syncTarget) {
      const container = containerRef.current;
      if (!container) return;
      const el = container.querySelector(`[data-page="${syncTarget}"]`);
      if (el) {
        el.scrollIntoView({ behavior: "instant", block: "start" });
      }
    }
  }, []);

  // Register page ref with observer
  const registerPageRef = useCallback(
    (pageNumber: number, element: HTMLDivElement | null) => {
      if (element) {
        pageRefs.current.set(pageNumber, element);
        observerRef.current?.observe(element);
      } else {
        const existing = pageRefs.current.get(pageNumber);
        if (existing) {
          observerRef.current?.unobserve(existing);
        }
        pageRefs.current.delete(pageNumber);
      }
    },
    [],
  );

  // Get fit mode styles
  const getImageStyles = useCallback((): React.CSSProperties => {
    const baseStyles: React.CSSProperties = {
      display: "block",
      margin: "0 auto",
    };

    switch (fitMode) {
      case "screen":
        return {
          ...baseStyles,
          maxWidth: "100%",
          maxHeight: "100vh",
          objectFit: "contain",
        };
      case "width":
        return {
          ...baseStyles,
          width: "100%",
          height: "auto",
        };
      case "width-shrink":
        return {
          ...baseStyles,
          maxWidth: "100%",
          height: "auto",
        };
      case "height":
        return {
          ...baseStyles,
          height: "100vh",
          width: "auto",
        };
      case "original":
        return baseStyles;
      default:
        return {
          ...baseStyles,
          maxWidth: "100%",
          height: "auto",
        };
    }
  }, [fitMode]);

  if (totalPages === 0) {
    return (
      <Center style={{ width: "100%", height: "100vh" }}>
        <Text c="dimmed">This book has no pages</Text>
      </Center>
    );
  }

  return (
    <Box
      ref={setContainerRef}
      data-testid="continuous-scroll-container"
      style={{
        width: "100%",
        height: "100vh",
        overflow: "auto",
        backgroundColor: BACKGROUND_COLORS[backgroundColor],
      }}
    >
      <Box
        data-testid="continuous-scroll-inner"
        style={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: effectivePageGap,
          paddingLeft: `${sidePadding}%`,
          paddingRight: `${sidePadding}%`,
        }}
      >
        {pages.map((page) => {
          const shouldRender = pagesToRender.has(page.pageNumber);

          return (
            <Box
              key={page.pageNumber}
              ref={(el) => registerPageRef(page.pageNumber, el)}
              data-page={page.pageNumber}
              data-testid={`page-container-${page.pageNumber}`}
              style={{
                width: "100%",
                minHeight: shouldRender ? undefined : "100vh",
                display: "flex",
                justifyContent: "center",
                alignItems: "center",
              }}
            >
              {shouldRender ? (
                <>
                  {!page.isLoaded && (
                    <Center style={{ minHeight: "50vh", width: "100%" }}>
                      <Loader size="md" color="gray" />
                    </Center>
                  )}
                  <img
                    src={page.src}
                    alt={`Page ${page.pageNumber}`}
                    data-testid={`page-image-${page.pageNumber}`}
                    style={{
                      ...getImageStyles(),
                      display: page.isLoaded ? "block" : "none",
                    }}
                    onLoad={() => handleImageLoad(page.pageNumber)}
                    onError={() => {
                      if (page.pageNumber > 1) {
                        useReaderStore
                          .getState()
                          .correctTotalPages(page.pageNumber - 1);
                      }
                    }}
                  />
                </>
              ) : (
                // Placeholder for unrendered pages
                <Box
                  data-testid={`page-placeholder-${page.pageNumber}`}
                  style={{
                    width: "100%",
                    height: "100vh",
                    display: "flex",
                    justifyContent: "center",
                    alignItems: "center",
                  }}
                >
                  <Text c="dimmed" size="sm">
                    Page {page.pageNumber}
                  </Text>
                </Box>
              )}
            </Box>
          );
        })}
      </Box>
    </Box>
  );
}

// =============================================================================
// Scroll-to-page utility hook
// =============================================================================

/**
 * Hook to scroll to a specific page in the continuous reader.
 * Returns a function that can be called with a page number.
 */
export function useScrollToPage(containerRef: React.RefObject<HTMLDivElement>) {
  return useCallback(
    (pageNumber: number, behavior: ScrollBehavior = "smooth") => {
      const container = containerRef.current;
      if (!container) return;

      const pageElement = container.querySelector(
        `[data-page="${pageNumber}"]`,
      );
      if (pageElement) {
        pageElement.scrollIntoView({ behavior, block: "start" });
      }
    },
    [containerRef],
  );
}
