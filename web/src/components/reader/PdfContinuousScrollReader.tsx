import { Box, Center, Loader, Text } from "@mantine/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Document, Page, pdfjs } from "react-pdf";
import { type BackgroundColor, useReaderStore } from "@/store/readerStore";
import type { PdfZoomLevel } from "./PdfReader";

// Import CSS for text layer and annotation layer
import "react-pdf/dist/esm/Page/TextLayer.css";
import "react-pdf/dist/esm/Page/AnnotationLayer.css";

// Configure PDF.js worker
pdfjs.GlobalWorkerOptions.workerSrc = new URL(
	"pdfjs-dist/build/pdf.worker.min.mjs",
	import.meta.url,
).toString();

// =============================================================================
// Types
// =============================================================================

interface PdfContinuousScrollReaderProps {
	/** Book ID */
	bookId: string;
	/** Total number of pages */
	totalPages: number;
	/** Current page (used for initial scroll position) */
	initialPage?: number;
	/** Zoom level for pages */
	zoomLevel: PdfZoomLevel;
	/** Background color */
	backgroundColor: BackgroundColor;
	/** Gap between pages in pixels */
	pageGap?: number;
	/** Number of pages to preload above/below visible area */
	preloadBuffer?: number;
	/** Search text to highlight */
	searchText?: string;
	/** Callback when the visible page changes (for progress tracking) */
	onPageChange?: (page: number) => void;
	/** Callback when PDF loads */
	onDocumentLoadSuccess?: (pdf: { numPages: number }) => void;
	/** Callback when PDF fails to load */
	onDocumentLoadError?: (error: Error) => void;
}

// =============================================================================
// Constants
// =============================================================================

const BACKGROUND_COLORS: Record<BackgroundColor, string> = {
	black: "#000000",
	gray: "#1a1a1a",
	white: "#ffffff",
};

const DEFAULT_PAGE_GAP = 16;
const DEFAULT_PRELOAD_BUFFER = 2;
const SCROLL_DEBOUNCE_MS = 100;

// =============================================================================
// Component
// =============================================================================

/**
 * Continuous scroll PDF reader for vertical reading.
 *
 * Features:
 * - Vertical scrolling with all pages in a single container
 * - Lazy loading: only renders pages that are visible or near-visible
 * - Intersection Observer for efficient visibility tracking
 * - Scroll-based progress tracking
 * - Text selection and search highlighting
 */
