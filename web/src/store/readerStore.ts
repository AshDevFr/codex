import { useEffect, useState } from "react";
import { create } from "zustand";
import { devtools, persist } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";

// =============================================================================
// Types
// =============================================================================

/**
 * Image scaling mode for the reader.
 * - "screen": Fit entire page within viewport (no scrolling needed)
 * - "width": Scale to viewport width (may need vertical scroll)
 * - "width-shrink": Like width, but only shrink larger images (never upscale)
 * - "height": Scale to viewport height (may need horizontal scroll)
 * - "original": Display at native resolution (1:1 pixels)
 */
export type FitMode = "screen" | "width" | "width-shrink" | "height" | "original";
export type PageLayout = "single" | "double" | "continuous";
export type ReadingDirection = "ltr" | "rtl" | "ttb" | "webtoon";
export type BackgroundColor = "black" | "gray" | "white";
export type PdfMode = "streaming" | "native";
export type EpubTheme = "light" | "sepia" | "dark" | "mint" | "slate";
export type EpubFontFamily = "default" | "serif" | "sans-serif" | "monospace" | "dyslexic";
export type BoundaryState = "none" | "at-start" | "at-end";
export type PageTransition = "none" | "fade" | "slide";
export type NavigationDirection = "next" | "prev" | null;

/** Minimal book info needed for series navigation */
export interface AdjacentBook {
	id: string;
	title: string;
	pageCount: number;
}

/** Page orientation detected from image dimensions */
export type PageOrientation = "portrait" | "landscape";

export interface ReaderSettings {
	/** How the image is scaled to fit the viewport */
	fitMode: FitMode;
	/** Page layout mode */
	pageLayout: PageLayout;
	/** Reading direction (left-to-right or right-to-left for manga) */
	readingDirection: ReadingDirection;
	/** Background color behind pages */
	backgroundColor: BackgroundColor;
	/** For PDF files: streaming (image-based) or native (pdf.js) */
	pdfMode: PdfMode;
	/** Auto-hide toolbar after delay */
	autoHideToolbar: boolean;
	/** Toolbar auto-hide delay in ms */
	toolbarHideDelay: number;
	/** EPUB theme (light, sepia, dark, mint, slate) */
	epubTheme: EpubTheme;
	/** EPUB font size as percentage (50-200) */
	epubFontSize: number;
	/** EPUB font family */
	epubFontFamily: EpubFontFamily;
	/** EPUB line height as percentage (100-250) */
	epubLineHeight: number;
	/** EPUB margin as percentage (0-30) */
	epubMargin: number;
	/** Number of pages to preload ahead/behind current page (0-5) */
	preloadPages: number;
	/** In double-page mode, show landscape/wide pages alone (default: true) */
	doublePageShowWideAlone: boolean;
	/** In double-page mode, start spreads on odd pages - good for manga covers (default: true) */
	doublePageStartOnOdd: boolean;
	/** Page transition animation type (none, fade, slide) */
	pageTransition: PageTransition;
	/** Transition duration in milliseconds (50-500) */
	transitionDuration: number;
	/** Webtoon mode: side padding as percentage (0-40 in 5% increments) */
	webtoonSidePadding: number;
	/** Webtoon mode: gap between pages in pixels (0-20 in 5px increments) */
	webtoonPageGap: number;
}

export interface ReaderState {
	// ==========================================================================
	// Settings (persisted)
	// ==========================================================================
	settings: ReaderSettings;

	// ==========================================================================
	// Session state (not persisted)
	// ==========================================================================
	/** Current page number (1-indexed) */
	currentPage: number;
	/** Total pages in the current book */
	totalPages: number;
	/** Whether the book data is loading */
	isLoading: boolean;
	/** Whether the toolbar is visible */
	toolbarVisible: boolean;
	/** Whether fullscreen is active */
	isFullscreen: boolean;
	/** Current book ID being read */
	currentBookId: string | null;
	/** Reading direction override from series/library (null = use default) */
	readingDirectionOverride: ReadingDirection | null;
	/** Adjacent books in the series (prev/next) */
	adjacentBooks: {
		prev: AdjacentBook | null;
		next: AdjacentBook | null;
	} | null;
	/** Current boundary state for series navigation */
	boundaryState: BoundaryState;
	/** Page orientations detected from image dimensions (lazily populated) */
	pageOrientations: Record<number, PageOrientation>;
	/** Last navigation direction for transition animations */
	lastNavigationDirection: NavigationDirection;
	/** Set of image URLs that have been successfully preloaded */
	preloadedImages: Set<string>;

