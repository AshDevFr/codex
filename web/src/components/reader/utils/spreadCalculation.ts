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
 * Cache for memoized spread calculations.
 * Stores the last computed spreads to avoid recalculating on every page turn.
 */
interface SpreadCache {
  key: string;
  spreads: SpreadEntry[];
}

let spreadCache: SpreadCache | null = null;

/**
 * Generate a cache key from the config.
 * Only includes values that affect spread calculation.
 */
function getCacheKey(config: SpreadConfig): string {
  // Sort orientation keys for consistent ordering
  const orientationKeys = Object.keys(config.pageOrientations)
    .map(Number)
    .sort((a, b) => a - b);
  const orientations = orientationKeys
    .map((k) => `${k}:${config.pageOrientations[k]}`)
    .join(",");

  return `${config.totalPages}|${config.showWideAlone}|${config.startOnOdd}|${orientations}`;
}

/**
 * Get spreads from cache or compute them.
 * Memoizes the result to avoid recalculating on every page turn.
 */
function getCachedSpreads(config: SpreadConfig): SpreadEntry[] {
  const key = getCacheKey(config);

  if (spreadCache && spreadCache.key === key) {
    return spreadCache.spreads;
  }

  const spreads = buildAllSpreadsInternal(config);
  spreadCache = { key, spreads };
  return spreads;
}

/**
 * Build all spreads for a book (memoized).
 * Uses caching to avoid recalculating on every page turn.
 *
 * @param config - Spread configuration
 * @returns Array of spread entries, each containing the pages in that spread
 */
