import { Box, Center, Loader, Text } from "@mantine/core";
import { useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { booksApi } from "@/api/books";
import {
  DOWNLOADS_BROADCAST_CHANNEL,
  type DownloadsBroadcast,
  getDownload,
} from "@/lib/offline/db";
import { getEffectivePreloadWindow } from "@/lib/offline/prefetchWindow";
import {
  type FitMode,
  type PageOrientation,
  selectEffectiveReadingDirection,
  selectSwipeNavigation,
  useReaderStore,
  type WebtoonFitMode,
} from "@/store/readerStore";
import { ChapterTransitionPanel } from "./ChapterTransitionPanel";
import { ComicReaderPage } from "./ComicReaderPage";
import { ContinuousScrollReader } from "./ContinuousScrollReader";
import { DoublePageSpread } from "./DoublePageSpread";
import {
  useAdjacentBooks,
  useKeyboardNav,
  useReadProgress,
  useSeriesNavigation,
  useSeriesReaderSettings,
  useTouchNav,
} from "./hooks";
import { MobileReaderBottomBar } from "./MobileReaderBottomBar";
import { PageTransitionWrapper } from "./PageTransitionWrapper";
import { ReaderFirstRunHint } from "./ReaderFirstRunHint";
import { ReaderSettings } from "./ReaderSettings";
import { ReaderToolbar } from "./ReaderToolbar";
import { SwipePager } from "./SwipePager";
import {
  detectPageOrientation,
  getDisplayOrder,
  getNextSpreadPage,
  getPreloadPages,
  getPrevSpreadPage,
  getSpreadPages,
  type SpreadConfig,
} from "./utils/spreadCalculation";

interface ComicReaderProps {
  /** Book ID */
  bookId: string;
  /** Series ID (for updating reading direction) */
  seriesId: string | null;
  /** Book title for display */
  title: string;
  /** Total number of pages */
  totalPages: number;
  /** Book format (CBZ, CBR, PDF, EPUB) */
  format: string;
  /** Reading direction from series/library metadata (optional) */
  readingDirectionOverride?: "ltr" | "rtl" | "ttb" | "webtoon" | null;
  /** Whether the book has been analyzed (page dimensions available) */
  analyzed?: boolean;
  /** Starting page from URL parameter (overrides saved progress) */
  startPage?: number;
  /** Incognito mode - when true, progress tracking is disabled */
  incognito?: boolean;
  /** Callback when reader should close */
  onClose: () => void;
}

/** Ordered cycle of fit modes for keyboard shortcut toggling */
const FIT_MODE_CYCLE: FitMode[] = [
  "screen",
  "width",
  "width-shrink",
  "height",
  "original",
];

/**
 * Main comic reader component.
 *
 * Features:
 * - Single page view with click navigation
 * - Keyboard navigation
 * - Progress tracking with backend sync
 * - Fullscreen support
 * - Auto-hiding toolbar
 * - Preloading adjacent pages
 */
export function ComicReader({
  bookId,
  seriesId,
  title,
  totalPages,
  format,
  readingDirectionOverride,
  analyzed = false,
  startPage,
  incognito,
  onClose,
}: ComicReaderProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const hideTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const initializedBookIdRef = useRef<string | null>(null);
  const [settingsOpened, setSettingsOpened] = useState(false);
  // Whether the user has scrolled to the trailing "Next Chapter" panel in
  // continuous/webtoon mode. Gates the auto-advance countdown so it only runs
  // once the panel is actually on screen (the panel is always in the scroll
  // content, far below the last page, until then).
  const [trailingReached, setTrailingReached] = useState(false);

  // Per-series settings (forkable settings with series overrides)
  const {
    effectiveSettings,
    isLoaded: seriesSettingsLoaded,
    hasSeriesOverride,
    updateSetting: updateSeriesSetting,
  } = useSeriesReaderSettings(seriesId);

  // Extract forkable settings from effective settings
  const {
    fitMode: comicFitMode,
    webtoonFitMode,
    backgroundColor,
    pageLayout,
    doublePageShowWideAlone,
    doublePageStartOnOdd,
  } = effectiveSettings;

  // Reader store state (global/non-forkable settings)
  const currentPage = useReaderStore((state) => state.currentPage);
  const currentBookId = useReaderStore((state) => state.currentBookId);
  const toolbarVisible = useReaderStore((state) => state.toolbarVisible);
  const isFullscreen = useReaderStore((state) => state.isFullscreen);
  const autoHideToolbar = useReaderStore(
    (state) => state.settings.autoHideToolbar,
  );
  const toolbarHideDelay = useReaderStore(
    (state) => state.settings.toolbarHideDelay,
  );
  const preloadPages = useReaderStore((state) => state.settings.preloadPages);
  const pageOrientations = useReaderStore((state) => state.pageOrientations);
  const readingDirection = useReaderStore(selectEffectiveReadingDirection);
  const swipeNavigation = useReaderStore(selectSwipeNavigation);

  // Resolve the active fit mode based on reading direction
  const isWebtoon = readingDirection === "webtoon";
  const fitMode: FitMode = isWebtoon ? webtoonFitMode : comicFitMode;

  const adjacentBooks = useReaderStore((state) => state.adjacentBooks);
  const boundaryView = useReaderStore((state) => state.boundaryView);
  const pageTransition = useReaderStore(
    (state) => state.settings.pageTransition,
  );
  const transitionDuration = useReaderStore(
    (state) => state.settings.transitionDuration,
  );
  const lastNavigationDirection = useReaderStore(
    (state) => state.lastNavigationDirection,
  );
  const webtoonSidePadding = useReaderStore(
    (state) => state.settings.webtoonSidePadding,
  );
  const webtoonPageGap = useReaderStore(
    (state) => state.settings.webtoonPageGap,
  );
  const autoAdvanceToNextBook = useReaderStore(
    (state) => state.settings.autoAdvanceToNextBook,
  );
  const autoAdvanceSeconds = useReaderStore(
    (state) => state.settings.autoAdvanceSeconds,
  );

  // Reader store actions
  const initializeReader = useReaderStore((state) => state.initializeReader);
  const setReadingDirectionOverrideAction = useReaderStore(
    (state) => state.setReadingDirectionOverride,
  );
  const setToolbarVisible = useReaderStore((state) => state.setToolbarVisible);
  const setFullscreen = useReaderStore((state) => state.setFullscreen);
  const toggleToolbar = useReaderStore((state) => state.toggleToolbar);
  const setPageOrientation = useReaderStore(
    (state) => state.setPageOrientation,
  );
  const goToPage = useReaderStore((state) => state.goToPage);
  const setBoundaryView = useReaderStore((state) => state.setBoundaryView);
  const correctTotalPages = useReaderStore((state) => state.correctTotalPages);
  const setLastNavigationDirection = useReaderStore(
    (state) => state.setLastNavigationDirection,
  );
  const addPreloadedImage = useReaderStore((state) => state.addPreloadedImage);
  const setGlobalFitMode = useReaderStore((state) => state.setFitMode);
  const setGlobalWebtoonFitMode = useReaderStore(
    (state) => state.setWebtoonFitMode,
  );
  const setGlobalPageLayout = useReaderStore((state) => state.setPageLayout);

  // Track whether the current book has been saved for offline reading.
  // When true, the prefetch window expands aggressively (every page is in
  // the SW cache; preloading them just primes the browser's image decoder).
  // The listener keeps the flag in sync if the user removes/re-downloads
  // the book while the reader stays open.
  const [isBookDownloaded, setIsBookDownloaded] = useState(false);
  useEffect(() => {
    let cancelled = false;
    async function hydrate() {
      try {
        const record = await getDownload(bookId);
        if (!cancelled) {
          setIsBookDownloaded(record?.status === "complete");
        }
      } catch {
        if (!cancelled) setIsBookDownloaded(false);
      }
    }
    void hydrate();

    let channel: BroadcastChannel | null = null;
    if (typeof BroadcastChannel !== "undefined") {
      channel = new BroadcastChannel(DOWNLOADS_BROADCAST_CHANNEL);
      channel.addEventListener("message", handleBroadcast);
    }
    function handleBroadcast(ev: MessageEvent<DownloadsBroadcast>) {
      const payload = ev.data;
      if (payload.kind === "delete" && payload.id === bookId) {
        setIsBookDownloaded(false);
      } else if (payload.kind === "clear") {
        setIsBookDownloaded(false);
      } else if (payload.kind === "put" && payload.record.id === bookId) {
        setIsBookDownloaded(payload.record.status === "complete");
      }
    }

    return () => {
      cancelled = true;
      if (channel) {
        channel.removeEventListener("message", handleBroadcast);
        channel.close();
      }
    };
  }, [bookId]);

  // Fetch adjacent books for series navigation
  useAdjacentBooks({ bookId, enabled: true });

  // Reset the trailing-panel reached flag whenever the book changes so the
  // countdown doesn't carry over from the previous book. Render-phase reset
  // (per the React docs) handles the case where the reader is reused across a
  // book navigation rather than remounted.
  const [prevBookId, setPrevBookId] = useState(bookId);
  if (bookId !== prevBookId) {
    setPrevBookId(bookId);
    setTrailingReached(false);
  }

  // Series navigation. The transition panels (in-flow for webtoon, overlay for
  // paginated) replace the old two-press boundary toast, so this no longer
  // consumes onBoundaryChange / handleEndBoundary.
  const {
    handleNextPage,
    handlePrevPage,
    goToNextBook,
    goToPrevBook,
    canGoNextBook,
    canGoPrevBook,
  } = useSeriesNavigation({
    onBeforeNavigateToNext: incognito
      ? undefined
      : () => {
          cancelPendingSave();
          booksApi.markAsRead(bookId);
        },
  });

  // Read progress hook (disabled in incognito mode)
  const {
    initialPage,
    isLoading: progressLoading,
    cancelPendingSave,
    saveProgress,
  } = useReadProgress({
    bookId,
    totalPages,
    enabled: !incognito,
  });

  // Fetch page dimensions when book is analyzed
  // This allows us to pre-populate orientations for smart spread calculation
  const { data: pages, isLoading: pagesLoading } = useQuery({
    queryKey: ["book-pages", bookId],
    queryFn: () => booksApi.getPages(bookId),
    enabled: analyzed,
    staleTime: Number.POSITIVE_INFINITY, // Page dimensions don't change
  });

  // Real per-page dimensions for the webtoon reader, so it can reserve each
  // page's exact height before the image loads (prevents scroll-position jumps
  // on variable-height pages).  Only populated for analyzed books.
  const pageDimensions = useMemo(() => {
    if (!pages || pages.length === 0) return undefined;
    const map = new Map<number, { width: number; height: number }>();
    for (const page of pages) {
      if (page.width != null && page.height != null) {
        map.set(page.pageNumber, { width: page.width, height: page.height });
      }
    }
    return map.size > 0 ? map : undefined;
  }, [pages]);

  // Are we still waiting for data needed before initialization?
  // For analyzed books, we wait for pages to load so we can compute final spreads.
  // For non-analyzed books, pages query is disabled so pagesLoading is false.
  const dataReady = !progressLoading && (!analyzed || !pagesLoading);

  // Initialize reader once all data is ready.
  // This runs once per bookId and does everything atomically:
  // 1. Populates all orientations from backend pages (if available)
  // 2. Computes the spread-adjusted start page
  // 3. Initializes the reader store with the correct page
  useEffect(() => {
    if (
      !dataReady ||
      totalPages <= 0 ||
      initializedBookIdRef.current === bookId
    ) {
      return;
    }
    initializedBookIdRef.current = bookId;

    // Populate all orientations from backend data before computing spreads
    if (pages && pages.length > 0) {
      for (const page of pages) {
        if (page.width != null && page.height != null) {
          const orientation = detectPageOrientation(page.width, page.height);
          setPageOrientation(page.pageNumber, orientation);
        }
      }
    }

    // Determine the effective starting page:
    // 1. URL parameter (startPage) takes priority if valid
    // 2. Otherwise use saved progress (initialPage)
    let effectiveStartPage: number;
    if (startPage && startPage >= 1 && startPage <= totalPages) {
      effectiveStartPage = startPage;
    } else {
      effectiveStartPage = initialPage;
    }

    initializeReader(bookId, totalPages, effectiveStartPage);

    // Set reading direction override from series/library
    if (readingDirectionOverride) {
      setReadingDirectionOverrideAction(readingDirectionOverride);
    }
  }, [
    dataReady,
    bookId,
    totalPages,
    startPage,
    initialPage,
    pages,
    readingDirectionOverride,
    initializeReader,
    setPageOrientation,
    setReadingDirectionOverrideAction,
  ]);

  // Cleanup on unmount only
  useEffect(() => {
    return () => {
      initializedBookIdRef.current = null;
      useReaderStore.getState().resetSession();
    };
  }, []);

  // Fullscreen handling
  useEffect(() => {
    const handleFullscreenChange = () => {
      setFullscreen(!!document.fullscreenElement);
    };

    document.addEventListener("fullscreenchange", handleFullscreenChange);
    return () => {
      document.removeEventListener("fullscreenchange", handleFullscreenChange);
    };
  }, [setFullscreen]);

  // Enter/exit fullscreen
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    if (isFullscreen && !document.fullscreenElement) {
      container.requestFullscreen?.().catch(() => {
        // Fullscreen request failed (e.g., not allowed)
        setFullscreen(false);
      });
    } else if (!isFullscreen && document.fullscreenElement) {
      document.exitFullscreen?.();
    }
  }, [isFullscreen, setFullscreen]);

  // Auto-hide toolbar. Suppressed while a paginated transition overlay is up so
  // the toolbar (close/settings) and bottom nav bar stay reachable — otherwise
  // an "End of series" overlay (no Continue button) could trap the user.
  const resetHideTimeout = useCallback(() => {
    if (hideTimeoutRef.current) {
      clearTimeout(hideTimeoutRef.current);
    }

    if (autoHideToolbar && toolbarVisible && boundaryView === "none") {
      hideTimeoutRef.current = setTimeout(() => {
        setToolbarVisible(false);
      }, toolbarHideDelay);
    }
  }, [
    autoHideToolbar,
    toolbarVisible,
    toolbarHideDelay,
    setToolbarVisible,
    boundaryView,
  ]);

  // Reveal the toolbar whenever the paginated transition overlay appears.
  useEffect(() => {
    if (boundaryView !== "none") {
      setToolbarVisible(true);
    }
  }, [boundaryView, setToolbarVisible]);

  useEffect(() => {
    resetHideTimeout();
    return () => {
      if (hideTimeoutRef.current) {
        clearTimeout(hideTimeoutRef.current);
      }
    };
  }, [resetHideTimeout]);

  // Show toolbar on mouse / pen move. Skip touch — synthetic mouse events
  // fire after every tap on touch devices, which would pop the toolbar open
  // every time the user paged forward via a side-zone tap.
  const handlePointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (e.pointerType === "touch") return;
      if (!toolbarVisible) {
        setToolbarVisible(true);
      }
      resetHideTimeout();
    },
    [toolbarVisible, setToolbarVisible, resetHideTimeout],
  );

  // Cycle fit mode - respects series settings if override exists
  // In webtoon mode, cycles between "width" and "original" only
  const handleCycleFitMode = useCallback(() => {
    if (isWebtoon) {
      const nextMode: WebtoonFitMode =
        webtoonFitMode === "width" ? "original" : "width";
      if (hasSeriesOverride) {
        updateSeriesSetting("webtoonFitMode", nextMode);
      } else {
        setGlobalWebtoonFitMode(nextMode);
      }
    } else {
      const currentIndex = FIT_MODE_CYCLE.indexOf(comicFitMode);
      const nextIndex = (currentIndex + 1) % FIT_MODE_CYCLE.length;
      const nextMode = FIT_MODE_CYCLE[nextIndex];
      if (hasSeriesOverride) {
        updateSeriesSetting("fitMode", nextMode);
      } else {
        setGlobalFitMode(nextMode);
      }
    }
  }, [
    isWebtoon,
    webtoonFitMode,
    comicFitMode,
    hasSeriesOverride,
    updateSeriesSetting,
    setGlobalFitMode,
    setGlobalWebtoonFitMode,
  ]);

  // Toggle page layout - respects series settings if override exists
  const handleTogglePageLayout = useCallback(() => {
    const newLayout = pageLayout === "single" ? "double" : "single";

    if (hasSeriesOverride) {
      updateSeriesSetting("pageLayout", newLayout);
    } else {
      setGlobalPageLayout(newLayout);
    }
  }, [pageLayout, hasSeriesOverride, updateSeriesSetting, setGlobalPageLayout]);

  // Webtoon keyboard boundary callbacks. A scroll-down key at the very bottom
  // (the trailing "Next Chapter" panel is fully in view) advances to the next
  // book; a scroll-up key at the very top goes to the previous book. This makes
  // the keyboard match the panel's button.
  const handleScrollReachedEnd = useCallback(() => {
    if (canGoNextBook) goToNextBook();
  }, [canGoNextBook, goToNextBook]);

  const handleScrollReachedStart = useCallback(() => {
    if (canGoPrevBook) goToPrevBook();
  }, [canGoPrevBook, goToPrevBook]);

  // Generate page URL
  const getPageUrl = useCallback(
    (pageNumber: number) => {
      return `/api/v1/books/${bookId}/pages/${pageNumber}`;
    },
    [bookId],
  );

  // When a page image fails to load, the real page count is less than metadata.
  // Correct totalPages so boundary detection works correctly.
  const handlePageError = useCallback(() => {
    const page = useReaderStore.getState().currentPage;
    if (page > 1) {
      correctTotalPages(page - 1);
    }
  }, [correctTotalPages]);

  // Determine if we have orientation data loaded from backend
  // Only enable showWideAlone when we have pre-populated orientations from backend pages
  const hasOrientationsLoaded = useMemo(() => {
    // If not analyzed, orientations come from preloading (not reliable for spreads)
    if (!analyzed) return false;
    // If pages haven't loaded yet, we don't have orientations
    if (!pages || pages.length === 0) return false;
    // Check if we have at least some orientations populated
    return Object.keys(pageOrientations).length > 0;
  }, [analyzed, pages, pageOrientations]);

  // Spread configuration for double-page mode
  // When book is not analyzed OR orientations haven't loaded yet, we disable showWideAlone
  // to use simple static spreads (1-2, 3-4, ... or 1, 2-3, 4-5, ... depending on startOnOdd)
  const spreadConfig: SpreadConfig = useMemo(
    () => ({
      totalPages,
      pageOrientations,
      showWideAlone: hasOrientationsLoaded ? doublePageShowWideAlone : false,
      startOnOdd: doublePageStartOnOdd,
      readingDirection,
    }),
    [
      totalPages,
      pageOrientations,
      hasOrientationsLoaded,
      doublePageShowWideAlone,
      doublePageStartOnOdd,
      readingDirection,
    ],
  );

  // Calculate current spread for double-page mode
  const currentSpread = useMemo(() => {
    if (pageLayout !== "double") {
      return { pages: [currentPage], isSinglePage: true };
    }
    return getSpreadPages(currentPage, spreadConfig);
  }, [pageLayout, currentPage, spreadConfig]);

  // Get display order based on reading direction
  const displayPages = useMemo(() => {
    if (pageLayout !== "double") {
      return [{ pageNumber: currentPage, src: getPageUrl(currentPage) }];
    }
    const orderedPages = getDisplayOrder(currentSpread.pages, readingDirection);
    return orderedPages.map((pageNum) => ({
      pageNumber: pageNum,
      src: getPageUrl(pageNum),
    }));
  }, [
    pageLayout,
    currentPage,
    currentSpread.pages,
    readingDirection,
    getPageUrl,
  ]);

  // Display-ordered pages for the spread anchored at `anchorPage`, used to render
  // the previous/next slides of the swipe filmstrip without mutating navigation
  // state. Returns null when there is no such spread (book boundary).
  const buildAdjacentSpread = useCallback(
    (anchorPage: number | null) => {
      if (anchorPage === null) return null;
      if (pageLayout !== "double") {
        return [{ pageNumber: anchorPage, src: getPageUrl(anchorPage) }];
      }
      const spread = getSpreadPages(anchorPage, spreadConfig);
      return getDisplayOrder(spread.pages, readingDirection).map((pageNum) => ({
        pageNumber: pageNum,
        src: getPageUrl(pageNum),
      }));
    },
    [pageLayout, spreadConfig, readingDirection, getPageUrl],
  );

  const prevSpreadPages = useMemo(() => {
    const anchor =
      pageLayout === "double"
        ? getPrevSpreadPage(currentPage, spreadConfig)
        : currentPage > 1
          ? currentPage - 1
          : null;
    return buildAdjacentSpread(anchor);
  }, [pageLayout, currentPage, spreadConfig, buildAdjacentSpread]);

  const nextSpreadPages = useMemo(() => {
    const anchor =
      pageLayout === "double"
        ? getNextSpreadPage(currentPage, spreadConfig)
        : currentPage < totalPages
          ? currentPage + 1
          : null;
    return buildAdjacentSpread(anchor);
  }, [pageLayout, currentPage, totalPages, spreadConfig, buildAdjacentSpread]);

  // Handle page orientation detection callback
  const handlePageOrientationDetected = useCallback(
    (pageNumber: number, orientation: PageOrientation) => {
      setPageOrientation(pageNumber, orientation);
    },
    [setPageOrientation],
  );

  // Spread-aware next page navigation
  const handleSpreadNextPage = useCallback(() => {
    setLastNavigationDirection("next");
    if (pageLayout !== "double") {
      handleNextPage();
      return;
    }

    const nextPage = getNextSpreadPage(currentPage, spreadConfig);
    if (nextPage !== null) {
      goToPage(nextPage);
    } else {
      // At end of book, trigger boundary detection via series navigation
      handleNextPage();
    }
  }, [
    pageLayout,
    currentPage,
    spreadConfig,
    goToPage,
    handleNextPage,
    setLastNavigationDirection,
  ]);

  // Spread-aware previous page navigation
  const handleSpreadPrevPage = useCallback(() => {
    setLastNavigationDirection("prev");
    if (pageLayout !== "double") {
      handlePrevPage();
      return;
    }

    const prevPage = getPrevSpreadPage(currentPage, spreadConfig);
    if (prevPage !== null) {
      goToPage(prevPage);
    } else {
      // At start of book, trigger boundary detection via series navigation
      handlePrevPage();
    }
  }, [
    pageLayout,
    currentPage,
    spreadConfig,
    goToPage,
    handlePrevPage,
    setLastNavigationDirection,
  ]);

  // Paginated boundary navigation (single/double/ttb). Replaces the old
  // two-press toast: paging past the last page raises the "Next Chapter"
  // overlay (and marks the book read); paging before page 1 raises the
  // "Previous Chapter" overlay. While the overlay is up, "next"/"prev" either
  // continue to the adjacent book or dismiss the overlay back to the page.
  const handlePaginatedNext = useCallback(() => {
    const view = useReaderStore.getState().boundaryView;
    if (view !== "none") {
      if (view === "at-end") {
        goToNextBook();
      } else {
        setBoundaryView("none");
      }
      return;
    }

    const atEnd =
      pageLayout === "double"
        ? getNextSpreadPage(currentPage, spreadConfig) === null
        : currentPage >= totalPages;
    if (atEnd) {
      setLastNavigationDirection("next");
      setBoundaryView("at-end");
      // Reaching the end marks the book complete (also fixes double-page
      // spreads whose last spread leaves currentPage short of totalPages).
      if (!incognito) saveProgress(totalPages);
      return;
    }

    handleSpreadNextPage();
  }, [
    pageLayout,
    currentPage,
    totalPages,
    spreadConfig,
    incognito,
    saveProgress,
    goToNextBook,
    setBoundaryView,
    setLastNavigationDirection,
    handleSpreadNextPage,
  ]);

  const handlePaginatedPrev = useCallback(() => {
    const view = useReaderStore.getState().boundaryView;
    if (view !== "none") {
      if (view === "at-start") {
        goToPrevBook();
      } else {
        setBoundaryView("none");
      }
      return;
    }

    const atStart =
      pageLayout === "double"
        ? getPrevSpreadPage(currentPage, spreadConfig) === null
        : currentPage <= 1;
    if (atStart) {
      setLastNavigationDirection("prev");
      setBoundaryView("at-start");
      return;
    }

    handleSpreadPrevPage();
  }, [
    pageLayout,
    currentPage,
    spreadConfig,
    goToPrevBook,
    setBoundaryView,
    setLastNavigationDirection,
    handleSpreadPrevPage,
  ]);

  // Keyboard navigation with series navigation support
  // In continuous/webtoon mode, scroll keys are left to the browser;
  // in double-page mode, use spread-aware navigation;
  // in single-page mode, use wrapped handlers that set navigation direction.
  const isContinuousScroll =
    pageLayout === "continuous" || readingDirection === "webtoon";

  // Swipe (finger-drag) paging applies to horizontal paged modes only. TTB pages
  // vertically, so the horizontal filmstrip doesn't fit; webtoon/continuous scroll.
  const useSwipePager =
    swipeNavigation &&
    !isContinuousScroll &&
    (readingDirection === "ltr" || readingDirection === "rtl");

  // Fit modes that can render a page wider than the viewport, where a horizontal
  // drag must pan the page instead of turning it. Pinch-zoom is handled separately
  // inside SwipePager. (screen/width/width-shrink never overflow horizontally.)
  const isContentHorizontallyPannable = useCallback(
    () => fitMode === "original" || fitMode === "height",
    [fitMode],
  );

  // Identity of the current spread, shared by the transition wrapper and the
  // swipe filmstrip so a committed turn re-centers cleanly.
  const pagedKey =
    pageLayout === "double"
      ? displayPages.map((p) => p.pageNumber).join("-")
      : String(currentPage);

  // Identity of the neighbor spreads, in the same page-number format as
  // `pagedKey`. Used as the filmstrip slide keys so that when a turn commits, the
  // neighbor that becomes the new current is matched by key and its decoded image
  // node is moved into the center rather than re-loaded (which flashes the old
  // page). The next spread's key here equals the current spread's key after the
  // turn, which is exactly what lets React reuse the node.
  const prevPagedKey = prevSpreadPages
    ? prevSpreadPages.map((p) => p.pageNumber).join("-")
    : undefined;
  const nextPagedKey = nextSpreadPages
    ? nextSpreadPages.map((p) => p.pageNumber).join("-")
    : undefined;

  // Render the page content for one filmstrip slide (single page or double spread).
  const renderSpreadSlide = useCallback(
    (slidePages: { pageNumber: number; src: string }[]) =>
      pageLayout === "double" ? (
        <DoublePageSpread
          pages={slidePages}
          fitMode={fitMode}
          backgroundColor={backgroundColor}
          onPageOrientationDetected={handlePageOrientationDetected}
        />
      ) : (
        <ComicReaderPage
          src={slidePages[0].src}
          alt={`Page ${slidePages[0].pageNumber} of ${title}`}
          fitMode={fitMode}
          backgroundColor={backgroundColor}
          onError={handlePageError}
        />
      ),
    [
      pageLayout,
      fitMode,
      backgroundColor,
      handlePageOrientationDetected,
      handlePageError,
      title,
    ],
  );

  useKeyboardNav({
    enabled: !settingsOpened,
    onEscape: onClose,
    scrollContainerRef: isContinuousScroll ? scrollContainerRef : undefined,
    onNextPage: handlePaginatedNext,
    onPrevPage: handlePaginatedPrev,
    onBoundaryEnd: isContinuousScroll ? handleScrollReachedEnd : undefined,
    onBoundaryStart: isContinuousScroll ? handleScrollReachedStart : undefined,
  });

  // Touch/tap navigation for mobile devices.
  // Paginated modes use tap zones (outer thirds page, center toggles toolbar).
  // Continuous-scroll / webtoon modes navigate by scrolling, so the whole
  // surface toggles the toolbar (tapZones: false) — this is the only way to
  // reveal the toolbar on a phone there, since pointer-move is mouse-only.
  const { touchRef } = useTouchNav({
    enabled: !settingsOpened,
    tapZones: !isContinuousScroll,
    onNextPage: handlePaginatedNext,
    onPrevPage: handlePaginatedPrev,
    onTap: toggleToolbar,
  });

  // Preload adjacent pages and track in store
  // Also detect orientation for preloaded images
  useEffect(() => {
    // Build list of pages to preload (current page always included)
    const pagesToPreload = new Set<number>([currentPage]);

    // Floor the prefetch window so cellular readers (and especially
    // downloaded books where every page is a free cache hit) get a snappy
    // next-page tap regardless of the user's preload-pages setting.
    const widePreload = getEffectivePreloadWindow(
      preloadPages,
      isBookDownloaded,
    );

    // Double-page mode doubles the preload count
    const effectivePreload =
      pageLayout === "double" ? widePreload * 2 : widePreload;

    // Preload pages around current position
    for (let i = 1; i <= effectivePreload; i++) {
      pagesToPreload.add(currentPage + i);
      pagesToPreload.add(currentPage - i);
    }

    // In double-page mode, also use spread-aware preloading
    if (pageLayout === "double" && effectivePreload > 0) {
      const spreadPreloadPages = getPreloadPages(
        currentPage,
        spreadConfig,
        effectivePreload,
      );
      for (const p of spreadPreloadPages) {
        pagesToPreload.add(p);
      }
    }

    const validPages = Array.from(pagesToPreload).filter(
      (p) => p >= 1 && p <= totalPages,
    );

    // Preload and track each image, also detect orientation if not already known
    for (const pageNum of validPages) {
      const url = getPageUrl(pageNum);
      const img = new Image();
      img.onload = () => {
        addPreloadedImage(url);
        // Only detect orientation from preloaded image if we don't already have it from backend
        // When hasOrientationsLoaded is true, we already have all page dimensions from the API
        if (!hasOrientationsLoaded) {
          const orientation =
            img.naturalWidth > img.naturalHeight ? "landscape" : "portrait";
          setPageOrientation(pageNum, orientation);
        }
      };
      img.src = url;
    }
  }, [
    currentPage,
    totalPages,
    preloadPages,
    isBookDownloaded,
    pageLayout,
    spreadConfig,
    getPageUrl,
    addPreloadedImage,
    setPageOrientation,
    hasOrientationsLoaded,
  ]);

  // Sync URL query parameter with current page
  // Uses replaceState to avoid polluting browser history
  useEffect(() => {
    if (currentPage > 0 && initializedBookIdRef.current !== null) {
      const url = new URL(window.location.href);
      url.searchParams.set("page", String(currentPage));
      window.history.replaceState(null, "", url.toString());
    }
  }, [currentPage]);

  // Loading state - wait for progress, series settings, and initialization
  // We check currentBookId === bookId to ensure the store has been updated
  // with the correct book and page before rendering
  // Exception: if totalPages is 0, we don't need to wait for initialization
  if (
    progressLoading ||
    !seriesSettingsLoaded ||
    (totalPages > 0 && currentBookId !== bookId)
  ) {
    return (
      <Center
        style={{ width: "100vw", height: "100dvh", backgroundColor: "#000" }}
      >
        <Loader size="lg" color="gray" />
      </Center>
    );
  }

  // No pages
  if (totalPages === 0) {
    return (
      <Center
        style={{ width: "100vw", height: "100dvh", backgroundColor: "#000" }}
      >
        <Text c="dimmed">This book has no pages</Text>
      </Center>
    );
  }

  // Webtoon/continuous transition panels: a "Previous Chapter" panel above the
  // first page and a "Next Chapter" panel after the last. The countdown only
  // runs once the user has actually scrolled the trailing panel into view.
  const panelReadingDirection = readingDirection === "rtl" ? "rtl" : "ltr";
  const webtoonLeadingPanel = (
    <ChapterTransitionPanel
      direction="prev"
      book={adjacentBooks?.prev ?? null}
      onContinue={goToPrevBook}
      readingDirection={panelReadingDirection}
    />
  );
  const webtoonTrailingPanel = (
    <ChapterTransitionPanel
      direction="next"
      book={adjacentBooks?.next ?? null}
      onContinue={goToNextBook}
      autoAdvance={autoAdvanceToNextBook && trailingReached}
      countdownSeconds={autoAdvanceSeconds}
      readingDirection={panelReadingDirection}
    />
  );

  return (
    <Box
      ref={containerRef}
      onPointerMove={handlePointerMove}
      style={{
        width: "100vw",
        height: "100dvh",
        position: "relative",
        overflow: "hidden",
        backgroundColor: "#000",
        // Allow scroll panning but no pinch / double-tap zoom, so the black
        // letterbox around the page can't zoom the UI (Android). iOS is handled
        // by useViewportZoomLock's gesture* preventDefault.
        touchAction: "pan-x pan-y",
      }}
    >
      {/* Toolbar */}
      <ReaderToolbar
        title={title}
        visible={toolbarVisible}
        onClose={onClose}
        onOpenSettings={() => setSettingsOpened(true)}
        prevBook={adjacentBooks?.prev}
        nextBook={adjacentBooks?.next}
        onPrevBook={canGoPrevBook ? goToPrevBook : undefined}
        onNextBook={canGoNextBook ? goToNextBook : undefined}
        fitMode={fitMode}
        onCycleFitMode={handleCycleFitMode}
        pageLayout={pageLayout}
        onTogglePageLayout={handleTogglePageLayout}
        hasSeriesOverride={hasSeriesOverride}
        isContinuousScroll={isContinuousScroll}
      />

      {/* Phone-only bottom navigation. Hidden in continuous/webtoon modes
          where pages are scrolled rather than navigated. */}
      {!isContinuousScroll && (
        <MobileReaderBottomBar
          visible={toolbarVisible}
          onPrevPage={handlePaginatedPrev}
          onNextPage={handlePaginatedNext}
        />
      )}

      {/* First-run hint teaches phone users that center-tap reveals the
          toolbar (CBZ tap zones are left/center/right). Once per session. */}
      <ReaderFirstRunHint />

      {/* Page display - use continuous scroll when pageLayout is continuous OR reading direction is webtoon */}
      {isContinuousScroll ? (
        <ContinuousScrollReader
          bookId={bookId}
          totalPages={totalPages}
          initialPage={currentPage}
          fitMode={fitMode}
          backgroundColor={backgroundColor}
          preloadBuffer={preloadPages}
          pageGap={webtoonPageGap}
          sidePadding={webtoonSidePadding}
          pageDimensions={pageDimensions}
          scrollContainerRef={scrollContainerRef}
          tapRef={touchRef}
          leadingSlot={webtoonLeadingPanel}
          trailingSlot={webtoonTrailingPanel}
          onTrailingReachedChange={setTrailingReached}
        />
      ) : useSwipePager ? (
        // Finger-drag filmstrip owns its own pointer input (tap zones + swipe),
        // so the page renders inside it instead of the PageTransitionWrapper —
        // the strip's snap is the transition, avoiding a double animation.
        <SwipePager
          current={renderSpreadSlide(displayPages)}
          prev={prevSpreadPages ? renderSpreadSlide(prevSpreadPages) : null}
          next={nextSpreadPages ? renderSpreadSlide(nextSpreadPages) : null}
          pageKey={pagedKey}
          prevKey={prevPagedKey}
          nextKey={nextPagedKey}
          readingDirection={readingDirection}
          onNext={handlePaginatedNext}
          onPrev={handlePaginatedPrev}
          onTap={toggleToolbar}
          onExit={onClose}
          enabled={!settingsOpened}
          duration={transitionDuration}
          isContentPannable={isContentHorizontallyPannable}
        />
      ) : (
        <Box
          ref={touchRef}
          style={{
            width: "100%",
            height: "100%",
            // Click-only navigation: we no longer compete with the browser's
            // native gestures. `manipulation` enables pan + pinch-zoom and
            // disables double-tap zoom for snappier taps.
            touchAction: "manipulation",
          }}
        >
          <PageTransitionWrapper
            pageKey={pagedKey}
            transition={pageTransition}
            duration={transitionDuration}
            navigationDirection={lastNavigationDirection}
            readingDirection={readingDirection}
          >
            {pageLayout === "double" ? (
              <DoublePageSpread
                pages={displayPages}
                fitMode={fitMode}
                backgroundColor={backgroundColor}
                onPageOrientationDetected={handlePageOrientationDetected}
              />
            ) : (
              <ComicReaderPage
                src={getPageUrl(currentPage)}
                alt={`Page ${currentPage} of ${title}`}
                fitMode={fitMode}
                backgroundColor={backgroundColor}
                onError={handlePageError}
              />
            )}
          </PageTransitionWrapper>
        </Box>
      )}

      {/* Chapter-transition overlay for paginated modes. Rendered above the
          page view (its own surface captures taps) when the user pages past a
          book boundary. The webtoon/continuous mode uses in-flow panels
          instead, so this only applies to single/double/ttb. */}
      {!isContinuousScroll && boundaryView !== "none" && (
        <Box style={{ position: "absolute", inset: 0, zIndex: 50 }}>
          <ChapterTransitionPanel
            direction={boundaryView === "at-end" ? "next" : "prev"}
            book={
              boundaryView === "at-end"
                ? (adjacentBooks?.next ?? null)
                : (adjacentBooks?.prev ?? null)
            }
            onContinue={boundaryView === "at-end" ? goToNextBook : goToPrevBook}
            autoAdvance={boundaryView === "at-end" && autoAdvanceToNextBook}
            countdownSeconds={autoAdvanceSeconds}
            readingDirection={panelReadingDirection}
          />
        </Box>
      )}

      {/* Settings modal */}
      <ReaderSettings
        opened={settingsOpened}
        onClose={() => setSettingsOpened(false)}
        seriesId={seriesId}
        format={format}
      />
    </Box>
  );
}