	// ==========================================================================
	// Actions - Settings
	// ==========================================================================
	setFitMode: (mode: FitMode) => void;
	cycleFitMode: () => void;
	setPageLayout: (layout: PageLayout) => void;
	setReadingDirection: (direction: ReadingDirection) => void;
	setBackgroundColor: (color: BackgroundColor) => void;
	cycleBackgroundColor: () => void;
	setPdfMode: (mode: PdfMode) => void;
	setAutoHideToolbar: (enabled: boolean) => void;
	setToolbarHideDelay: (delay: number) => void;
	setEpubTheme: (theme: EpubTheme) => void;
	setEpubFontSize: (size: number) => void;
	setEpubFontFamily: (family: EpubFontFamily) => void;
	setEpubLineHeight: (height: number) => void;
	setEpubMargin: (margin: number) => void;
	setPreloadPages: (count: number) => void;
	setDoublePageShowWideAlone: (enabled: boolean) => void;
	setDoublePageStartOnOdd: (enabled: boolean) => void;
	setPageTransition: (transition: PageTransition) => void;
	setTransitionDuration: (duration: number) => void;
	setWebtoonSidePadding: (padding: number) => void;
	setWebtoonPageGap: (gap: number) => void;

	// ==========================================================================
	// Actions - Navigation
	// ==========================================================================
	setPage: (page: number) => void;
	nextPage: () => void;
	prevPage: () => void;
	firstPage: () => void;
	lastPage: () => void;
	goToPage: (page: number) => void;

	// ==========================================================================
	// Actions - Session
	// ==========================================================================
	initializeReader: (
		bookId: string,
		totalPages: number,
		startPage?: number,
	) => void;
	setReadingDirectionOverride: (direction: ReadingDirection | null) => void;
	setLoading: (loading: boolean) => void;
	setToolbarVisible: (visible: boolean) => void;
	toggleToolbar: () => void;
	setFullscreen: (fullscreen: boolean) => void;
	toggleFullscreen: () => void;
	resetSession: () => void;

	// ==========================================================================
	// Actions - Series Navigation
	// ==========================================================================
	setAdjacentBooks: (books: { prev: AdjacentBook | null; next: AdjacentBook | null } | null) => void;
	setBoundaryState: (state: BoundaryState) => void;
	clearBoundaryState: () => void;

	// ==========================================================================
	// Actions - Page Orientation
	// ==========================================================================
	setPageOrientation: (pageNumber: number, orientation: PageOrientation) => void;
	clearPageOrientations: () => void;

	// ==========================================================================
	// Actions - Navigation Direction (for transitions)
	// ==========================================================================
	setLastNavigationDirection: (direction: NavigationDirection) => void;

	// ==========================================================================
	// Actions - Preloaded Images
	// ==========================================================================
	addPreloadedImage: (url: string) => void;
	isImagePreloaded: (url: string) => boolean;
	clearPreloadedImages: () => void;
}

// =============================================================================
// Default values
// =============================================================================

const DEFAULT_SETTINGS: ReaderSettings = {
	fitMode: "screen",
	pageLayout: "single",
	readingDirection: "ltr",
	backgroundColor: "black",
	pdfMode: "streaming",
	autoHideToolbar: true,
	toolbarHideDelay: 3000,
	epubTheme: "light",
	epubFontSize: 100,
	epubFontFamily: "default",
	epubLineHeight: 140,
	epubMargin: 10,
	preloadPages: 1,
	doublePageShowWideAlone: true,
	doublePageStartOnOdd: true,
	pageTransition: "slide",
	transitionDuration: 200,
	webtoonSidePadding: 0,
	webtoonPageGap: 0,
};

