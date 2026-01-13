import { useCallback, useEffect } from "react";
import {
	selectEffectiveReadingDirection,
	useReaderStore,
} from "@/store/readerStore";

interface UseKeyboardNavOptions {
	/** Whether keyboard navigation is enabled */
	enabled?: boolean;
	/** Callback when escape is pressed */
	onEscape?: () => void;
	/** Custom handler for next page (overrides default store action) */
	onNextPage?: () => void;
	/** Custom handler for previous page (overrides default store action) */
	onPrevPage?: () => void;
}

/**
 * Hook for keyboard navigation in the reader.
 *
 * Supports:
 * - Arrow keys (left/right/up/down) for page navigation
 * - Page Up/Down for page navigation
 * - Space for next page
 * - Home/End for first/last page
 * - F for fullscreen toggle
 * - Escape for exit/close
 *
 * Reading direction is respected:
 * - LTR: Left = previous, Right = next, Up = previous, Down = next
 * - RTL: Left = next, Right = previous, Up = previous, Down = next
 * - TTB: Up = previous, Down = next, Left = previous, Right = next
 */
export function useKeyboardNav({
	enabled = true,
	onEscape,
	onNextPage,
	onPrevPage,
}: UseKeyboardNavOptions = {}) {
	const storeNextPage = useReaderStore((state) => state.nextPage);
	const storePrevPage = useReaderStore((state) => state.prevPage);
	const firstPage = useReaderStore((state) => state.firstPage);
	const lastPage = useReaderStore((state) => state.lastPage);
	const toggleFullscreen = useReaderStore((state) => state.toggleFullscreen);
	const toggleToolbar = useReaderStore((state) => state.toggleToolbar);
	const cycleFitMode = useReaderStore((state) => state.cycleFitMode);
	const readingDirection = useReaderStore(selectEffectiveReadingDirection);

	// Use custom handlers if provided, otherwise fall back to store actions
	const nextPage = onNextPage ?? storeNextPage;
	const prevPage = onPrevPage ?? storePrevPage;

	const handleKeyDown = useCallback(
		(event: KeyboardEvent) => {
			// Don't handle if focus is on an input element
			const target = event.target as HTMLElement;
			if (
				target.tagName === "INPUT" ||
				target.tagName === "TEXTAREA" ||
				target.isContentEditable
			) {
				return;
			}

			// Navigation keys based on reading direction
			// LTR: Right = next, Left = prev
			// RTL: Right = prev, Left = next (reversed horizontal)
			// TTB: Down = next, Up = prev, Left/Right act like LTR
			const isRtl = readingDirection === "rtl";

			switch (event.key) {
				case "ArrowRight":
					event.preventDefault();
					if (isRtl) {
						prevPage();
					} else {
						nextPage();
					}
					break;

				case "ArrowLeft":
					event.preventDefault();
					if (isRtl) {
						nextPage();
					} else {
						prevPage();
					}
					break;

				case "ArrowDown":
				case "PageDown":
				case " ": // Space
					event.preventDefault();
					nextPage();
					break;

				case "ArrowUp":
				case "PageUp":
					event.preventDefault();
					prevPage();
					break;

				case "Home":
					event.preventDefault();
					firstPage();
					break;

				case "End":
					event.preventDefault();
					lastPage();
					break;

				case "f":
				case "F":
					event.preventDefault();
					toggleFullscreen();
					break;

				case "t":
				case "T":
					event.preventDefault();
					toggleToolbar();
					break;

				case "m":
				case "M":
					// Cycle through fit modes
					event.preventDefault();
					cycleFitMode();
					break;

				case "Escape":
					event.preventDefault();
					onEscape?.();
					break;

				default:
					break;
			}
		},
		[
			readingDirection,
			nextPage,
			prevPage,
			firstPage,
			lastPage,
			toggleFullscreen,
			toggleToolbar,
			cycleFitMode,
			onEscape,
		],
	);

	useEffect(() => {
		if (!enabled) return;

		window.addEventListener("keydown", handleKeyDown);
		return () => {
			window.removeEventListener("keydown", handleKeyDown);
		};
	}, [enabled, handleKeyDown]);
}
