import { Box, Center, Loader, Text } from "@mantine/core";
import {
  type ReactNode,
  type RefObject,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  type BackgroundColor,
  type FitMode,
  useReaderStore,
} from "@/store/readerStore";
import { type PageDimension, reservedPageHeight } from "./utils/pageHeight";

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
  /**
   * Real per-page pixel dimensions (from backend analysis), keyed by page
   * number. When present, each page's box is reserved at its exact rendered
   * height before the image loads, so loading causes zero layout shift and the
   * scroll position never jumps. Absent for un-analyzed books.
   */
  pageDimensions?: ReadonlyMap<number, PageDimension>;
  /** Side padding as percentage (0-40) */
  sidePadding?: number;
  /** Callback when the visible page changes (for progress tracking) */
  onPageChange?: (page: number) => void;
  /** External ref to the scroll container (for keyboard scrolling) */
  scrollContainerRef?: RefObject<HTMLDivElement | null>;
  /**
   * Callback ref attached to the scroll container so tap-to-toggle-toolbar
   * navigation (useTouchNav) can listen on the same element that scrolls.
   */
  tapRef?: (el: HTMLDivElement | null) => void;
  /** Panel rendered before the first page (e.g. a "Previous Chapter" panel). */
  leadingSlot?: ReactNode;
  /** Panel rendered after the last page (e.g. a "Next Chapter" panel). */
  trailingSlot?: ReactNode;
  /**
   * Fired on the rising/falling edge of the trailing panel being reached
   * (scrolled to the very bottom). Used to gate the auto-advance countdown.
   */
  onTrailingReachedChange?: (reached: boolean) => void;
  /** Fired on the rising/falling edge of the leading panel being reached (top). */
  onLeadingReachedChange?: (reached: boolean) => void;
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
/** Sub-pixel tolerance when deciding the scroll has reached the top/bottom. */
const BOUNDARY_TOLERANCE = 4;
/**
 * The trailing "Next Chapter" panel counts as reached once it substantially
 * fills the viewport, expressed as a fraction of the viewport height measured
 * up from the very bottom. Engaging before the exact pixel-bottom lets the
 * auto-advance countdown appear as the panel scrolls into view (instead of a
 * brief "Continue Reading" flash that then morphs into the countdown), and
 * keeps the gate far clear of the few dozen px the countdown UI adds so it can
 * never flip-flop. Once engaged the panel only un-reaches after scrolling up
 * past the larger release fraction (hysteresis).
 */
