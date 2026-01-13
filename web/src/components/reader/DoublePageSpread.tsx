import { Box, Center } from "@mantine/core";
import { useCallback, useState } from "react";
import type {
	BackgroundColor,
	FitMode,
	PageOrientation,
	ReadingDirection,
} from "@/store/readerStore";
import { useReaderStore } from "@/store/readerStore";
import { detectPageOrientation } from "./utils/spreadCalculation";

interface PageState {
	isLoading: boolean;
	hasError: boolean;
}

interface DoublePageSpreadProps {
	/** URLs of the page images to display (1 or 2 pages) */
	pages: Array<{ pageNumber: number; src: string }>;
	/** How to fit the images */
	fitMode: FitMode;
	/** Background color */
	backgroundColor: BackgroundColor;
	/** Reading direction (affects page order in display) */
	readingDirection: ReadingDirection;
	/** Whether this spread is currently visible */
	isVisible?: boolean;
	/** Click handler for navigation zones */
	onClick?: (zone: "left" | "right") => void;
	/** Callback when a page's dimensions are detected */
	onPageOrientationDetected?: (
		pageNumber: number,
		orientation: PageOrientation,
	) => void;
}

const BACKGROUND_COLORS: Record<BackgroundColor, string> = {
	black: "#000000",
	gray: "#1a1a1a",
	white: "#ffffff",
};

/**
 * Get CSS styles for fit mode in double-page spread.
 * Similar to single page but accounts for side-by-side display.
 *
 * - screen: Fit entire page within viewport (no scrolling)
 * - width: Scale to viewport width (may need vertical scroll)
 * - width-shrink: Fit to width, but only shrink larger images (never upscale)
 * - height: Scale to viewport height (may need horizontal scroll)
 * - original: Display at native resolution (1:1 pixels)
 */
function getSpreadFitModeStyles(
	fitMode: FitMode,
	isSinglePage: boolean,
): React.CSSProperties {
	// Single page uses standard fit modes
	if (isSinglePage) {
		switch (fitMode) {
			case "screen":
				// Fit entire page within viewport - scale to fill available space
				return {
					width: "auto",
					height: "100%",
					maxWidth: "100%",
					maxHeight: "100%",
				};
			case "width":
				// Scale to viewport width (may need vertical scroll)
				return {
					width: "100%",
					height: "auto",
					maxHeight: "none",
				};
			case "width-shrink":
				// Fit to width, but only shrink (never upscale small images)
				return {
					maxWidth: "100%",
					width: "auto",
					height: "auto",
					maxHeight: "none",
				};
			case "height":
				// Scale to viewport height (may need horizontal scroll)
				return {
					width: "auto",
					height: "100%",
					maxWidth: "none",
				};
			case "original":
				// Display at native resolution (1:1 pixels)
				return {
					width: "auto",
					height: "auto",
					maxWidth: "none",
					maxHeight: "none",
				};
			default:
				return {
					maxWidth: "100%",
					maxHeight: "100%",
				};
		}
	}

	// Double page: each image fills its container (container handles 50% split)
	switch (fitMode) {
		case "screen":
			// Fit entire spread within viewport - scale to fill container height
			// while respecting width constraint (objectFit: contain handles aspect ratio)
			return {
				width: "auto",
				height: "100%",
				maxWidth: "100%",
				maxHeight: "100%",
			};
		case "width":
			// Scale to viewport width (may need vertical scroll)
			return {
				width: "100%",
				height: "auto",
				maxHeight: "none",
			};
		case "width-shrink":
			// Fit to width, but only shrink (never upscale small images)
			return {
				maxWidth: "100%",
				width: "auto",
				height: "auto",
				maxHeight: "none",
			};
		case "height":
			// Scale to viewport height (may need horizontal scroll)
			return {
				width: "auto",
				height: "100%",
				maxWidth: "100%",
			};
		case "original":
			// Display at native resolution (1:1 pixels)
			return {
				width: "auto",
				height: "auto",
				maxWidth: "none",
				maxHeight: "none",
			};
		default:
			return {
				maxWidth: "100%",
				maxHeight: "100%",
			};
	}
}

/**
 * Double page spread display component for the comic reader.
 *
 * Features:
 * - Displays 1 or 2 pages side-by-side
 * - RTL support (swaps page positions)
 * - Click zones for navigation (left half, right half)
 * - Loading state with spinner
 * - Page orientation detection on load
 */