export function buildAllSpreads(config: SpreadConfig): SpreadEntry[] {
  return getCachedSpreads(config);
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
 * Check if a page's orientation is known (loaded).
 */
function isOrientationKnown(
  pageNumber: number,
  pageOrientations: Record<number, PageOrientation>,
): boolean {
  return pageNumber in pageOrientations;
}

/**
 * Check if we can safely pair two pages (both are confirmed portrait).
 * Returns false if either page is wide or has unknown orientation.
 */
function canPairPages(
  page1: number,
  page2: number,
  pageOrientations: Record<number, PageOrientation>,
): boolean {
  // Both pages must have known orientation and be portrait
  return (
    isOrientationKnown(page1, pageOrientations) &&
    isOrientationKnown(page2, pageOrientations) &&
    !isWidePage(page1, pageOrientations) &&
    !isWidePage(page2, pageOrientations)
  );
}

/**
 * Internal implementation of spread building.
 *
 * Two modes based on showWideAlone:
 *
 * 1. showWideAlone=false (simple odd/even pairing):
 *    - startOnOdd=true: 1, 2-3, 4-5, 6-7, ...
 *    - startOnOdd=false: 1-2, 3-4, 5-6, ...
 *
 * 2. showWideAlone=true (wide-page aware pairing):
 *    - Wide pages are shown alone
 *    - Wide pages act as "reset points" for pairing alignment
 *    - Between wide pages (or book boundaries), we pair pages working backward
 *      from the wide page to ensure proper facing-page alignment
 *    - Example with startOnOdd=true and page 17 being wide:
 *      1 (cover), 2 (alone to align), 3-4, 5-6, ..., 15-16, 17 (wide), 18-19, ...
 */
function buildAllSpreadsInternal(config: SpreadConfig): SpreadEntry[] {
  const { totalPages, pageOrientations, showWideAlone, startOnOdd } = config;

  if (totalPages === 0) {
    return [];
  }

  if (!showWideAlone) {
    // Simple mode: static odd/even pairing, no wide page detection
    return buildSimpleSpreads(totalPages, startOnOdd);
  }

  // Wide page mode: build segments between wide pages and align each segment
  return buildWideAwareSpreads(totalPages, pageOrientations, startOnOdd);
}

/**
 * Build simple spreads without wide page detection.
 * Used when showWideAlone=false or as fallback.
 */
function buildSimpleSpreads(
  totalPages: number,
  startOnOdd: boolean,
): SpreadEntry[] {
  const spreads: SpreadEntry[] = [];
  let currentPage = 1;

  // Handle first page separately if startOnOdd (cover page alone)
  if (startOnOdd && totalPages >= 1) {
    spreads.push({ pages: [1], startPage: 1 });
    currentPage = 2;
  }

  while (currentPage <= totalPages) {
    const nextPage = currentPage + 1;
    if (nextPage > totalPages) {
      // Last page, show alone
      spreads.push({ pages: [currentPage], startPage: currentPage });
      currentPage++;
    } else {
      // Pair current and next
      spreads.push({
        pages: [currentPage, nextPage],
        startPage: currentPage,
      });
      currentPage += 2;
    }
  }

  return spreads;
}

/**
 * Build spreads with wide page awareness.
 * Wide pages act as alignment boundaries - we work backward from each wide page
 * to ensure proper facing-page alignment.
 */
function buildWideAwareSpreads(
  totalPages: number,
  pageOrientations: Record<number, "portrait" | "landscape">,
  startOnOdd: boolean,
): SpreadEntry[] {
  // First, identify all wide pages and their positions
  const widePages: number[] = [];
  for (let p = 1; p <= totalPages; p++) {
    if (isWidePage(p, pageOrientations)) {
      widePages.push(p);
    }
  }

  // Build spreads for each segment between boundaries
  // Boundaries are: start of book, wide pages, end of book
  const spreads: SpreadEntry[] = [];

  // Determine starting point (after cover if startOnOdd)
  const effectiveStart = startOnOdd ? 2 : 1;

  // Add cover page if startOnOdd
  if (startOnOdd && totalPages >= 1) {
    spreads.push({ pages: [1], startPage: 1 });
  }

  // Process segments between wide pages
  let segmentStart = effectiveStart;

  for (const widePage of widePages) {
    // Build spreads for segment before this wide page
    // This segment needs backward alignment (align to the wide page)
    if (segmentStart < widePage) {
      const segmentSpreads = buildSegmentSpreadsBackward(
        segmentStart,
        widePage - 1,
        pageOrientations,
      );
      spreads.push(...segmentSpreads);
    }

    // Add the wide page itself
    spreads.push({ pages: [widePage], startPage: widePage });
    segmentStart = widePage + 1;
  }

  // Build spreads for final segment (after last wide page to end of book)
  // This segment uses forward alignment (pair from start)
  if (segmentStart <= totalPages) {
    const segmentSpreads = buildSegmentSpreadsForward(
      segmentStart,
      totalPages,
      pageOrientations,
    );
    spreads.push(...segmentSpreads);
  }

  // Sort by start page to ensure correct order
  spreads.sort((a, b) => a.startPage - b.startPage);

  return spreads;
}

/**
 * Build spreads for a segment that precedes a wide page.
 * Uses backward alignment: if odd page count, the FIRST page is shown alone
 * so that the last page of the segment is paired (properly aligned before the wide page).
 *
 * Example: segment 2-16 before wide page 17
 * - 15 pages (odd count)
 * - Result: 2 (alone), 3-4, 5-6, 7-8, 9-10, 11-12, 13-14, 15-16
 */
function buildSegmentSpreadsBackward(
  start: number,
  end: number,
  pageOrientations: Record<number, "portrait" | "landscape">,
): SpreadEntry[] {
  const pageCount = end - start + 1;

  if (pageCount <= 0) {
    return [];
  }

  if (pageCount === 1) {
    return [{ pages: [start], startPage: start }];
  }

  const spreads: SpreadEntry[] = [];

  if (pageCount % 2 === 0) {
    // Even count: pair all pages from start
    for (let p = start; p <= end; p += 2) {
      const page1 = p;
      const page2 = p + 1;
      if (canPairPages(page1, page2, pageOrientations)) {
        spreads.push({ pages: [page1, page2], startPage: page1 });
      } else {
        spreads.push({ pages: [page1], startPage: page1 });
        spreads.push({ pages: [page2], startPage: page2 });
      }
    }
  } else {
    // Odd count: first page alone, then pair the rest
    spreads.push({ pages: [start], startPage: start });
    for (let p = start + 1; p <= end; p += 2) {
      const page1 = p;
      const page2 = p + 1;
      if (page2 <= end && canPairPages(page1, page2, pageOrientations)) {
        spreads.push({ pages: [page1, page2], startPage: page1 });
      } else if (page2 <= end) {
        spreads.push({ pages: [page1], startPage: page1 });
        spreads.push({ pages: [page2], startPage: page2 });
      } else {
        spreads.push({ pages: [page1], startPage: page1 });
      }
    }
  }

  return spreads;
}

/**
 * Build spreads for a segment that follows a wide page (or is at the end of book).
 * Uses forward alignment: pair from the start, if odd count the LAST page is shown alone.
 *
 * Example: segment 2-10 (no wide pages in book, startOnOdd=true)
 * - 9 pages (odd count)
 * - Result: 2-3, 4-5, 6-7, 8-9, 10 (alone)
 */
function buildSegmentSpreadsForward(
  start: number,
  end: number,
  pageOrientations: Record<number, "portrait" | "landscape">,
): SpreadEntry[] {
  const pageCount = end - start + 1;

  if (pageCount <= 0) {
    return [];
  }

  if (pageCount === 1) {
    return [{ pages: [start], startPage: start }];
  }

  const spreads: SpreadEntry[] = [];

  // Pair from the start, last page alone if odd count
  for (let p = start; p <= end; p += 2) {
    const page1 = p;
    const page2 = p + 1;
    if (page2 <= end && canPairPages(page1, page2, pageOrientations)) {
      spreads.push({ pages: [page1, page2], startPage: page1 });
    } else if (page2 <= end) {
      // Can't pair, show both alone
      spreads.push({ pages: [page1], startPage: page1 });
      spreads.push({ pages: [page2], startPage: page2 });
    } else {
      // Last page alone (odd count)
      spreads.push({ pages: [page1], startPage: page1 });
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