const FIT_MODE_CYCLE: FitMode[] = [
	"screen",
	"width",
	"width-shrink",
	"height",
	"original",
];
const BACKGROUND_CYCLE: BackgroundColor[] = ["black", "gray", "white"];

// =============================================================================
// Store
// =============================================================================

export const useReaderStore = create<ReaderState>()(
	devtools(
		persist(
			immer((set, get) => ({
				// Default settings
				settings: DEFAULT_SETTINGS,

				// Default session state
				currentPage: 1,
				totalPages: 0,
				isLoading: false,
				toolbarVisible: true,
				isFullscreen: false,
				currentBookId: null,
				readingDirectionOverride: null,
				adjacentBooks: null,
				boundaryState: "none" as BoundaryState,
				pageOrientations: {} as Record<number, PageOrientation>,
				lastNavigationDirection: null as NavigationDirection,
				preloadedImages: new Set<string>(),

				// ==========================================================================
				// Settings Actions
				// ==========================================================================

				setFitMode: (mode) =>
					set((state) => {
						state.settings.fitMode = mode;
					}),

				cycleFitMode: () =>
					set((state) => {
						const currentIndex = FIT_MODE_CYCLE.indexOf(state.settings.fitMode);
						const nextIndex = (currentIndex + 1) % FIT_MODE_CYCLE.length;
						state.settings.fitMode = FIT_MODE_CYCLE[nextIndex];
					}),

				setPageLayout: (layout) =>
					set((state) => {
						state.settings.pageLayout = layout;
					}),

				setReadingDirection: (direction) =>
					set((state) => {
						state.settings.readingDirection = direction;
					}),

				setBackgroundColor: (color) =>
					set((state) => {
						state.settings.backgroundColor = color;
					}),

				cycleBackgroundColor: () =>
					set((state) => {
						const currentIndex = BACKGROUND_CYCLE.indexOf(
							state.settings.backgroundColor,
						);
						const nextIndex = (currentIndex + 1) % BACKGROUND_CYCLE.length;
						state.settings.backgroundColor = BACKGROUND_CYCLE[nextIndex];
					}),

				setPdfMode: (mode) =>
					set((state) => {
						state.settings.pdfMode = mode;
					}),

				setAutoHideToolbar: (enabled) =>
					set((state) => {
						state.settings.autoHideToolbar = enabled;
					}),

				setToolbarHideDelay: (delay) =>
					set((state) => {
						state.settings.toolbarHideDelay = delay;
					}),

				setEpubTheme: (theme) =>
					set((state) => {
						state.settings.epubTheme = theme;
					}),

				setEpubFontSize: (size) =>
					set((state) => {
						// Clamp font size between 50% and 200%
						state.settings.epubFontSize = Math.max(50, Math.min(200, size));
					}),

				setEpubFontFamily: (family) =>
					set((state) => {
						state.settings.epubFontFamily = family;
					}),

				setEpubLineHeight: (height) =>
					set((state) => {
						// Clamp line height between 100% and 250%
						state.settings.epubLineHeight = Math.max(100, Math.min(250, height));
					}),

				setEpubMargin: (margin) =>
					set((state) => {
						// Clamp margin between 0% and 30%
						state.settings.epubMargin = Math.max(0, Math.min(30, margin));
					}),

				setPreloadPages: (count) =>
					set((state) => {
						// Clamp preload pages between 0 and 5
						state.settings.preloadPages = Math.max(0, Math.min(5, count));
					}),

				setDoublePageShowWideAlone: (enabled) =>
					set((state) => {
						state.settings.doublePageShowWideAlone = enabled;
					}),

				setDoublePageStartOnOdd: (enabled) =>
					set((state) => {
						state.settings.doublePageStartOnOdd = enabled;
					}),

				setPageTransition: (transition) =>
					set((state) => {
						state.settings.pageTransition = transition;
					}),

				setTransitionDuration: (duration) =>
					set((state) => {
						// Clamp duration between 50ms and 500ms
						state.settings.transitionDuration = Math.max(50, Math.min(500, duration));
					}),

				setWebtoonSidePadding: (padding) =>
					set((state) => {
						// Clamp padding between 0% and 40%
						state.settings.webtoonSidePadding = Math.max(0, Math.min(40, padding));
					}),

				setWebtoonPageGap: (gap) =>
					set((state) => {
						// Clamp gap between 0px and 20px
						state.settings.webtoonPageGap = Math.max(0, Math.min(20, gap));
					}),

				// ==========================================================================
				// Navigation Actions
				// ==========================================================================

				setPage: (page) =>
					set((state) => {
						const { totalPages } = state;
						if (totalPages > 0 && page >= 1 && page <= totalPages) {
							state.currentPage = page;
						}
					}),

				nextPage: () =>
					set((state) => {
						const { currentPage, totalPages } = state;
						if (currentPage < totalPages) {
							state.currentPage = currentPage + 1;
						}
					}),

				prevPage: () =>
					set((state) => {
						const { currentPage } = state;
						if (currentPage > 1) {
							state.currentPage = currentPage - 1;
						}
					}),

				firstPage: () =>
					set((state) => {
						state.currentPage = 1;
					}),

				lastPage: () =>
					set((state) => {
						if (state.totalPages > 0) {
							state.currentPage = state.totalPages;
						}
					}),

				goToPage: (page) => {
					get().setPage(page);
				},

				// ==========================================================================
				// Session Actions
				// ==========================================================================

				initializeReader: (bookId, totalPages, startPage = 1) =>
					set((state) => {
						state.currentBookId = bookId;
						state.totalPages = totalPages;
						state.currentPage = Math.min(Math.max(1, startPage), totalPages);
						state.isLoading = false;
						state.toolbarVisible = true;
					}),

				setReadingDirectionOverride: (direction) =>
					set((state) => {
						state.readingDirectionOverride = direction;
					}),

				setLoading: (loading) =>
					set((state) => {
						state.isLoading = loading;
					}),

				setToolbarVisible: (visible) =>
					set((state) => {
						state.toolbarVisible = visible;
					}),

				toggleToolbar: () =>
					set((state) => {
						state.toolbarVisible = !state.toolbarVisible;
					}),

				setFullscreen: (fullscreen) =>
					set((state) => {
						state.isFullscreen = fullscreen;
					}),

				toggleFullscreen: () =>
					set((state) => {
						state.isFullscreen = !state.isFullscreen;
					}),

				resetSession: () =>
					set((state) => {
						state.currentPage = 1;
						state.totalPages = 0;
						state.isLoading = false;
						state.toolbarVisible = true;
						state.isFullscreen = false;
						state.currentBookId = null;
						state.readingDirectionOverride = null;
						state.adjacentBooks = null;
						state.boundaryState = "none";
						state.pageOrientations = {};
						state.lastNavigationDirection = null;
						state.preloadedImages = new Set<string>();
					}),

				// ==========================================================================
				// Series Navigation Actions
				// ==========================================================================

				setAdjacentBooks: (books) =>
					set((state) => {
						state.adjacentBooks = books;
					}),

				setBoundaryState: (boundaryState) =>
					set((state) => {
						state.boundaryState = boundaryState;
					}),

				clearBoundaryState: () =>
					set((state) => {
						state.boundaryState = "none";
					}),

				// ==========================================================================
				// Page Orientation Actions
				// ==========================================================================

				setPageOrientation: (pageNumber, orientation) =>
					set((state) => {
						state.pageOrientations[pageNumber] = orientation;
					}),

				clearPageOrientations: () =>
					set((state) => {
						state.pageOrientations = {};
					}),

				// ==========================================================================
				// Navigation Direction Actions (for transitions)
				// ==========================================================================

				setLastNavigationDirection: (direction) =>
					set((state) => {
						state.lastNavigationDirection = direction;
					}),

				// ==========================================================================
				// Preloaded Images Actions
				// ==========================================================================

				addPreloadedImage: (url) =>
					set((state) => {
						state.preloadedImages.add(url);
					}),

				isImagePreloaded: (url) => get().preloadedImages.has(url),

				clearPreloadedImages: () =>
					set((state) => {
						state.preloadedImages.clear();
					}),
			})),
			{
				name: "reader-settings-storage",
				// Only persist settings, not session state
				partialize: (state) => ({
					settings: state.settings,
				}),
			},
		),
		{
			name: "ReaderStore",
			enabled: import.meta.env.DEV,
		},
	),
);

