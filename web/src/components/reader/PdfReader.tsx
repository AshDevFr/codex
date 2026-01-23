import { Box, Center, Loader, Text, TextInput } from "@mantine/core";
import { useDebouncedValue } from "@mantine/hooks";
import { IconSearch, IconX } from "@tabler/icons-react";
import {
	type CSSProperties,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { Document, Page, pdfjs } from "react-pdf";
import { useReaderStore } from "@/store/readerStore";
import { BoundaryNotification } from "./BoundaryNotification";
import {
	useAdjacentBooks,
	useKeyboardNav,
	useReadProgress,
	useSeriesNavigation,
	useTouchNav,
} from "./hooks";
import { PdfContinuousScrollReader } from "./PdfContinuousScrollReader";
import { PdfReaderSettings } from "./PdfReaderSettings";
import { ReaderToolbar } from "./ReaderToolbar";

// Import CSS for text layer and annotation layer
import "react-pdf/dist/esm/Page/TextLayer.css";
import "react-pdf/dist/esm/Page/AnnotationLayer.css";

// Configure PDF.js worker - use CDN with the exact version bundled in react-pdf
// This avoids version mismatches when pdfjs-dist is also installed as a direct dependency
pdfjs.GlobalWorkerOptions.workerSrc = `//unpkg.com/pdfjs-dist@${pdfjs.version}/build/pdf.worker.min.mjs`;

export type PdfZoomLevel =
	| "fit-page"
	| "fit-width"
	| "50%"
	| "75%"
	| "100%"
	| "125%"
	| "150%"
	| "200%";

export interface PdfReaderProps {
	/** Book ID */
	bookId: string;
	/** Book title */
	title: string;
	/** Total pages (from backend metadata, may differ from actual PDF) */
	totalPages: number;
	/** Starting page from URL parameter */
	startPage?: number;
	/** Incognito mode - when true, progress tracking is disabled */
	incognito?: boolean;
	/** Callback when reader should close */
	onClose: () => void;
}

/**
 * Native PDF reader component using react-pdf (pdf.js).
 *
 * Features:
 * - Native PDF rendering with vector graphics
 * - Text selection and copy
 * - Search within document
 * - Multiple zoom levels
 * - Progress tracking
 */
export function PdfReader({
	bookId,
	title,
	totalPages: _backendTotalPages,
	startPage,
	incognito,
	onClose,
}: PdfReaderProps) {
	const containerRef = useRef<HTMLDivElement>(null);
	const pageContainerRef = useRef<HTMLDivElement>(null);
	const hideTimeoutRef = useRef<NodeJS.Timeout | null>(null);
	const initializedBookIdRef = useRef<string | null>(null);

	// Local state for PDF-specific features
	const [numPages, setNumPages] = useState<number>(0);
	const [pageError, setPageError] = useState<string | null>(null);
	const [settingsOpened, setSettingsOpened] = useState(false);
	const [searchOpen, setSearchOpen] = useState(false);
	const [searchText, setSearchText] = useState("");
	const [debouncedSearchText] = useDebouncedValue(searchText, 300);
	const [boundaryNotification, setBoundaryNotification] = useState<
		string | null
	>(null);
	const [containerDimensions, setContainerDimensions] = useState({
		width: 0,
		height: 0,
	});

	// PDF zoom state (local, not in global store since it's PDF-specific)
	const [zoomLevel, setZoomLevel] = useState<PdfZoomLevel>("fit-page");

	// Cycle through PDF zoom levels (for toolbar fit button)
	const cyclePdfZoom = useCallback(() => {
		setZoomLevel((current) => {
			switch (current) {
				case "fit-page":
					return "fit-width";
				case "fit-width":
					return "100%";
				case "100%":
					return "fit-page";
				default:
					return "fit-page";
			}
		});
	}, []);

	// Map PDF zoom level to FitMode for toolbar display
	const toolbarFitMode = useMemo(() => {
		switch (zoomLevel) {
			case "fit-page":
				return "screen" as const;
			case "fit-width":
				return "width" as const;
			default:
				return "original" as const;
		}
	}, [zoomLevel]);

	// Reader store state
	const currentPage = useReaderStore((state) => state.currentPage);
	const toolbarVisible = useReaderStore((state) => state.toolbarVisible);
	const isFullscreen = useReaderStore((state) => state.isFullscreen);
	const autoHideToolbar = useReaderStore(
		(state) => state.settings.autoHideToolbar,
	);
	const toolbarHideDelay = useReaderStore(
		(state) => state.settings.toolbarHideDelay,
	);
	const backgroundColor = useReaderStore(
		(state) => state.settings.backgroundColor,
	);
	const pdfSpreadMode = useReaderStore((state) => state.settings.pdfSpreadMode);
	const pdfContinuousScroll = useReaderStore(
		(state) => state.settings.pdfContinuousScroll,
	);
	const adjacentBooks = useReaderStore((state) => state.adjacentBooks);
	const boundaryState = useReaderStore((state) => state.boundaryState);

	// Reader store actions
	const initializeReader = useReaderStore((state) => state.initializeReader);
	const setToolbarVisible = useReaderStore((state) => state.setToolbarVisible);
	const setFullscreen = useReaderStore((state) => state.setFullscreen);
	const toggleToolbar = useReaderStore((state) => state.toggleToolbar);

	// Fetch adjacent books for series navigation
	useAdjacentBooks({ bookId, enabled: true });

	// Series navigation with boundary detection
	const {
		handleNextPage: baseNextPage,
		handlePrevPage: basePrevPage,
		goToNextBook,
		goToPrevBook,
		canGoNextBook,
		canGoPrevBook,
	} = useSeriesNavigation({
		onBoundaryChange: (_state, message) => {
			setBoundaryNotification(message);
			setTimeout(() => setBoundaryNotification(null), 3000);
		},
	});

	// Spread-aware navigation: in double-page modes, move by 2 pages
	const handleNextPage = useCallback(() => {
		if (pdfSpreadMode === "single" || pdfContinuousScroll) {
			baseNextPage();
		} else {
			// Double-page mode: advance by 2 pages
			baseNextPage();
			baseNextPage();
		}
	}, [pdfSpreadMode, pdfContinuousScroll, baseNextPage]);

	const handlePrevPage = useCallback(() => {
		if (pdfSpreadMode === "single" || pdfContinuousScroll) {
			basePrevPage();
		} else {
			// Double-page mode: go back by 2 pages
			basePrevPage();
			basePrevPage();
		}
	}, [pdfSpreadMode, pdfContinuousScroll, basePrevPage]);

	// Read progress hook (use numPages from PDF if available, disabled in incognito mode)
	const effectiveTotalPages = numPages > 0 ? numPages : _backendTotalPages;
	const { initialPage, isLoading: progressLoading } = useReadProgress({
		bookId,
		totalPages: effectiveTotalPages,
		enabled: !incognito,
	});

	// Calculate page dimensions based on zoom level
	// Note: PDF scale 1.0 = 72 DPI, which is small on modern displays
	// We use a base scale of 1.5 to make 100% appear at a readable size
	// (roughly equivalent to viewing at 96 DPI on a standard display)
	const BASE_SCALE = 1.5;

	const getPageDimensions = useCallback((): {
		width?: number;
		height?: number;
		scale?: number;
	} => {
		const isDoublePageMode = pdfSpreadMode !== "single";

		// For scale-based zoom levels, we don't need container dimensions
		// This ensures zoom changes work even before container is measured
		switch (zoomLevel) {
			case "50%":
				return { scale: 0.5 * BASE_SCALE };
			case "75%":
				return { scale: 0.75 * BASE_SCALE };
			case "100%":
				return { scale: 1.0 * BASE_SCALE };
			case "125%":
				return { scale: 1.25 * BASE_SCALE };
			case "150%":
				return { scale: 1.5 * BASE_SCALE };
			case "200%":
				return { scale: 2.0 * BASE_SCALE };
		}

		// For fit modes, we need container dimensions
		if (!containerDimensions.width || !containerDimensions.height) {
			// Fallback to scale-based rendering until container is measured
			// Use a smaller scale for double-page mode since we need to fit two pages
			return { scale: isDoublePageMode ? BASE_SCALE * 0.4 : BASE_SCALE * 0.7 };
		}

		const toolbarHeight = 64;
		const padding = 40;
		const gap = isDoublePageMode ? 8 : 0;
		const availableWidth = containerDimensions.width - padding;
		const availableHeight =
			containerDimensions.height - toolbarHeight - padding;

		// For double-page mode, each page gets half the available width
		const perPageWidth = isDoublePageMode
			? Math.floor((availableWidth - gap) / 2)
			: availableWidth;

		// Typical PDF page aspect ratio (US Letter ~8.5x11 = 0.773, A4 ~0.707)
		// Use a conservative ratio that works for most documents
		const typicalPageAspectRatio = 0.75; // width/height

		switch (zoomLevel) {
			case "fit-page": {
				// Fit page: the page should fit within both width and height constraints
				// react-pdf ignores height when width is provided, so we must calculate
				// the width that will result in a page that fits the height
				const widthFromHeight = Math.floor(
					availableHeight * typicalPageAspectRatio,
				);
				// Use the smaller of the two to ensure it fits both constraints
				const fitWidth = Math.min(perPageWidth, widthFromHeight);
				return { width: fitWidth };
			}
			case "fit-width":
				// Fit width: only constrain by width, allow vertical scrolling
				return { width: perPageWidth };
			default:
				return { scale: BASE_SCALE };
		}
	}, [containerDimensions, zoomLevel, pdfSpreadMode]);

	// Initialize reader when PDF loads and progress is ready
	useEffect(() => {
		if (
			!progressLoading &&
			numPages > 0 &&
			initializedBookIdRef.current !== bookId
		) {
			initializedBookIdRef.current = bookId;

			let effectiveStartPage: number;
			if (startPage && startPage >= 1 && startPage <= numPages) {
				effectiveStartPage = startPage;
			} else {
				effectiveStartPage = initialPage;
			}

			initializeReader(bookId, numPages, effectiveStartPage);
		}
	}, [
		bookId,
		numPages,
		startPage,
		initialPage,
		progressLoading,
		initializeReader,
	]);

	// Cleanup on unmount
	useEffect(() => {
		return () => {
			initializedBookIdRef.current = null;
			useReaderStore.getState().resetSession();
		};
	}, []);

	// Update container dimensions on resize
	useEffect(() => {
		const updateDimensions = () => {
			if (containerRef.current) {
				setContainerDimensions({
					width: containerRef.current.clientWidth,
					height: containerRef.current.clientHeight,
				});
			}
		};

		updateDimensions();
		window.addEventListener("resize", updateDimensions);
		return () => window.removeEventListener("resize", updateDimensions);
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

	// Keyboard navigation
	useKeyboardNav({
		enabled: !settingsOpened && !searchOpen,
		onEscape: searchOpen ? () => setSearchOpen(false) : onClose,
		onNextPage: handleNextPage,
		onPrevPage: handlePrevPage,
	});

	// Touch/swipe navigation for mobile devices
	const { touchRef } = useTouchNav({
		enabled: !settingsOpened && !searchOpen,
		onNextPage: handleNextPage,
		onPrevPage: handlePrevPage,
		onTap: toggleToolbar,
	});

	// Combined ref callback for page container (both pageContainerRef and touchRef)
	const setPageContainerRef = useCallback(
		(element: HTMLDivElement | null) => {
			// Update the regular ref
			(
				pageContainerRef as React.MutableRefObject<HTMLDivElement | null>
			).current = element;
			// Update touch ref
			touchRef(element);
		},
		[touchRef],
	);

	// Handle Ctrl+F for search
	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			if ((e.ctrlKey || e.metaKey) && e.key === "f") {
				e.preventDefault();
				setSearchOpen(true);
			}
		};

		document.addEventListener("keydown", handleKeyDown);
		return () => document.removeEventListener("keydown", handleKeyDown);
	}, []);

	// PDF document load success
	const handleDocumentLoadSuccess = useCallback(
		({ numPages: pdfNumPages }: { numPages: number }) => {
			setNumPages(pdfNumPages);
			setPageError(null);
		},
		[],
	);

	// PDF document load error
	const handleDocumentLoadError = useCallback((error: Error) => {
		console.error("Error loading PDF:", error);
		setPageError(error.message || "Failed to load PDF");
	}, []);

	// Page click handler
	const handlePageClick = useCallback(
		(e: React.MouseEvent) => {
			const rect = pageContainerRef.current?.getBoundingClientRect();
			if (!rect) return;

			const x = e.clientX - rect.left;
			const width = rect.width;
			const relativeX = x / width;

			if (relativeX < 0.3) {
				handlePrevPage();
			} else if (relativeX > 0.7) {
				handleNextPage();
			} else {
				toggleToolbar();
			}
		},
		[handlePrevPage, handleNextPage, toggleToolbar],
	);

	// Sync URL query parameter with current page
	useEffect(() => {
		if (currentPage > 0 && initializedBookIdRef.current !== null) {
			const url = new URL(window.location.href);
			url.searchParams.set("page", String(currentPage));
			window.history.replaceState(null, "", url.toString());
		}
	}, [currentPage]);

	// Get PDF file URL
	const pdfUrl = useMemo(() => `/api/v1/books/${bookId}/file`, [bookId]);

	// Page dimensions for rendering
	const pageDimensions = useMemo(
		() => getPageDimensions(),
		[getPageDimensions],
	);

	// Calculate spread page dimensions
	// Note: getPageDimensions already handles double-page mode for fit-page and fit-width
	// We only need to adjust scale-based zoom levels here
	const spreadPageDimensions = useMemo(() => {
		if (pdfSpreadMode === "single") {
			return pageDimensions;
		}
		// For scale-based zoom levels (50%, 75%, 100%, etc.), halve the scale for double-page
		if ("scale" in pageDimensions && pageDimensions.scale !== undefined) {
			return { ...pageDimensions, scale: pageDimensions.scale * 0.5 };
		}
		// For fit-page and fit-width modes, getPageDimensions already calculated
		// the correct per-page dimensions, so just pass through
		return pageDimensions;
	}, [pageDimensions, pdfSpreadMode]);

	// Calculate which pages to display based on spread mode
	const spreadPages = useMemo((): {
		left: number | null;
		right: number | null;
	} => {
		if (pdfSpreadMode === "single") {
			return { left: currentPage, right: null };
		}

		if (pdfSpreadMode === "double") {
			// Double mode: show pages in pairs (1-2, 3-4, etc.)
			// Current page determines the spread
			const isOddPage = currentPage % 2 === 1;
			if (isOddPage) {
				// Odd page is on left
				return {
					left: currentPage,
					right: currentPage + 1 <= numPages ? currentPage + 1 : null,
				};
			} else {
				// Even page - show with previous odd page
				return {
					left: currentPage - 1,
					right: currentPage,
				};
			}
		}

		// double-odd: First page alone, then pairs starting from even pages (2-3, 4-5, etc.)
		// This is typical for books where page 1 is the cover
		if (currentPage === 1) {
			return { left: 1, right: null };
		}
		const isEvenPage = currentPage % 2 === 0;
		if (isEvenPage) {
			// Even page is on left
			return {
				left: currentPage,
				right: currentPage + 1 <= numPages ? currentPage + 1 : null,
			};
		} else {
			// Odd page (except 1) - show with previous even page
			return {
				left: currentPage - 1,
				right: currentPage,
			};
		}
	}, [currentPage, numPages, pdfSpreadMode]);

	// Background color style
	const bgColor = useMemo(() => {
		switch (backgroundColor) {
			case "black":
				return "#000";
			case "gray":
				return "#404040";
			case "white":
				return "#fff";
			default:
				return "#000";
		}
	}, [backgroundColor]);

	// Container style
	const containerStyle: CSSProperties = useMemo(
		() => ({
			width: "100vw",
			height: "100vh",
			position: "relative",
			overflow: "hidden",
			backgroundColor: bgColor,
		}),
		[bgColor],
	);

	// Page container style
	const pageContainerStyle: CSSProperties = useMemo(
		() => ({
			position: "absolute",
			top: toolbarVisible ? 64 : 0,
			left: 0,
			right: 0,
			bottom: 0,
			overflow: "auto",
			display: "flex",
			justifyContent: "center",
			alignItems: "flex-start",
			padding: "20px",
			transition: "top 0.2s ease-in-out",
		}),
		[toolbarVisible],
	);

	// Loading state
	if (progressLoading && numPages === 0) {
		return (
			<Center
				style={{ width: "100vw", height: "100vh", backgroundColor: "#000" }}
			>
				<Loader size="lg" color="gray" />
			</Center>
		);
	}

	return (
		<Box
			ref={containerRef}
			onMouseMove={handleMouseMove}
			style={containerStyle}
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
				fitMode={toolbarFitMode}
				onCycleFitMode={cyclePdfZoom}
			/>

			{/* Boundary notification */}
			<BoundaryNotification
				message={boundaryNotification}
				visible={boundaryState !== "none"}
				type={boundaryState}
			/>

			{/* Search bar (when open) */}
			{searchOpen && (
				<Box
					style={{
						position: "absolute",
						top: toolbarVisible ? 64 : 0,
						left: "50%",
						transform: "translateX(-50%)",
						zIndex: 100,
						backgroundColor: "rgba(0,0,0,0.9)",
						padding: "8px 16px",
						borderRadius: "0 0 8px 8px",
					}}
				>
					<TextInput
						placeholder="Search in PDF..."
						size="sm"
						leftSection={<IconSearch size={16} />}
						rightSection={
							searchText ? (
								<IconX
									size={16}
									style={{ cursor: "pointer" }}
									onClick={() => setSearchText("")}
								/>
							) : null
						}
						value={searchText}
						onChange={(e) => setSearchText(e.target.value)}
						style={{ width: 300 }}
						autoFocus
						onKeyDown={(e) => {
							if (e.key === "Escape") {
								setSearchOpen(false);
							}
						}}
					/>
				</Box>
			)}

			{/* PDF Document - Continuous Scroll or Paginated */}
			{pdfContinuousScroll ? (
				<Box
					style={{
						position: "absolute",
						top: toolbarVisible ? 64 : 0,
						left: 0,
						right: 0,
						bottom: 0,
						transition: "top 0.2s ease-in-out",
					}}
				>
					<PdfContinuousScrollReader
						bookId={bookId}
						totalPages={numPages}
						initialPage={currentPage}
						zoomLevel={zoomLevel}
						backgroundColor={backgroundColor}
						searchText={debouncedSearchText}
						onDocumentLoadSuccess={handleDocumentLoadSuccess}
						onDocumentLoadError={handleDocumentLoadError}
					/>
				</Box>
			) : (
				<Box
					ref={setPageContainerRef}
					onClick={handlePageClick}
					style={{ ...pageContainerStyle, touchAction: "none" }}
				>
					{pageError ? (
						<Center style={{ width: "100%", height: "100%" }}>
							<Text c="red">{pageError}</Text>
						</Center>
					) : (
						<Document
							file={pdfUrl}
							onLoadSuccess={handleDocumentLoadSuccess}
							onLoadError={handleDocumentLoadError}
							loading={
								<Center
									style={{
										width: "100%",
										height: "calc(100vh - 128px)",
										backgroundColor: "transparent",
									}}
								>
									<Loader size="lg" color="gray" />
								</Center>
							}
						>
							<Box
								style={{
									display: "flex",
									flexDirection: "row",
									gap: pdfSpreadMode !== "single" ? "8px" : "0",
									justifyContent: "center",
									alignItems: "flex-start",
								}}
							>
								{/* Left page (or single page) */}
								{spreadPages.left && (
									<Page
										pageNumber={spreadPages.left}
										{...(pdfSpreadMode === "single"
											? pageDimensions
											: spreadPageDimensions)}
										renderTextLayer={true}
										renderAnnotationLayer={true}
										loading={
											<Center
												style={{
													width: 300,
													height: 400,
													backgroundColor: "transparent",
												}}
											>
												<Loader size="md" color="gray" />
											</Center>
										}
										customTextRenderer={
											debouncedSearchText
												? ({ str }) => {
														if (!debouncedSearchText) return str;
														const regex = new RegExp(
															`(${debouncedSearchText.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`,
															"gi",
														);
														const parts = str.split(regex);
														return parts
															.map((part) =>
																regex.test(part)
																	? `<mark style="background-color: yellow; padding: 0;">${part}</mark>`
																	: part,
															)
															.join("");
													}
												: undefined
										}
									/>
								)}
								{/* Right page (only in spread modes) */}
								{spreadPages.right && (
									<Page
										pageNumber={spreadPages.right}
										{...spreadPageDimensions}
										renderTextLayer={true}
										renderAnnotationLayer={true}
										loading={
											<Center
												style={{
													width: 300,
													height: 400,
													backgroundColor: "transparent",
												}}
											>
												<Loader size="md" color="gray" />
											</Center>
										}
										customTextRenderer={
											debouncedSearchText
												? ({ str }) => {
														if (!debouncedSearchText) return str;
														const regex = new RegExp(
															`(${debouncedSearchText.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`,
															"gi",
														);
														const parts = str.split(regex);
														return parts
															.map((part) =>
																regex.test(part)
																	? `<mark style="background-color: yellow; padding: 0;">${part}</mark>`
																	: part,
															)
															.join("");
													}
												: undefined
										}
									/>
								)}
							</Box>
						</Document>
					)}
				</Box>
			)}

			{/* Settings modal */}
			<PdfReaderSettings
				opened={settingsOpened}
				onClose={() => setSettingsOpened(false)}
				zoomLevel={zoomLevel}
				onZoomChange={setZoomLevel}
			/>
		</Box>
	);
}
