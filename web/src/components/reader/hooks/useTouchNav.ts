import { useCallback, useEffect, useLayoutEffect, useRef } from "react";
import {
  selectEffectiveReadingDirection,
  useReaderStore,
} from "@/store/readerStore";
import { classifyTapZone, isTap, TAP_TOLERANCE } from "./swipeGesture";

export interface UseTouchNavOptions {
  /** Whether pointer/touch navigation is enabled */
  enabled?: boolean;
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
}

const INITIAL_GESTURE: GestureState = {
  pointerId: null,
  startX: 0,
  startY: 0,
};

/**
 * Hook for tap navigation in the reader. Click/tap only — we intentionally
 * do not implement swipe gestures; movement above {@link TAP_TOLERANCE} is
 * ignored so the browser keeps its native pan/scroll/back-swipe behavior.
 *
 * Uses Pointer Events so a single code path covers touch (finger), mouse
 * (desktop, Chrome mobile-viewport emulation), and pen input.
 *
 * Tap-zone mapping (when `tapZones` is true, the default):
 * - LTR: left third → prev, middle → toolbar toggle, right third → next.
 * - RTL: mirrored.
 * - TTB / webtoon: top → prev, middle → toolbar, bottom → next.
 *
 * With `tapZones: false`, every tap fires `onTap` (used by continuous-scroll
 * modes where the whole surface is a toolbar toggle).
 *
 * @returns ref to attach to the touchable element
 */
export function useTouchNav({
  enabled = true,
  onNextPage,
  onPrevPage,
  onTap,
  tapZones = true,
}: UseTouchNavOptions = {}) {
  const storeNextPage = useReaderStore((state) => state.nextPage);
  const storePrevPage = useReaderStore((state) => state.prevPage);
  const readingDirection = useReaderStore(selectEffectiveReadingDirection);

  const nextPage = onNextPage ?? storeNextPage;
  const prevPage = onPrevPage ?? storePrevPage;

  const gestureState = useRef<GestureState>({ ...INITIAL_GESTURE });
  const elementRef = useRef<HTMLElement | null>(null);

  // Stash live config in a ref so the attached listeners (whose identity is
  // stable) always read fresh state without detach/reattach churn.
  const configRef = useRef({
    enabled,
    readingDirection,
    nextPage,
    prevPage,
    onTap,
    tapZones,
  });
  useLayoutEffect(() => {
    configRef.current = {
      enabled,
      readingDirection,
      nextPage,
      prevPage,
      onTap,
      tapZones,
    };
  });

  const handlePointerDown = useCallback((e: PointerEvent) => {
    const cfg = configRef.current;
    if (!cfg.enabled) return;
    if (!e.isPrimary) return;
    if (e.pointerType === "mouse" && e.button !== 0) return;

    gestureState.current = {
      pointerId: e.pointerId,
      startX: e.clientX,
      startY: e.clientY,
    };
  }, []);

  const handlePointerUp = useCallback((e: PointerEvent) => {
    const cfg = configRef.current;
    const state = gestureState.current;
    if (state.pointerId === null || state.pointerId !== e.pointerId) return;

    const deltaX = e.clientX - state.startX;
    const deltaY = e.clientY - state.startY;

    gestureState.current = { ...INITIAL_GESTURE };
    if (!cfg.enabled) return;
    if (!isTap(deltaX, deltaY)) return;

    if (!cfg.tapZones) {
      cfg.onTap?.();
      return;
    }

    const element = elementRef.current;
    if (!element) {
      cfg.onTap?.();
      return;
    }
    const rect = element.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) {
      cfg.onTap?.();
      return;
    }
    const zone = classifyTapZone(
      e.clientX - rect.left,
      e.clientY - rect.top,
      rect.width,
      rect.height,
      { readingDirection: cfg.readingDirection },
    );
    if (zone === "center") {
      cfg.onTap?.();
    } else if (zone === "next") {
      cfg.nextPage();
    } else {
      cfg.prevPage();
    }
  }, []);

  const handlePointerCancel = useCallback((e: PointerEvent) => {
    const state = gestureState.current;
    if (state.pointerId === e.pointerId) {
      gestureState.current = { ...INITIAL_GESTURE };
    }
  }, []);

  const setRef = useCallback(
    (element: HTMLElement | null) => {
      if (elementRef.current && elementRef.current !== element) {
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

      if (element) {
        element.addEventListener("pointerdown", handlePointerDown);
        element.addEventListener("pointerup", handlePointerUp);
        element.addEventListener("pointercancel", handlePointerCancel);
      }
    },
    [handlePointerDown, handlePointerUp, handlePointerCancel],
  );

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
