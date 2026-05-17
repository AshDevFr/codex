import { useCallback, useEffect, useRef } from "react";
import {
  selectEffectiveReadingDirection,
  useReaderStore,
} from "@/store/readerStore";
import { classifySwipe, classifyTapZone } from "./swipeGesture";

export interface UseTouchNavOptions {
  /** Whether pointer/touch navigation is enabled */
  enabled?: boolean;
  /** Minimum swipe distance in pixels to trigger navigation (default: 50) */
  minSwipeDistance?: number;
  /** Maximum time in ms for a swipe gesture (default: 300) */
  maxSwipeTime?: number;
  /** Custom handler for next page (overrides default store action) */
  onNextPage?: () => void;
  /** Custom handler for previous page (overrides default store action) */
  onPrevPage?: () => void;
  /** Callback when a center-zone tap is detected (for toolbar toggle). When
   *  `tapZones` is false this fires for taps anywhere on the surface. */
  onTap?: () => void;
  /** Whether taps on the outer thirds navigate (prev/next), with the middle
   *  third reserved for `onTap`. Default true. Set false in continuous-scroll
   *  modes where the whole surface should toggle the toolbar. */
  tapZones?: boolean;
}

interface GestureState {
  pointerId: number | null;
  startX: number;
  startY: number;
  startTime: number;
}

const INITIAL_GESTURE: GestureState = {
  pointerId: null,
  startX: 0,
  startY: 0,
  startTime: 0,
};

/**
 * Hook for tap/swipe navigation in the reader.
 *
 * Uses Pointer Events so a single code path covers touch (finger) **and**
 * mouse (desktop, Chrome mobile-viewport emulation, trackpad drag). Without
 * this, mouse-drag swipes in Chrome DevTools never reach the navigation code
 * unless the user manually enables Sensors > Touch (see R10-3 / R10-4).
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
  tapZones = true,
}: UseTouchNavOptions = {}) {
  const storeNextPage = useReaderStore((state) => state.nextPage);
  const storePrevPage = useReaderStore((state) => state.prevPage);
  const readingDirection = useReaderStore(selectEffectiveReadingDirection);

  // Use custom handlers if provided, otherwise fall back to store actions
  const nextPage = onNextPage ?? storeNextPage;
  const prevPage = onPrevPage ?? storePrevPage;

  const gestureState = useRef<GestureState>({ ...INITIAL_GESTURE });
  const elementRef = useRef<HTMLElement | null>(null);

  const handlePointerDown = useCallback(
    (e: PointerEvent) => {
      if (!enabled) return;

      // Only track the primary pointer; ignore secondary touches, right-click,
      // and middle-click drags.
      if (!e.isPrimary) return;
      if (e.pointerType === "mouse" && e.button !== 0) return;

      gestureState.current = {
        pointerId: e.pointerId,
        startX: e.clientX,
        startY: e.clientY,
        startTime: e.timeStamp || Date.now(),
      };
    },
    [enabled],
  );

  const handlePointerUp = useCallback(
    (e: PointerEvent) => {
      if (!enabled) return;
      const state = gestureState.current;
      if (state.pointerId === null || state.pointerId !== e.pointerId) return;

      const deltaX = e.clientX - state.startX;
      const deltaY = e.clientY - state.startY;
      const deltaTime = (e.timeStamp || Date.now()) - state.startTime;

      gestureState.current = { ...INITIAL_GESTURE };

      const gesture = classifySwipe(deltaX, deltaY, deltaTime, {
        minSwipeDistance,
        maxSwipeTime,
        readingDirection,
      });

      switch (gesture) {
        case "tap": {
          if (!tapZones) {
            onTap?.();
            break;
          }
          // Map the tap location to a zone (prev/center/next) relative to the
          // element. Without an attached element we can't know the geometry,
          // so fall back to a plain toolbar toggle.
          const element = elementRef.current;
          if (!element) {
            onTap?.();
            break;
          }
          const rect = element.getBoundingClientRect();
          const zone = classifyTapZone(
            e.clientX - rect.left,
            e.clientY - rect.top,
            rect.width,
            rect.height,
            { readingDirection },
          );
          if (zone === "center") {
            onTap?.();
          } else if (zone === "next") {
            nextPage();
          } else {
            prevPage();
          }
          break;
        }
        case "next":
          nextPage();
          break;
        case "prev":
          prevPage();
          break;
        case "none":
          break;
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
      tapZones,
    ],
  );

  const handlePointerCancel = useCallback(
    (e: PointerEvent) => {
      const state = gestureState.current;
      if (state.pointerId === null || state.pointerId !== e.pointerId) return;

      const deltaX = e.clientX - state.startX;
      const deltaY = e.clientY - state.startY;
      const deltaTime = (e.timeStamp || Date.now()) - state.startTime;
      gestureState.current = { ...INITIAL_GESTURE };

      if (!enabled) return;

      // iOS WebKit fires pointercancel mid-gesture when it claims a swipe for
      // its own scroll/back-navigation logic. If the user moved far enough to
      // count as a swipe, treat the cancel as the gesture's terminus so users
      // don't have to fight the browser. Taps (negligible movement) are
      // discarded because a canceled tap usually means the browser took the
      // press for something else (text selection, context menu).
      const gesture = classifySwipe(deltaX, deltaY, deltaTime, {
        minSwipeDistance,
        maxSwipeTime,
        readingDirection,
      });

      if (gesture === "next") nextPage();
      else if (gesture === "prev") prevPage();
    },
    [
      enabled,
      minSwipeDistance,
      maxSwipeTime,
      readingDirection,
      nextPage,
      prevPage,
    ],
  );

  // Set ref callback to attach/detach listeners
  const setRef = useCallback(
    (element: HTMLElement | null) => {
      if (elementRef.current) {
        elementRef.current.removeEventListener(
          "pointerdown",
          handlePointerDown,
        );
        elementRef.current.removeEventListener("pointerup", handlePointerUp);
        elementRef.current.removeEventListener(
          "pointercancel",
          handlePointerCancel,
        );
      }

      elementRef.current = element;

      if (element && enabled) {
        // Pointer events are passive by default unless preventDefault() is
        // called; we don't, so listeners stay cheap and don't block scroll.
        element.addEventListener("pointerdown", handlePointerDown);
        element.addEventListener("pointerup", handlePointerUp);
        element.addEventListener("pointercancel", handlePointerCancel);
      }
    },
    [enabled, handlePointerDown, handlePointerUp, handlePointerCancel],
  );

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (elementRef.current) {
        elementRef.current.removeEventListener(
          "pointerdown",
          handlePointerDown,
        );
        elementRef.current.removeEventListener("pointerup", handlePointerUp);
        elementRef.current.removeEventListener(
          "pointercancel",
          handlePointerCancel,
        );
      }
    };
  }, [handlePointerDown, handlePointerUp, handlePointerCancel]);

  return { touchRef: setRef };
}