export function PdfContinuousScrollReader({
	bookId,
	totalPages,
	initialPage = 1,
	zoomLevel,
	backgroundColor,
	pageGap = DEFAULT_PAGE_GAP,
	preloadBuffer = DEFAULT_PRELOAD_BUFFER,
	searchText,
	onPageChange,
	onDocumentLoadSuccess,
	onDocumentLoadError,
}: PdfContinuousScrollReaderProps) {
	const containerRef = useRef<HTMLDivElement>(null);
	const pageRefs = useRef<Map<number, HTMLDivElement>>(new Map());
	const observerRef = useRef<IntersectionObserver | null>(null);
	const scrollTimeoutRef = useRef<NodeJS.Timeout | null>(null);
	const hasScrolledToInitialRef = useRef(false);
	const lastReportedPageRef = useRef<number>(0);

	// State
	const [visiblePages, setVisiblePages] = useState<Set<number>>(new Set());
	const [currentVisiblePage, setCurrentVisiblePage] = useState(initialPage);
	const [containerWidth, setContainerWidth] = useState(0);
	const [pdfLoaded, setPdfLoaded] = useState(false);

	// Reader store actions
	const goToPage = useReaderStore((state) => state.goToPage);

	// PDF file URL
	const pdfUrl = useMemo(() => `/api/v1/books/${bookId}/file`, [bookId]);

	// Calculate page dimensions based on zoom level
	const getPageDimensions = useCallback(() => {
		if (!containerWidth) {
			return { width: undefined, height: undefined };
		}

		const padding = 40;
		const availableWidth = containerWidth - padding;

		switch (zoomLevel) {
			case "fit-page":
			case "fit-width":
				return { width: availableWidth, height: undefined };
			case "50%":
				return { scale: 0.5 };
			case "75%":
				return { scale: 0.75 };
			case "100%":
				return { scale: 1.0 };
			case "125%":
				return { scale: 1.25 };
			case "150%":
				return { scale: 1.5 };
			case "200%":
				return { scale: 2.0 };
			default:
				return { width: availableWidth, height: undefined };
		}
	}, [containerWidth, zoomLevel]);

	const pageDimensions = useMemo(() => getPageDimensions(), [getPageDimensions]);

	// Determine which pages should be rendered (visible + buffer)
	const pagesToRender = useMemo(() => {
		if (visiblePages.size === 0) {
			// If no pages visible yet, render around initial page
			const start = Math.max(1, initialPage - preloadBuffer);
			const end = Math.min(totalPages, initialPage + preloadBuffer);
			return new Set(
				Array.from({ length: end - start + 1 }, (_, i) => start + i),
			);
		}

		const minVisible = Math.min(...visiblePages);
		const maxVisible = Math.max(...visiblePages);

		// Render visible pages plus buffer
		const start = Math.max(1, minVisible - preloadBuffer);
		const end = Math.min(totalPages, maxVisible + preloadBuffer);
		return new Set(
			Array.from({ length: end - start + 1 }, (_, i) => start + i),
		);
	}, [visiblePages, initialPage, totalPages, preloadBuffer]);

	// Update container width on resize
	useEffect(() => {
		const updateWidth = () => {
			if (containerRef.current) {
				setContainerWidth(containerRef.current.clientWidth);
			}
		};

		updateWidth();
		window.addEventListener("resize", updateWidth);
		return () => window.removeEventListener("resize", updateWidth);
	}, []);

	// Set up intersection observer
	useEffect(() => {
		const options: IntersectionObserverInit = {
			root: containerRef.current,
			rootMargin: "100px 0px",
			threshold: [0, 0.1, 0.5, 0.9, 1],
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
					const rect = entry.boundingClientRect;
					const containerRect = containerRef.current?.getBoundingClientRect();
					if (containerRect) {
						const visibleTop = Math.max(rect.top, containerRect.top);
						const visibleBottom = Math.min(rect.bottom, containerRect.bottom);
						const visibleHeight = visibleBottom - visibleTop;
						const ratio = visibleHeight / rect.height;

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
		if (!pdfLoaded) return;
		if (initialPage <= 1) {
			hasScrolledToInitialRef.current = true;
			return;
		}

		const targetRef = pageRefs.current.get(initialPage);
		if (targetRef && containerRef.current) {
			hasScrolledToInitialRef.current = true;
			targetRef.scrollIntoView({ behavior: "instant", block: "start" });
		}
	}, [initialPage, pdfLoaded]);

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

	// Handle PDF load success
	const handleDocumentLoadSuccess = useCallback(
		(pdf: { numPages: number }) => {
			setPdfLoaded(true);
			onDocumentLoadSuccess?.(pdf);
		},
		[onDocumentLoadSuccess],
	);

	// Custom text renderer for search highlighting
	const customTextRenderer = useMemo(() => {
		if (!searchText) return undefined;
		return ({ str }: { str: string }) => {
			const regex = new RegExp(
				`(${searchText.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`,
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
		};
	}, [searchText]);

	if (totalPages === 0) {
		return (
			<Center style={{ width: "100%", height: "100vh" }}>
				<Text c="dimmed">This PDF has no pages</Text>
			</Center>
		);
	}

	return (
		<Box
			ref={containerRef}
			data-testid="pdf-continuous-scroll-container"
			style={{
				width: "100%",
				height: "100%",
				overflow: "auto",
				backgroundColor: BACKGROUND_COLORS[backgroundColor],
			}}
		>
			<Document
				file={pdfUrl}
				onLoadSuccess={handleDocumentLoadSuccess}
				onLoadError={onDocumentLoadError}
				loading={
					<Center style={{ width: "100%", height: 400 }}>
						<Loader size="lg" color="gray" />
					</Center>
				}
			>
				<Box
					data-testid="pdf-continuous-scroll-inner"
					style={{
						display: "flex",
						flexDirection: "column",
						alignItems: "center",
						gap: pageGap,
						padding: "20px",
					}}
				>
					{Array.from({ length: totalPages }, (_, i) => {
						const pageNumber = i + 1;
						const shouldRender = pagesToRender.has(pageNumber);

						return (
							<Box
								key={pageNumber}
								ref={(el) => registerPageRef(pageNumber, el)}
								data-page={pageNumber}
								data-testid={`pdf-page-container-${pageNumber}`}
								style={{
									width: "100%",
									minHeight: shouldRender ? undefined : "800px",
									display: "flex",
									justifyContent: "center",
									alignItems: "center",
								}}
							>
								{shouldRender ? (
									<Page
										pageNumber={pageNumber}
										width={pageDimensions.width}
										height={pageDimensions.height}
										scale={"scale" in pageDimensions ? pageDimensions.scale : undefined}
										renderTextLayer={true}
										renderAnnotationLayer={true}
										loading={
											<Center style={{ width: "100%", height: 400 }}>
												<Loader size="md" color="gray" />
											</Center>
										}
										customTextRenderer={customTextRenderer}
									/>
								) : (
									<Box
										data-testid={`pdf-page-placeholder-${pageNumber}`}
										style={{
											width: "100%",
											height: "800px",
											display: "flex",
											justifyContent: "center",
											alignItems: "center",
										}}
									>
										<Text c="dimmed" size="sm">
											Page {pageNumber}
										</Text>
									</Box>
								)}
							</Box>
						);
					})}
				</Box>
			</Document>
		</Box>
	);
}
