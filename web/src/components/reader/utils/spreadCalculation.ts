import type { PageOrientation, ReadingDirection } from "@/store/readerStore";

/**
 * Configuration for spread calculation
 */
export interface SpreadConfig {
  /** Total number of pages in the book */
  totalPages: number;
  /** Page orientations map (page number -> orientation) */
  pageOrientations: Record<number, PageOrientation>;
  /** Whether to show landscape/wide pages alone in double-page mode */
  showWideAlone: boolean;
  /** Whether to start spreads on odd pages (true = page 1 alone, 2-3, 4-5, etc.) */
  startOnOdd: boolean;
  /** Reading direction (affects page order within spread, not spread calculation) */
  readingDirection: ReadingDirection;
}

/**
 * Result of spread calculation
 */
export interface SpreadResult {
  /** Pages to display in this spread (1 or 2 page numbers) */
  pages: number[];
  /** Whether this is a single-page display (landscape or boundary) */
  isSinglePage: boolean;
}

/**
 * A spread entry used for building the spread map
 */
interface SpreadEntry {
  pages: number[];
  startPage: number;
}

/**
 * Detect page orientation from image dimensions.
 * Returns 'landscape' if width > height, 'portrait' otherwise.
 */
export function detectPageOrientation(
  width: number,
  height: number,
): PageOrientation {
  return width > height ? "landscape" : "portrait";
}

/**
 * Check if a page is a wide/landscape page.
 * Returns false if orientation is unknown (not yet loaded).
 */
export function isWidePage(
  pageNumber: number,
  pageOrientations: Record<number, PageOrientation>,
): boolean {
  return pageOrientations[pageNumber] === "landscape";
}

/**
 * Build all spreads for a book by walking through pages sequentially.
 * This properly handles landscape pages that shift the pairing for subsequent pages.
 *
 * @param config - Spread configuration
 * @returns Array of spread entries, each containing the pages in that spread
 */
export function buildAllSpreads(config: SpreadConfig): SpreadEntry[] {
  const { totalPages, pageOrientations, showWideAlone, startOnOdd } = config;

  if (totalPages === 0) {
    return [];
  }

  const spreads: SpreadEntry[] = [];
  let currentPage = 1;

  // Handle first page separately if startOnOdd
  if (startOnOdd && totalPages >= 1) {
    // Page 1 is always shown alone when startOnOdd is true (cover page)
    spreads.push({ pages: [1], startPage: 1 });
    currentPage = 2;
  }

  // Process remaining pages sequentially
  while (currentPage <= totalPages) {
    const isLandscape =
      showWideAlone && isWidePage(currentPage, pageOrientations);

    if (isLandscape) {
      // Landscape page is shown alone
      spreads.push({ pages: [currentPage], startPage: currentPage });
      currentPage++;
    } else {
      // Portrait page - try to pair with next page
      const nextPage = currentPage + 1;

      if (nextPage > totalPages) {
        // Last page, show alone
        spreads.push({ pages: [currentPage], startPage: currentPage });
        currentPage++;
      } else {
        const nextIsLandscape =
          showWideAlone && isWidePage(nextPage, pageOrientations);

        if (nextIsLandscape) {
          // Next page is landscape, show current alone
          spreads.push({ pages: [currentPage], startPage: currentPage });
          currentPage++;
        } else {
          // Both pages are portrait, pair them
          spreads.push({
            pages: [currentPage, nextPage],
            startPage: currentPage,
          });
          currentPage += 2;
        }
      }
    }
  }

  return spreads;
}

/**
 * Find which spread contains the given page.
 *
 * @param page - The page number to find
 * @param spreads - Array of spread entries
 * @returns The spread entry containing the page, or undefined if not found
 */
function findSpreadForPage(
  page: number,
  spreads: SpreadEntry[],
): SpreadEntry | undefined {
  return spreads.find((spread) => spread.pages.includes(page));
}

