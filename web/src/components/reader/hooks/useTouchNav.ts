import { useCallback, useEffect, useRef } from "react";
import {
	selectEffectiveReadingDirection,
	useReaderStore,
} from "@/store/readerStore";

export interface UseTouchNavOptions {
	/** Whether touch navigation is enabled */
	enabled?: boolean;
	/** Minimum swipe distance in pixels to trigger navigation (default: 50) */
	minSwipeDistance?: number;
	/** Maximum time in ms for a swipe gesture (default: 300) */
	maxSwipeTime?: number;
	/** Custom handler for next page (overrides default store action) */
	onNextPage?: () => void;
	/** Custom handler for previous page (overrides default store action) */
	onPrevPage?: () => void;
	/** Callback when a tap is detected (for toolbar toggle) */
	onTap?: () => void;
}

interface TouchState {
	startX: number;
	startY: number;
	startTime: number;
	isTracking: boolean;
}

/**
 * Hook for touch/swipe navigation in the reader.
 *
 * Supports:
 * - Horizontal swipes for page navigation
 * - Vertical swipes for page navigation (TTB/webtoon modes)
 * - Tap detection for toolbar toggle
 *
 * Reading direction is respected:
 * - LTR: Swipe left = next, Swipe right = prev
 * - RTL: Swipe left = prev, Swipe right = next
 * - TTB/Webtoon: Swipe up = next, Swipe down = prev
 *
 * @returns ref to attach to the touchable element
 */
export function useTouchNav({
	enabled = true,
	minSwipeDistance = 50,
	maxSwipeTime = 300,
	onNextPage,
	onPrevPage,
	onTap,
}: UseTouchNavOptions = {}) {
	const storeNextPage = useReaderStore((state) => state.nextPage);
	const storePrevPage = useReaderStore((state) => state.prevPage);
	const readingDirection = useReaderStore(selectEffectiveReadingDirection);

	// Use custom handlers if provided, otherwise fall back to store actions
	const nextPage = onNextPage ?? storeNextPage;
	const prevPage = onPrevPage ?? storePrevPage;

	// Track touch state
	const touchState = useRef<TouchState>({
		startX: 0,
		startY: 0,
		startTime: 0,
		isTracking: false,
	});

	// Element ref for attaching listeners
	const elementRef = useRef<HTMLElement | null>(null);

	const handleTouchStart = useCallback(
		(e: TouchEvent) => {
			if (!enabled) return;

			const touch = e.touches[0];
			touchState.current = {
				startX: touch.clientX,
				startY: touch.clientY,
				startTime: Date.now(),
				isTracking: true,
			};
		},
		[enabled],
	);

	const handleTouchEnd = useCallback(
		(e: TouchEvent) => {
			if (!enabled || !touchState.current.isTracking) return;

			const touch = e.changedTouches[0];
			const { startX, startY, startTime } = touchState.current;

			const deltaX = touch.clientX - startX;
			const deltaY = touch.clientY - startY;
			const deltaTime = Date.now() - startTime;

			// Reset tracking
			touchState.current.isTracking = false;

			// Check if it's within time limit for a swipe
			if (deltaTime > maxSwipeTime) {
				return;
			}

			const absX = Math.abs(deltaX);
			const absY = Math.abs(deltaY);

			// Determine if this is primarily a horizontal or vertical swipe
			const isHorizontalSwipe = absX > absY && absX >= minSwipeDistance;
			const isVerticalSwipe = absY > absX && absY >= minSwipeDistance;

			// Check for tap (minimal movement)
			if (absX < 10 && absY < 10) {
				onTap?.();
				return;
			}

			// Handle based on reading direction
			const isVerticalMode =
				readingDirection === "ttb" || readingDirection === "webtoon";
			const isRtl = readingDirection === "rtl";

			if (isVerticalMode) {
				// TTB/Webtoon: vertical swipes control navigation
				if (isVerticalSwipe) {
					if (deltaY < 0) {
						// Swipe up = next page
						nextPage();
					} else {
						// Swipe down = prev page
						prevPage();
					}
				}
			} else {
				// LTR/RTL: horizontal swipes control navigation
				if (isHorizontalSwipe) {
					if (isRtl) {
						// RTL: reversed
						if (deltaX < 0) {
							prevPage();
						} else {
							nextPage();
						}
					} else {
						// LTR: normal
						if (deltaX < 0) {
							nextPage();
						} else {
							prevPage();
						}
					}
				}
			}
		},
		[
			enabled,
			minSwipeDistance,
			maxSwipeTime,
			readingDirection,
			nextPage,
			prevPage,
			onTap,
		],
	);

	const handleTouchCancel = useCallback(() => {
		touchState.current.isTracking = false;
	}, []);

	// Set ref callback to attach/detach listeners
	const setRef = useCallback(
		(element: HTMLElement | null) => {
			// Remove listeners from previous element
			if (elementRef.current) {
				elementRef.current.removeEventListener("touchstart", handleTouchStart);
				elementRef.current.removeEventListener("touchend", handleTouchEnd);
				elementRef.current.removeEventListener(
					"touchcancel",
					handleTouchCancel,
				);
			}

			elementRef.current = element;

			// Add listeners to new element
			if (element && enabled) {
				element.addEventListener("touchstart", handleTouchStart, {
					passive: true,
				});
				element.addEventListener("touchend", handleTouchEnd, { passive: true });
				element.addEventListener("touchcancel", handleTouchCancel, {
					passive: true,
				});
			}
		},
		[enabled, handleTouchStart, handleTouchEnd, handleTouchCancel],
	);

	// Cleanup on unmount
	useEffect(() => {
		return () => {
			if (elementRef.current) {
				elementRef.current.removeEventListener("touchstart", handleTouchStart);
				elementRef.current.removeEventListener("touchend", handleTouchEnd);
				elementRef.current.removeEventListener(
					"touchcancel",
					handleTouchCancel,
				);
			}
		};
	}, [handleTouchStart, handleTouchEnd, handleTouchCancel]);

	return { touchRef: setRef };
}