// =============================================================================
// Selectors
// =============================================================================

/** Get the effective reading direction (override > default) */
export const selectEffectiveReadingDirection = (
	state: ReaderState,
): ReadingDirection =>
	state.readingDirectionOverride ?? state.settings.readingDirection;

/** Select fit mode */
export const selectFitMode = (state: ReaderState): FitMode =>
	state.settings.fitMode;

/** Select page layout */
export const selectPageLayout = (state: ReaderState): PageLayout =>
	state.settings.pageLayout;

/** Select background color */
export const selectBackgroundColor = (state: ReaderState): BackgroundColor =>
	state.settings.backgroundColor;

/** Select current progress as percentage */
export const selectProgressPercent = (state: ReaderState): number => {
	if (state.totalPages === 0) return 0;
	return Math.round((state.currentPage / state.totalPages) * 100);
};

/** Check if at first page */
export const selectIsFirstPage = (state: ReaderState): boolean =>
	state.currentPage === 1;

/** Check if at last page */
export const selectIsLastPage = (state: ReaderState): boolean =>
	state.currentPage === state.totalPages;

/** Check if there's a previous book in the series */
export const selectHasPrevBook = (state: ReaderState): boolean =>
	state.adjacentBooks?.prev != null;

/** Check if there's a next book in the series */
export const selectHasNextBook = (state: ReaderState): boolean =>
	state.adjacentBooks?.next != null;