const TRAILING_ENGAGE_FRACTION = 0.25;
const TRAILING_RELEASE_FRACTION = 0.4;

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
  pageDimensions,
  onPageChange,
  scrollContainerRef,
  tapRef,
  leadingSlot,
  trailingSlot,
  onTrailingReachedChange,
  onLeadingReachedChange,
}: ContinuousScrollReaderProps) {
  // Use explicit undefined checks to allow 0 as a valid value
  const effectivePageGap = pageGap ?? DEFAULT_PAGE_GAP;
  const effectivePreloadBuffer = preloadBuffer ?? 0;
  const containerRef = useRef<HTMLDivElement>(null);

  // Ref callback that assigns the container to the internal ref, the optional
  // external scroll-container ref, and the optional tap ref (so tap-to-toggle
  // navigation can listen on the element that actually scrolls).
  const setContainerRef = useCallback(
    (el: HTMLDivElement | null) => {
      containerRef.current = el;
      if (scrollContainerRef) {
        (scrollContainerRef as { current: HTMLDivElement | null }).current = el;
      }
      tapRef?.(el);
    },
    [scrollContainerRef, tapRef],
  );
  const pageRefs = useRef<Map<number, HTMLDivElement>>(new Map());
  // Measured rendered height (px) of each page once its image has loaded.
  // Used as the reserved height while a page is a placeholder or still
  // loading, so virtualising a page in/out of the render window does not
  // change the height of content above the viewport and shift the user's
  // scroll position.  Without this, stopping a scroll (which flushes the
  // render window) snaps the view because off-screen pages above revert to a
  // fixed 100vh placeholder that rarely matches their real height.
  const pageHeightsRef = useRef<Map<number, number>>(new Map());
  // Pages whose image just loaded, awaiting post-commit height measurement.
  // We measure after React commits (the img is display:none at onLoad time) to
  // record a fallback height for un-analyzed pages and to re-pin an in-progress
  // external sync.  We no longer adjust scrollTop here — native scroll
  // anchoring keeps the view steady (see the container's overflow-anchor).
  const pendingLoadsRef = useRef<number[]>([]);
  const observerRef = useRef<IntersectionObserver | null>(null);
  const hasScrolledToInitialRef = useRef(false);
  // Initialise to initialPage so the external-sync effect doesn't scroll on mount.
  const lastReportedPageRef = useRef<number>(initialPage);
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

  // Live layout metrics used to reserve each page's exact height from its real
  // dimensions: the width available to an image (container minus side padding)
  // and the scroll-container height (for viewport-relative fit modes).
  const [layout, setLayout] = useState({ contentWidth: 0, viewportHeight: 0 });
  useLayoutEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const measure = () => {
      const padFactor = Math.max(0, 1 - (2 * sidePadding) / 100);
      const contentWidth = container.clientWidth * padFactor;
      const viewportHeight = container.clientHeight;
      setLayout((prev) =>
        prev.contentWidth === contentWidth &&
        prev.viewportHeight === viewportHeight
          ? prev
          : { contentWidth, viewportHeight },
      );
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(container);
    return () => observer.disconnect();
  }, [sidePadding]);

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
  });
  callbacksRef.current = {
    goToPage,
    onPageChange,
  };
  const totalPagesRef = useRef(totalPages);
  totalPagesRef.current = totalPages;

  // Stable view of the transition-panel config for the scroll handler, plus
  // edge-tracking refs so reached-change callbacks only fire on transitions.
  const reachConfigRef = useRef({
    hasLeading: leadingSlot != null,
    hasTrailing: trailingSlot != null,
    onLeadingReachedChange,
    onTrailingReachedChange,
  });
  reachConfigRef.current = {
    hasLeading: leadingSlot != null,
    hasTrailing: trailingSlot != null,
    onLeadingReachedChange,
    onTrailingReachedChange,
  };
  const trailingReachedRef = useRef(false);
  const leadingReachedRef = useRef(false);

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
      let topMostVisibleHeight = 0;

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

          // The "topmost" page is the one with the most visible area in the
          // viewport.  This correctly handles the last page of a webtoon
          // that may extend well beyond the viewport (its top edge is far
          // above the container top, but it still has the largest visible
          // portion when fully scrolled to the bottom).
          if (visibleHeight > topMostVisibleHeight) {
            topMostPage = pageNum;
            topMostVisibleHeight = visibleHeight;
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
      const cbs = callbacksRef.current;

      // Sync visible pages to state only when the set contents actually changed,
      // avoiding unnecessary re-renders that cause visual flicker.
      const snapshot = Array.from(visiblePagesRef.current).sort().join(",");
      if (snapshot !== lastFlushedVisibleRef.current) {
        lastFlushedVisibleRef.current = snapshot;
        setVisiblePages(new Set(visiblePagesRef.current));
      }

      // While an external sync is active, skip page reporting.
      // The observer is also locked, so currentVisiblePageRef
      // still holds the sync target.  We only flush visible-pages state
      // (above) so lazy loading keeps working.
      if (syncTargetPageRef.current != null) return;

      // Transition-panel reach detection.  When a trailing "Next Chapter"
      // panel is present and the user has scrolled to the very bottom, the
      // final page has been passed — force the reported page to the last page
      // so progress reaches 100% and the book is marked complete.  Without
      // this, the "most visible page" heuristic settles on the second-to-last
      // page (a tall final image or the panel itself dominates the viewport),
      // capping progress short of the end.  Routing it through the normal
      // report path (below) also updates lastReportedPageRef, so the external
      // sync effect won't yank the scroll back up to the last page.
      const reach = reachConfigRef.current;
      let atBottom = false;
      let atTop = false;
      if (reach.hasTrailing || reach.hasLeading) {
        const distanceFromBottom =
          container.scrollHeight -
          (container.scrollTop + container.clientHeight);
        // Engage once the trailing panel substantially fills the viewport, and
        // (hysteresis) stay engaged until the user scrolls up past the larger
        // release fraction. The asymmetric, viewport-relative threshold makes
        // the countdown appear as the panel arrives and keeps the growing
        // countdown UI from flip-flopping the gate.
        const trailingFraction = trailingReachedRef.current
          ? TRAILING_RELEASE_FRACTION
          : TRAILING_ENGAGE_FRACTION;
        atBottom =
          distanceFromBottom <= container.clientHeight * trailingFraction;
        atTop = container.scrollTop <= BOUNDARY_TOLERANCE;
        if (reach.hasTrailing && atBottom) {
          currentVisiblePageRef.current = totalPagesRef.current;
        }
      }

      const page = currentVisiblePageRef.current;

      if (page !== lastReportedPageRef.current) {
        lastReportedPageRef.current = page;
        cbs.goToPage(page);
        cbs.onPageChange?.(page);
      }

      // Notify rising/falling edges of reaching the trailing/leading panels so
      // the parent can gate the auto-advance countdown.  Two-press keyboard
      // boundary detection still lives in useKeyboardNav.
      if (reach.hasTrailing && atBottom !== trailingReachedRef.current) {
        trailingReachedRef.current = atBottom;
        reach.onTrailingReachedChange?.(atBottom);
      }
      if (reach.hasLeading && atTop !== leadingReachedRef.current) {
        leadingReachedRef.current = atTop;
        reach.onLeadingReachedChange?.(atTop);
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
    // further wheel events don't produce "scroll" events, so we schedule a
    // flush to keep visible-pages state and page tracking up to date.
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

  // Scroll to initial page on mount.  A leading panel sits above page 1, so
  // even when starting at page 1 we must scroll down past it to land on the
  // first page rather than opening on the "Previous Chapter" panel.
  const hasLeadingSlot = leadingSlot != null;
  useEffect(() => {
    if (hasScrolledToInitialRef.current) return;
    const targetPage = Math.max(1, initialPage);
    if (targetPage <= 1 && !hasLeadingSlot) {
      hasScrolledToInitialRef.current = true;
      return;
    }

    // Wait for page refs to be set
    const targetRef = pageRefs.current.get(targetPage);
    if (targetRef && containerRef.current) {
      hasScrolledToInitialRef.current = true;
      targetRef.scrollIntoView({ behavior: "instant", block: "start" });
    }
  }, [initialPage, hasLeadingSlot]);

  // Handle image load.  The img is still display:none here (loadedPages hasn't
  // flushed yet), so just queue the page; measurement happens after commit.
  const handleImageLoad = useCallback((pageNumber: number) => {
    pendingLoadsRef.current.push(pageNumber);
    setLoadedPages((prev) => new Set([...prev, pageNumber]));
  }, []);

  // Post-commit bookkeeping for freshly-loaded images.  Runs after React
  // commits loadedPages, when the images occupy their real height.
  // - Record each measured height as a fallback reserved height (only matters
  //   for un-analyzed pages without known dimensions).
  // - During an external sync (slider/chevron jump), if an earlier page changed
  //   height, re-pin the sync target to the viewport top.
  // We deliberately do NOT touch scrollTop otherwise: the container enables
  // native scroll anchoring (overflow-anchor:auto) which keeps the view steady
  // for any residual shift, and exact reserved heights mean there is usually no
  // shift at all.  The old manual scrollTop math is what caused the visible
  // jump when a page preloaded mid-scroll.
  // biome-ignore lint/correctness/useExhaustiveDependencies: loadedPages is the commit signal for freshly-loaded images; the effect reads refs
  useLayoutEffect(() => {
    const pending = pendingLoadsRef.current;
    if (pending.length === 0) return;
    pendingLoadsRef.current = [];
    const container = containerRef.current;
    if (!container) return;

    const syncTarget = syncTargetPageRef.current;
    let resyncNeeded = false;

    for (const page of pending) {
      const el = pageRefs.current.get(page);
      if (!el) continue;
      const newHeight = el.offsetHeight;
      if (newHeight > 0) {
        pageHeightsRef.current.set(page, newHeight);
      }
      if (syncTarget != null && page < syncTarget) {
        resyncNeeded = true;
      }
    }

    if (syncTarget != null && resyncNeeded) {
      const el = container.querySelector(`[data-page="${syncTarget}"]`);
      el?.scrollIntoView({ behavior: "instant", block: "start" });
    }
  }, [loadedPages]);

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
      <Center style={{ width: "100%", height: "100dvh" }}>
        <Text c="dimmed">This book has no pages</Text>
      </Center>
    );
  }

  // Estimate for pages without known dimensions that have never been measured.
  // Webtoon pages vary a lot, so the average of already-measured pages is a far
  // better guess than a flat 100vh — it keeps first-load shifts small for
  // un-analyzed books (where native scroll anchoring picks up the slack).
  const measured = Array.from(pageHeightsRef.current.values());
  const estimatedHeight =
    measured.length > 0
      ? `${Math.round(measured.reduce((sum, h) => sum + h, 0) / measured.length)}px`
      : "100vh";

  // Exact reserved height (px) for a page from its real dimensions + current
  // layout, or null when unavailable (un-analyzed page, or layout not measured
  // yet).  This is the primary jump-prevention mechanism: a correctly-sized box
  // means the image loads with no reflow.
  const knownReservedHeight = (pageNumber: number): number | null => {
    const dimension = pageDimensions?.get(pageNumber);
    if (!dimension) return null;
    return reservedPageHeight({
      fitMode,
      contentWidth: layout.contentWidth,
      viewportHeight: layout.viewportHeight,
      dimension,
    });
  };

  return (
    <Box
      ref={setContainerRef}
      data-testid="continuous-scroll-container"
      style={{
        width: "100%",
        height: "100dvh",
        overflow: "auto",
        backgroundColor: BACKGROUND_COLORS[backgroundColor],
        // Let the browser keep the scroll position pinned when content above
        // the viewport changes size (e.g. a page image finishing loading).
        // Combined with exact reserved heights (from real page dimensions),
        // this prevents the scroll from jumping — no manual scrollTop math.
        // Note: unsupported in iOS Safari, which is why exact reservation is
        // the primary mechanism (it removes the shift rather than absorbing it).
        overflowAnchor: "auto",
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
        {leadingSlot && (
          <Box data-testid="continuous-leading-slot" style={{ width: "100%" }}>
            {leadingSlot}
          </Box>
        )}
        {pages.map((page) => {
          const shouldRender = pagesToRender.has(page.pageNumber);
          // Reserve the page's height while it is a placeholder OR rendered but
          // not yet loaded, so both virtualising in/out and the loading state
          // are layout-neutral and don't shift the scroll position.  Prefer the
          // exact height from real dimensions; fall back to the last measured
          // height, then the average estimate (un-analyzed books only).
          const exactHeight = knownReservedHeight(page.pageNumber);
          const measuredHeight = pageHeightsRef.current.get(page.pageNumber);
          const reservedHeight =
            exactHeight != null && exactHeight > 0
              ? `${exactHeight}px`
              : measuredHeight
                ? `${measuredHeight}px`
                : estimatedHeight;

          return (
            <Box
              key={page.pageNumber}
              ref={(el) => registerPageRef(page.pageNumber, el)}
              data-page={page.pageNumber}
              data-testid={`page-container-${page.pageNumber}`}
              style={{
                position: "relative",
                width: "100%",
                minHeight:
                  shouldRender && page.isLoaded ? undefined : reservedHeight,
                display: "flex",
                justifyContent: "center",
                alignItems: "center",
              }}
            >
              {shouldRender ? (
                <>
                  {!page.isLoaded && (
                    // Overlay the loader so it never expands the reserved box
                    // (a short page may be shorter than the loader).
                    <Center
                      style={{ position: "absolute", inset: 0, width: "100%" }}
                    >
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
                    height: reservedHeight,
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
        {trailingSlot && (
          <Box data-testid="continuous-trailing-slot" style={{ width: "100%" }}>
            {trailingSlot}
          </Box>
        )}
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
