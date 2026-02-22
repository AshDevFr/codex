import { Box, Center, Loader, Text } from "@mantine/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
  /** Whether this page is visible in the viewport */
  isVisible: boolean;
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
}: ContinuousScrollReaderProps) {
  // Use explicit undefined checks to allow 0 as a valid value
  const effectivePageGap = pageGap ?? DEFAULT_PAGE_GAP;
  const effectivePreloadBuffer = preloadBuffer ?? 0;
  const containerRef = useRef<HTMLDivElement>(null);
  const pageRefs = useRef<Map<number, HTMLDivElement>>(new Map());
  const observerRef = useRef<IntersectionObserver | null>(null);
  const scrollTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const hasScrolledToInitialRef = useRef(false);
  const lastReportedPageRef = useRef<number>(0);

  // Track which pages are visible
  const [visiblePages, setVisiblePages] = useState<Set<number>>(new Set());
  // Track which pages have been loaded
  const [loadedPages, setLoadedPages] = useState<Set<number>>(new Set());
  // Current page based on scroll position (topmost visible page)
  const [currentVisiblePage, setCurrentVisiblePage] = useState(initialPage);

  // Reader store actions
  const goToPage = useReaderStore((state) => state.goToPage);

  // Generate page entries
  const pages: PageEntry[] = useMemo(() => {
    return Array.from({ length: totalPages }, (_, i) => {
      const pageNumber = i + 1;
      return {
        pageNumber,
        src: `/api/v1/books/${bookId}/pages/${pageNumber}`,
        isVisible: visiblePages.has(pageNumber),
        isLoaded: loadedPages.has(pageNumber),
      };
    });
  }, [bookId, totalPages, visiblePages, loadedPages]);

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

  // Set up intersection observer
  useEffect(() => {
    const options: IntersectionObserverInit = {
      root: containerRef.current,
      rootMargin: "100px 0px", // Start loading pages slightly before they enter viewport
      threshold: [0, 0.1, 0.5, 0.9, 1], // Multiple thresholds for better tracking
    };

    observerRef.current = new IntersectionObserver((entries) => {
      const newVisiblePages = new Set(visiblePages);
      let topMostPage = currentVisiblePage;
      let topMostRatio = 0;

      for (const entry of entries) {
        const pageNum = Number(entry.target.getAttribute("data-page"));
        if (Number.isNaN(pageNum)) continue;

        if (entry.isIntersecting) {
          newVisiblePages.add(pageNum);
          // Track which page is most visible at the top
          const rect = entry.boundingClientRect;
          const containerRect = containerRef.current?.getBoundingClientRect();
          if (containerRect) {
            // Calculate how much of the page is visible from the top
            const visibleTop = Math.max(rect.top, containerRect.top);
            const visibleBottom = Math.min(rect.bottom, containerRect.bottom);
            const visibleHeight = visibleBottom - visibleTop;
            const ratio = visibleHeight / rect.height;

            // Prefer pages that are more visible and higher in the viewport
            if (rect.top <= containerRect.top + 100 && ratio > topMostRatio) {
              topMostPage = pageNum;
              topMostRatio = ratio;
            }
          }
        } else {
          newVisiblePages.delete(pageNum);
        }
      }

      setVisiblePages(newVisiblePages);
      if (topMostPage !== currentVisiblePage) {
        setCurrentVisiblePage(topMostPage);
      }
    }, options);

    // Observe all page elements
    const currentObserver = observerRef.current;
    for (const [, element] of pageRefs.current) {
      currentObserver.observe(element);
    }

    return () => {
      currentObserver.disconnect();
    };
  }, [currentVisiblePage, visiblePages]);

  // Report page changes with debouncing
  useEffect(() => {
    if (scrollTimeoutRef.current) {
      clearTimeout(scrollTimeoutRef.current);
    }

    scrollTimeoutRef.current = setTimeout(() => {
      if (currentVisiblePage !== lastReportedPageRef.current) {
        lastReportedPageRef.current = currentVisiblePage;
        goToPage(currentVisiblePage);
        onPageChange?.(currentVisiblePage);
      }
    }, SCROLL_DEBOUNCE_MS);

    return () => {
      if (scrollTimeoutRef.current) {
        clearTimeout(scrollTimeoutRef.current);
      }
    };
  }, [currentVisiblePage, goToPage, onPageChange]);

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

  // Handle image load
  const handleImageLoad = useCallback((pageNumber: number) => {
    setLoadedPages((prev) => new Set([...prev, pageNumber]));
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
      ref={containerRef}
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
