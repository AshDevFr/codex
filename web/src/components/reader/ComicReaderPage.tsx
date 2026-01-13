import { Box, Center, Loader } from "@mantine/core";
import { useState } from "react";
import type { BackgroundColor, FitMode } from "@/store/readerStore";
import { useReaderStore } from "@/store/readerStore";

interface ComicReaderPageProps {
	/** URL of the page image */
	src: string;
	/** Alt text for the image */
	alt: string;
	/** How to fit the image */
	fitMode: FitMode;
	/** Background color */
	backgroundColor: BackgroundColor;
	/** Whether this page is currently visible */
	isVisible?: boolean;
	/** Click handler for navigation zones */
	onClick?: (zone: "left" | "center" | "right") => void;
}

const BACKGROUND_COLORS: Record<BackgroundColor, string> = {
	black: "#000000",
	gray: "#1a1a1a",
	white: "#ffffff",
};

/**
 * Get CSS styles for fit mode.
 * Each mode determines how the image scales within the viewport.
 *
 * - screen: Fit entire page within viewport (no scrolling)
 * - width: Scale to viewport width (may need vertical scroll)
 * - width-shrink: Fit to width, but only shrink larger images (never upscale)
 * - height: Scale to viewport height (may need horizontal scroll)
 * - original: Display at native resolution (1:1 pixels)
 */
function getFitModeStyles(fitMode: FitMode): React.CSSProperties {
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

/**
 * Single page display component for the comic reader.
 *
 * Features:
 * - Fit modes for scaling images
 * - Click zones for navigation (left third, center, right third)
 * - Loading state with spinner
 * - Error handling with retry
 */
export function ComicReaderPage({
	src,
	alt,
	fitMode,
	backgroundColor,
	isVisible = true,
	onClick,
}: ComicReaderPageProps) {
	// Check if this image is already preloaded to avoid showing loader
	const isPreloaded = useReaderStore((state) => state.preloadedImages.has(src));
	const [loadingState, setLoadingState] = useState<{
		src: string;
		isLoading: boolean;
		hasError: boolean;
	}>({ src, isLoading: !isPreloaded, hasError: false });

	// Reset state when src changes
	const isLoading = loadingState.src === src ? loadingState.isLoading : !isPreloaded;
	const hasError = loadingState.src === src ? loadingState.hasError : false;

	const handleImageLoad = () => {
		setLoadingState({ src, isLoading: false, hasError: false });
	};

	const handleImageError = () => {
		setLoadingState({ src, isLoading: false, hasError: true });
	};

	const handleClick = (event: React.MouseEvent<HTMLDivElement>) => {
		if (!onClick) return;

		const rect = event.currentTarget.getBoundingClientRect();
		const x = event.clientX - rect.left;
		const width = rect.width;

		// Divide into thirds: left, center, right
		const third = width / 3;
		if (x < third) {
			onClick("left");
		} else if (x > 2 * third) {
			onClick("right");
		} else {
			onClick("center");
		}
	};

	if (!isVisible) {
		return null;
	}

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
				cursor: onClick ? "pointer" : "default",
				userSelect: "none",
				position: "relative",
			}}
			onClick={handleClick}
		>
			{hasError ? (
				<Center style={{ color: "#666" }}>Failed to load page</Center>
			) : (
				<>
					{/* Always render image - use opacity instead of display:none to allow instant cache hits */}
					<img
						src={src}
						alt={alt}
						style={{
							...getFitModeStyles(fitMode),
							objectFit: "contain",
						}}
						onLoad={handleImageLoad}
						onError={handleImageError}
						draggable={false}
					/>
					{/* Loader overlay - only show when actually loading (not preloaded) */}
					{isLoading && (
						<Center
							style={{
								position: "absolute",
								inset: 0,
								backgroundColor: BACKGROUND_COLORS[backgroundColor],
							}}
						>
							<Loader size="lg" color="gray" />
						</Center>
					)}
				</>
			)}
		</Box>
	);
}
