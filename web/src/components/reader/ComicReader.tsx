import { Box, Center, Loader, Text } from "@mantine/core";
import { useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { booksApi } from "@/api/books";
import {
  type FitMode,
  type PageOrientation,
  selectEffectiveReadingDirection,
  useReaderStore,
} from "@/store/readerStore";
import { BoundaryNotification } from "./BoundaryNotification";
import { ComicReaderPage } from "./ComicReaderPage";
import { ContinuousScrollReader } from "./ContinuousScrollReader";
import { DoublePageSpread } from "./DoublePageSpread";
import {
  useAdjacentBooks,
  useBoundaryNotification,
  useKeyboardNav,
  useReadProgress,
  useSeriesNavigation,
  useSeriesReaderSettings,
  useTouchNav,
} from "./hooks";
import { PageTransitionWrapper } from "./PageTransitionWrapper";
import { ReaderSettings } from "./ReaderSettings";
import { ReaderToolbar } from "./ReaderToolbar";
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
  const hideTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const initializedBookIdRef = useRef<string | null>(null);
  const [settingsOpened, setSettingsOpened] = useState(false);
  const {
    message: boundaryNotification,
    onBoundaryChange,
    clearNotification,
  } = useBoundaryNotification();

  // Per-series settings (forkable settings with series overrides)
  const {
    effectiveSettings,
    isLoaded: seriesSettingsLoaded,
    hasSeriesOverride,
    updateSetting: updateSeriesSetting,
  } = useSeriesReaderSettings(seriesId);

  // Extract forkable settings from effective settings
  const {
    fitMode,
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
  const adjacentBooks = useReaderStore((state) => state.adjacentBooks);
  const boundaryState = useReaderStore((state) => state.boundaryState);
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
  const correctTotalPages = useReaderStore((state) => state.correctTotalPages);
  const setLastNavigationDirection = useReaderStore(
    (state) => state.setLastNavigationDirection,
  );
  const addPreloadedImage = useReaderStore((state) => state.addPreloadedImage);
  const setGlobalFitMode = useReaderStore((state) => state.setFitMode);
  const setGlobalPageLayout = useReaderStore((state) => state.setPageLayout);

  // Fetch adjacent books for series navigation
  useAdjacentBooks({ bookId, enabled: true });

  // Series navigation with boundary detection
  const {
    handleNextPage,
    handlePrevPage,
    goToNextBook,
    goToPrevBook,
    canGoNextBook,
    canGoPrevBook,
    isSeriesEnd,
    isSeriesStart,
  } = useSeriesNavigation({ onBoundaryChange, clearNotification });

  // Read progress hook (disabled in incognito mode)
  const { initialPage, isLoading: progressLoading } = useReadProgress({
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

  // Auto-hide toolbar
  const resetHideTimeout = useCallback(() => {
    if (hideTimeoutRef.current) {
      clearTimeout(hideTimeoutRef.current);
    }

    if (autoHideToolbar && toolbarVisible) {
      hideTimeoutRef.current = setTimeout(() => {
        setToolbarVisible(false);
      }, toolbarHideDelay);
    }
  }, [autoHideToolbar, toolbarVisible, toolbarHideDelay, setToolbarVisible]);

  useEffect(() => {
    resetHideTimeout();
    return () => {
      if (hideTimeoutRef.current) {
        clearTimeout(hideTimeoutRef.current);
      }
    };
  }, [resetHideTimeout]);

  // Show toolbar on mouse move
  const handleMouseMove = useCallback(() => {
    if (!toolbarVisible) {
      setToolbarVisible(true);
    }
    resetHideTimeout();
  }, [toolbarVisible, setToolbarVisible, resetHideTimeout]);

  // Wrapped handlers for single-page mode that set navigation direction
  const handleNextPageWithDirection = useCallback(() => {
    setLastNavigationDirection("next");
    handleNextPage();
  }, [setLastNavigationDirection, handleNextPage]);

  const handlePrevPageWithDirection = useCallback(() => {
    setLastNavigationDirection("prev");
    handlePrevPage();
  }, [setLastNavigationDirection, handlePrevPage]);

  // Cycle fit mode - respects series settings if override exists
  const FIT_MODE_CYCLE: FitMode[] = [
    "screen",
    "width",
    "width-shrink",
    "height",
    "original",
  ];
  const handleCycleFitMode = useCallback(() => {
    const currentIndex = FIT_MODE_CYCLE.indexOf(fitMode);
    const nextIndex = (currentIndex + 1) % FIT_MODE_CYCLE.length;
    const nextMode = FIT_MODE_CYCLE[nextIndex];

    if (hasSeriesOverride) {
      updateSeriesSetting("fitMode", nextMode);
    } else {
      setGlobalFitMode(nextMode);
    }
  }, [fitMode, hasSeriesOverride, updateSeriesSetting, setGlobalFitMode]);

  // Toggle page layout - respects series settings if override exists
  const handleTogglePageLayout = useCallback(() => {
    const newLayout = pageLayout === "single" ? "double" : "single";

    if (hasSeriesOverride) {
      updateSeriesSetting("pageLayout", newLayout);
    } else {
      setGlobalPageLayout(newLayout);
    }
  }, [pageLayout, hasSeriesOverride, updateSeriesSetting, setGlobalPageLayout]);

  // Handle click zones for single-page navigation
  const handleSinglePageClick = useCallback(
    (zone: "left" | "center" | "right") => {
      if (zone === "center") {
        toggleToolbar();
        return;
      }

      // Adjust for reading direction
      // Uses wrapped handlers that set navigation direction for transitions
      if (readingDirection === "ltr") {
        if (zone === "left") handlePrevPageWithDirection();
        if (zone === "right") handleNextPageWithDirection();
      } else {
        if (zone === "left") handleNextPageWithDirection();
        if (zone === "right") handlePrevPageWithDirection();
      }
    },
    [
      readingDirection,
      handleNextPageWithDirection,
      handlePrevPageWithDirection,
      toggleToolbar,
    ],
  );

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

  // Handle click zones for double-page navigation (left/right halves only)
  const handleDoublePageClick = useCallback(
    (zone: "left" | "right") => {
      // In double-page mode, left/right zones navigate spreads
      // Reading direction is already handled in DoublePageSpread component
      if (zone === "left") {
        handleSpreadPrevPage();
      } else {
        handleSpreadNextPage();
      }
    },
    [handleSpreadPrevPage, handleSpreadNextPage],
  );

  // Keyboard navigation with series navigation support
  // In double-page mode, use spread-aware navigation
  // In single-page mode, use wrapped handlers that set navigation direction for transitions
  useKeyboardNav({
    enabled: !settingsOpened,
    onEscape: onClose,
    onNextPage:
      pageLayout === "double"
        ? handleSpreadNextPage
        : handleNextPageWithDirection,
    onPrevPage:
      pageLayout === "double"
        ? handleSpreadPrevPage
        : handlePrevPageWithDirection,
  });

  // Touch/swipe navigation for mobile devices
  // Only enabled for paginated modes (not continuous scroll)
  const { touchRef } = useTouchNav({
    enabled:
      !settingsOpened &&
      pageLayout !== "continuous" &&
      readingDirection !== "webtoon",
    onNextPage:
      pageLayout === "double"
        ? handleSpreadNextPage
        : handleNextPageWithDirection,
    onPrevPage:
      pageLayout === "double"
        ? handleSpreadPrevPage
        : handlePrevPageWithDirection,
    onTap: toggleToolbar,
  });

  // Preload adjacent pages and track in store
  // Also detect orientation for preloaded images
  // In double-page mode, we always preload a few pages ahead to detect orientations
  // before they're needed for spread calculation
  useEffect(() => {
    // Build list of pages to preload (current page + adjacent pages)
    const pagesToPreload = new Set<number>([currentPage]);

    // Always preload pages around current position
    const basePreloadCount = Math.max(preloadPages, 2); // At least 2 pages ahead
    for (let i = 1; i <= basePreloadCount; i++) {
      pagesToPreload.add(currentPage + i);
      pagesToPreload.add(currentPage - i);
    }

    // In double-page mode, also use spread-aware preloading
    if (pageLayout === "double" && preloadPages > 0) {
      const spreadPreloadPages = getPreloadPages(
        currentPage,
        spreadConfig,
        preloadPages * 2,
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
        style={{ width: "100vw", height: "100vh", backgroundColor: "#000" }}
      >
        <Loader size="lg" color="gray" />
      </Center>
    );
  }

  // No pages
  if (totalPages === 0) {
    return (
      <Center
        style={{ width: "100vw", height: "100vh", backgroundColor: "#000" }}
      >
        <Text c="dimmed">This book has no pages</Text>
      </Center>
    );
  }

  return (
    <Box
      ref={containerRef}
      onMouseMove={handleMouseMove}
      style={{
        width: "100vw",
        height: "100vh",
        position: "relative",
        overflow: "hidden",
        backgroundColor: "#000",
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
      />

      {/* Boundary notification */}
      <BoundaryNotification
        message={boundaryNotification}
        visible={boundaryState !== "none"}
        type={boundaryState}
        readingDirection={readingDirection === "rtl" ? "rtl" : "ltr"}
        isSeriesEnd={isSeriesEnd || isSeriesStart}
      />

      {/* Page display - use continuous scroll when pageLayout is continuous OR reading direction is webtoon */}
      {pageLayout === "continuous" || readingDirection === "webtoon" ? (
        <ContinuousScrollReader
          bookId={bookId}
          totalPages={totalPages}
          initialPage={currentPage}
          fitMode={fitMode}
          backgroundColor={backgroundColor}
          preloadBuffer={preloadPages}
          pageGap={webtoonPageGap}
          sidePadding={webtoonSidePadding}
        />
      ) : (
        <Box
          ref={touchRef}
          style={{
            width: "100%",
            height: "100%",
            touchAction: "none", // Prevent browser default touch handling
          }}
        >
          <PageTransitionWrapper
            pageKey={
              pageLayout === "double"
                ? displayPages.map((p) => p.pageNumber).join("-")
                : String(currentPage)
            }
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
                readingDirection={readingDirection}
                onClick={handleDoublePageClick}
                onPageOrientationDetected={handlePageOrientationDetected}
              />
            ) : (
              <ComicReaderPage
                src={getPageUrl(currentPage)}
                alt={`Page ${currentPage} of ${title}`}
                fitMode={fitMode}
                backgroundColor={backgroundColor}
                onClick={handleSinglePageClick}
                onError={handlePageError}
              />
            )}
          </PageTransitionWrapper>
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