/**
 * Calculate which pages to display for a given current page in double-page mode.
 *
 * This algorithm walks through pages sequentially from the beginning, properly
 * handling landscape pages that shift the pairing for subsequent pages.
 *
 * Example with startOnOdd=true and page 6 being landscape:
 * - Page 1: alone (cover)
 * - Pages 2-3: spread
 * - Pages 4-5: spread
 * - Page 6: alone (landscape)
 * - Pages 7-8: spread (pairing shifts because of landscape page)
 * - Pages 9-10: spread
 *
 * @param currentPage - The current page number (1-indexed)
 * @param config - Spread configuration options
 * @returns SpreadResult with pages to display
 */
export function getSpreadPages(
  currentPage: number,
  config: SpreadConfig,
): SpreadResult {
  const { totalPages } = config;

  // Boundary check
  if (currentPage < 1 || currentPage > totalPages || totalPages === 0) {
    return { pages: [], isSinglePage: true };
  }

  // Build all spreads and find the one containing the current page
  const spreads = buildAllSpreads(config);
  const spread = findSpreadForPage(currentPage, spreads);

  if (!spread) {
    // Should not happen, but handle gracefully
    return { pages: [currentPage], isSinglePage: true };
  }

  return {
    pages: spread.pages,
    isSinglePage: spread.pages.length === 1,
  };
}

/**
 * Get the display order of pages based on reading direction.
 * In LTR, left page is first. In RTL, right page is first (manga style).
 *
 * @param pages - Array of page numbers from getSpreadPages
 * @param readingDirection - Current reading direction
 * @returns Pages in display order (left to right on screen)
 */
export function getDisplayOrder(
  pages: number[],
  readingDirection: ReadingDirection,
): number[] {
  if (pages.length !== 2) {
    return pages;
  }

  // In LTR: [left, right] stays as is
  // In RTL: [left, right] becomes [right, left] so higher page is on left
  return readingDirection === "rtl" ? [pages[1], pages[0]] : pages;
}

/**
 * Navigate to the next spread from the current page.
 * Returns the first page of the next spread.
 *
 * @param currentPage - Current page number
 * @param config - Spread configuration
 * @returns Next page to navigate to, or null if at end
 */
export function getNextSpreadPage(
  currentPage: number,
  config: SpreadConfig,
): number | null {
  const spreads = buildAllSpreads(config);
  const currentSpreadIndex = spreads.findIndex((spread) =>
    spread.pages.includes(currentPage),
  );

  if (currentSpreadIndex === -1 || currentSpreadIndex >= spreads.length - 1) {
    return null;
  }

  return spreads[currentSpreadIndex + 1].startPage;
}

/**
 * Navigate to the previous spread from the current page.
 * Returns the first page of the previous spread.
 *
 * @param currentPage - Current page number
 * @param config - Spread configuration
 * @returns Previous page to navigate to, or null if at start
 */
export function getPrevSpreadPage(
  currentPage: number,
  config: SpreadConfig,
): number | null {
  const spreads = buildAllSpreads(config);
  const currentSpreadIndex = spreads.findIndex((spread) =>
    spread.pages.includes(currentPage),
  );

  if (currentSpreadIndex <= 0) {
    return null;
  }

  return spreads[currentSpreadIndex - 1].startPage;
}

/**
 * Calculate pages to preload based on current spread and preload count.
 *
 * @param currentPage - Current page number
 * @param config - Spread configuration
 * @param preloadCount - Number of spreads to preload ahead and behind
 * @returns Array of page numbers to preload
 */
export function getPreloadPages(
  currentPage: number,
  config: SpreadConfig,
  preloadCount: number,
): number[] {
  const spreads = buildAllSpreads(config);
  const currentSpreadIndex = spreads.findIndex((spread) =>
    spread.pages.includes(currentPage),
  );

  if (currentSpreadIndex === -1) {
    return [];
  }

  const pagesToPreload = new Set<number>();

  // Calculate range of spreads to preload
  const startIndex = Math.max(0, currentSpreadIndex - preloadCount);
  const endIndex = Math.min(
    spreads.length - 1,
    currentSpreadIndex + preloadCount,
  );

  // Add all pages from spreads in range
  for (let i = startIndex; i <= endIndex; i++) {
    for (const page of spreads[i].pages) {
      pagesToPreload.add(page);
    }
  }

  return Array.from(pagesToPreload).sort((a, b) => a - b);
}