export function DoublePageSpread({
	pages,
	fitMode,
	backgroundColor,
	readingDirection,
	isVisible = true,
	onClick,
	onPageOrientationDetected,
}: DoublePageSpreadProps) {
	// Get preloaded images to check if pages are already loaded
	const preloadedImages = useReaderStore((state) => state.preloadedImages);

	// Track loading/error state for each page independently
	// Store includes the page src to detect when pages change
	const [pageStates, setPageStates] = useState<Record<string, PageState & { src: string }>>({});

	// Helper to get effective loading state - considers both stored state and preload status
	const getPageState = (pageNumber: number, src: string): PageState => {
		const stored = pageStates[pageNumber];
		// If we have state for this exact src, use it
		if (stored && stored.src === src) {
			return { isLoading: stored.isLoading, hasError: stored.hasError };
		}
		// Otherwise, check if preloaded
		if (preloadedImages.has(src)) {
			return { isLoading: false, hasError: false };
		}
		// Default to loading
		return { isLoading: true, hasError: false };
	};

	const handleImageLoad = useCallback(
		(pageNumber: number, src: string, event: React.SyntheticEvent<HTMLImageElement>) => {
			const img = event.currentTarget;

			// Detect orientation and report it
			if (onPageOrientationDetected) {
				const orientation = detectPageOrientation(
					img.naturalWidth,
					img.naturalHeight,
				);
				onPageOrientationDetected(pageNumber, orientation);
			}

			setPageStates((prev) => ({
				...prev,
				[pageNumber]: { isLoading: false, hasError: false, src },
			}));
		},
		[onPageOrientationDetected],
	);

	const handleImageError = useCallback((pageNumber: number, src: string) => {
		setPageStates((prev) => ({
			...prev,
			[pageNumber]: { isLoading: false, hasError: true, src },
		}));
	}, []);

	const handleClick = (event: React.MouseEvent<HTMLDivElement>) => {
		if (!onClick) return;

		const rect = event.currentTarget.getBoundingClientRect();
		const x = event.clientX - rect.left;
		const width = rect.width;

		// For double-page mode, divide into halves: left half = prev, right half = next
		// In RTL mode, this is reversed
		const isLeftHalf = x < width / 2;

		if (readingDirection === "rtl") {
			// RTL: left half advances (next), right half goes back (prev)
			onClick(isLeftHalf ? "right" : "left");
		} else {
			// LTR: left half goes back (prev), right half advances (next)
			onClick(isLeftHalf ? "left" : "right");
		}
	};

	if (!isVisible) {
		return null;
	}

	const isSinglePage = pages.length === 1;

	// Pages are already ordered by the parent component (ComicReader) via getDisplayOrder()
	// In RTL mode, pages come in as [2, 1] so higher page number displays on the left
	const displayPages = pages;

	return (
		<Box
			style={{
				width: "100%",
				height: "100%",
				backgroundColor: BACKGROUND_COLORS[backgroundColor],
				overflow:
					fitMode === "original" || fitMode === "width" || fitMode === "width-shrink"
						? "auto"
						: "hidden",
				display: "flex",
				alignItems: "center",
				justifyContent: "center",
				gap: 0,
				cursor: onClick ? "pointer" : "default",
				userSelect: "none",
				position: "relative",
			}}
			onClick={handleClick}
			data-testid="double-page-spread"
		>
			{displayPages.map((page, index) => {
				const state = getPageState(page.pageNumber, page.src);
				const { hasError } = state;

				// For double-page spread, align pages toward each other (left page to right edge, right page to left edge)
				const isLeftPage = index === 0;
				const justifyContent = isSinglePage
					? "center"
					: isLeftPage
						? "flex-end"
						: "flex-start";

				return (
					<Box
						key={page.pageNumber}
						style={{
							display: "flex",
							alignItems: "center",
							justifyContent,
							flex: isSinglePage ? "1 1 100%" : "0 1 50%",
							maxWidth: isSinglePage ? "100%" : "50%",
							height: "100%",
							overflow: "hidden",
						}}
						data-testid={`spread-page-${page.pageNumber}`}
					>
						{hasError ? (
							<Center style={{ color: "#666", padding: 20 }}>
								Failed to load page {page.pageNumber}
							</Center>
						) : (
							<img
								src={page.src}
								alt={`Page ${page.pageNumber}`}
								style={{
									...getSpreadFitModeStyles(fitMode, isSinglePage),
									objectFit: "contain",
								}}
								onLoad={(e) => handleImageLoad(page.pageNumber, page.src, e)}
								onError={() => handleImageError(page.pageNumber, page.src)}
								draggable={false}
							/>
						)}
					</Box>
				);
			})}
		</Box>
	);
}