/** Get the adjacent books */
export const selectAdjacentBooks = (
	state: ReaderState,
): { prev: AdjacentBook | null; next: AdjacentBook | null } | null =>
	state.adjacentBooks;

/** Get the current boundary state */
export const selectBoundaryState = (state: ReaderState): BoundaryState =>
	state.boundaryState;

/** Select double-page show wide alone setting */
export const selectDoublePageShowWideAlone = (state: ReaderState): boolean =>
	state.settings.doublePageShowWideAlone;

/** Select double-page start on odd setting */
export const selectDoublePageStartOnOdd = (state: ReaderState): boolean =>
	state.settings.doublePageStartOnOdd;

/** Get page orientation for a specific page */
export const selectPageOrientation = (
	state: ReaderState,
	pageNumber: number,
): PageOrientation | undefined => state.pageOrientations[pageNumber];

/** Get all page orientations */
export const selectPageOrientations = (
	state: ReaderState,
): Record<number, PageOrientation> => state.pageOrientations;

/** Select page transition type */
export const selectPageTransition = (state: ReaderState): PageTransition =>
	state.settings.pageTransition;

/** Select transition duration in ms */
export const selectTransitionDuration = (state: ReaderState): number =>
	state.settings.transitionDuration;

/** Get last navigation direction */
export const selectLastNavigationDirection = (
	state: ReaderState,
): NavigationDirection => state.lastNavigationDirection;

// =============================================================================
// Hydration Hook
// =============================================================================

/**
 * Hook that returns true once the store has finished hydrating from localStorage.
 * Use this to prevent flash of default values before persisted state loads.
 */
export function useReaderStoreHydrated(): boolean {
	const [hasHydrated, setHasHydrated] = useState(
		useReaderStore.persist.hasHydrated(),
	);

	useEffect(() => {
		const unsub = useReaderStore.persist.onFinishHydration(() => {
			setHasHydrated(true);
		});
		return unsub;
	}, []);

	return hasHydrated;
}
