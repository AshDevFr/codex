import { Box, Center, Loader, Text } from "@mantine/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
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
	/** Starting page from URL parameter (overrides saved progress) */
	startPage?: number;
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
	format: _format,
	readingDirectionOverride,
	startPage,
	onClose,
}: ComicReaderProps) {
	const containerRef = useRef<HTMLDivElement>(null);
	const hideTimeoutRef = useRef<NodeJS.Timeout | null>(null);
	const initializedBookIdRef = useRef<string | null>(null);
	const [settingsOpened, setSettingsOpened] = useState(false);
	const [boundaryNotification, setBoundaryNotification] = useState<string | null>(null);

	// Per-series settings (forkable settings with series overrides)
	const { effectiveSettings, isLoaded: seriesSettingsLoaded } = useSeriesReaderSettings(seriesId);

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
	const setLastNavigationDirection = useReaderStore(
		(state) => state.setLastNavigationDirection,
	);
	const addPreloadedImage = useReaderStore((state) => state.addPreloadedImage);

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
	} = useSeriesNavigation({
		onBoundaryChange: (_state, message) => {
			setBoundaryNotification(message);
			// Auto-hide notification after 3 seconds
			setTimeout(() => setBoundaryNotification(null), 3000);
		},
	});

	// Read progress hook
	const { initialPage, isLoading: progressLoading } = useReadProgress({
		bookId,
		totalPages,
		enabled: true,
	});

	// Initialize reader when progress loads and we haven't initialized this book yet
	// Track by bookId to handle navigation between different books
	useEffect(() => {
		if (
			!progressLoading &&
			totalPages > 0 &&
			initializedBookIdRef.current !== bookId
		) {
			initializedBookIdRef.current = bookId;

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
		}
	}, [
		bookId,
		totalPages,
		startPage,
		initialPage,
		progressLoading,
		readingDirectionOverride,
		initializeReader,
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
		[readingDirection, handleNextPageWithDirection, handlePrevPageWithDirection, toggleToolbar],
	);

	// Generate page URL
	const getPageUrl = useCallback(
		(pageNumber: number) => {
			return `/api/v1/books/${bookId}/pages/${pageNumber}`;
		},
		[bookId],
	);

	// Spread configuration for double-page mode
	const spreadConfig: SpreadConfig = useMemo(
		() => ({
			totalPages,
			pageOrientations,
			showWideAlone: doublePageShowWideAlone,
			startOnOdd: doublePageStartOnOdd,
			readingDirection,
		}),
		[
			totalPages,
			pageOrientations,
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
	}, [pageLayout, currentPage, currentSpread.pages, readingDirection, getPageUrl]);

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
	}, [pageLayout, currentPage, spreadConfig, goToPage, handleNextPage, setLastNavigationDirection]);

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
	}, [pageLayout, currentPage, spreadConfig, goToPage, handlePrevPage, setLastNavigationDirection]);

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
		onNextPage: pageLayout === "double" ? handleSpreadNextPage : handleNextPageWithDirection,
		onPrevPage: pageLayout === "double" ? handleSpreadPrevPage : handlePrevPageWithDirection,
	});

	// Touch/swipe navigation for mobile devices
	// Only enabled for paginated modes (not continuous scroll)
	const { touchRef } = useTouchNav({
		enabled: !settingsOpened && pageLayout !== "continuous" && readingDirection !== "webtoon",
		onNextPage: pageLayout === "double" ? handleSpreadNextPage : handleNextPageWithDirection,
		onPrevPage: pageLayout === "double" ? handleSpreadPrevPage : handlePrevPageWithDirection,
		onTap: toggleToolbar,
	});

	// Preload adjacent pages (spread-aware) and track in store
	useEffect(() => {
		// Build list of pages to preload (current page + adjacent pages)
		let pagesToPreload: number[] = [currentPage];

		if (preloadPages > 0) {
			if (pageLayout === "double") {
				// Use spread-aware preloading - double the count since each "page" in settings
				// should mean one spread (2 pages) in double-page mode
				pagesToPreload = [...pagesToPreload, ...getPreloadPages(currentPage, spreadConfig, preloadPages * 2)];
			} else {
				// Single page preloading
				for (let i = 1; i <= preloadPages; i++) {
					pagesToPreload.push(currentPage - i, currentPage + i);
				}
			}
		}

		const validPages = pagesToPreload.filter(
			(p) => p >= 1 && p <= totalPages,
		);

		// Preload and track each image
		for (const pageNum of validPages) {
			const url = getPageUrl(pageNum);
			const img = new Image();
			img.onload = () => {
				addPreloadedImage(url);
			};
			img.src = url;
		}
	}, [currentPage, totalPages, preloadPages, pageLayout, spreadConfig, getPageUrl, addPreloadedImage]);

	// Sync URL query parameter with current page
	// Uses replaceState to avoid polluting browser history
	useEffect(() => {
		if (currentPage > 0 && initializedBookIdRef.current !== null) {
			const url = new URL(window.location.href);
			url.searchParams.set("page", String(currentPage));
			window.history.replaceState(null, "", url.toString());
		}
	}, [currentPage]);

	// Loading state - wait for both progress and series settings to load
	if (progressLoading || !seriesSettingsLoaded) {
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
			/>

			{/* Boundary notification */}
			<BoundaryNotification
				message={boundaryNotification}
				visible={boundaryState !== "none"}
				type={boundaryState}
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
						pageKey={pageLayout === "double" ? displayPages.map(p => p.pageNumber).join("-") : String(currentPage)}
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
			/>
		</Box>
	);
}
