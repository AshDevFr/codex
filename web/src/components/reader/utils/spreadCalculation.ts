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
 * Calculate which pages to display for a given current page in double-page mode.
 *
 * The algorithm:
 * 1. If startOnOdd is true, page 1 is displayed alone, then 2-3, 4-5, etc.
 *    This ensures manga covers (typically page 1) are shown alone.
 * 2. If a page is landscape (wide), it's displayed alone.
 * 3. If the next page in a potential pair is landscape, current page is shown alone.
 * 4. If we're at the last page and it's odd-positioned, show it alone.
 *
 * @param currentPage - The current page number (1-indexed)
 * @param config - Spread configuration options
 * @returns SpreadResult with pages to display
 */
export function getSpreadPages(
	currentPage: number,
	config: SpreadConfig,
): SpreadResult {
	const { totalPages, pageOrientations, showWideAlone, startOnOdd } = config;

	// Boundary check
	if (currentPage < 1 || currentPage > totalPages || totalPages === 0) {
		return { pages: [], isSinglePage: true };
	}

	// Check if current page is landscape (show alone)
	if (showWideAlone && isWidePage(currentPage, pageOrientations)) {
		return { pages: [currentPage], isSinglePage: true };
	}

	// Determine if this page should be the left side of a spread
	// If startOnOdd: page 1 alone, then evens are left (2, 4, 6, ...)
	// If !startOnOdd: odds are left (1, 3, 5, ...)
	const isLeftPage = startOnOdd
		? currentPage % 2 === 0 // 2, 4, 6, ... are left pages
		: currentPage % 2 === 1; // 1, 3, 5, ... are left pages

	// Page 1 is always shown alone if startOnOdd is true
	if (startOnOdd && currentPage === 1) {
		return { pages: [currentPage], isSinglePage: true };
	}

	// Calculate the spread based on position
	let leftPage: number;
	let rightPage: number;

	if (isLeftPage) {
		leftPage = currentPage;
		rightPage = currentPage + 1;
	} else {
		// We're on a right page, back up to show the spread
		leftPage = currentPage - 1;
		rightPage = currentPage;
	}

	// Handle edge case: if leftPage is 0 (shouldn't happen with above logic)
	if (leftPage < 1) {
		return { pages: [currentPage], isSinglePage: true };
	}

	// Check if right page exists
	if (rightPage > totalPages) {
		return { pages: [leftPage], isSinglePage: true };
	}

	// Check if right page is landscape (show left alone)
	if (showWideAlone && isWidePage(rightPage, pageOrientations)) {
		return { pages: [leftPage], isSinglePage: true };
	}

	// Return the spread
	return { pages: [leftPage, rightPage], isSinglePage: false };
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
	const currentSpread = getSpreadPages(currentPage, config);

	if (currentSpread.pages.length === 0) {
		return null;
	}

	// Get the last page of the current spread
	const lastPageOfSpread = Math.max(...currentSpread.pages);

	// Next spread starts at the page after the last page of current spread
	const nextPage = lastPageOfSpread + 1;

	if (nextPage > config.totalPages) {
		return null;
	}

	return nextPage;
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
	const currentSpread = getSpreadPages(currentPage, config);

	if (currentSpread.pages.length === 0) {
		return null;
	}

	// Get the first page of the current spread
	const firstPageOfSpread = Math.min(...currentSpread.pages);

	if (firstPageOfSpread <= 1) {
		return null;
	}

	// Previous spread ends at the page before the first page of current spread
	const prevLastPage = firstPageOfSpread - 1;

	// Get the spread that contains this page
	const prevSpread = getSpreadPages(prevLastPage, config);

	if (prevSpread.pages.length === 0) {
		return null;
	}

	// Return the first page of the previous spread
	return Math.min(...prevSpread.pages);
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
	const pagesToPreload = new Set<number>();
	const currentSpread = getSpreadPages(currentPage, config);

	// Add current spread pages
	for (const page of currentSpread.pages) {
		pagesToPreload.add(page);
	}

	// Preload ahead
	let nextPage = currentPage;
	for (let i = 0; i < preloadCount; i++) {
		const next = getNextSpreadPage(nextPage, config);
		if (next === null) break;

		const nextSpread = getSpreadPages(next, config);
		for (const page of nextSpread.pages) {
			pagesToPreload.add(page);
		}
		nextPage = next;
	}

	// Preload behind
	let prevPage = currentPage;
	for (let i = 0; i < preloadCount; i++) {
		const prev = getPrevSpreadPage(prevPage, config);
		if (prev === null) break;

		const prevSpread = getSpreadPages(prev, config);
		for (const page of prevSpread.pages) {
			pagesToPreload.add(page);
		}
		prevPage = prev;
	}

	return Array.from(pagesToPreload).sort((a, b) => a - b);
}
