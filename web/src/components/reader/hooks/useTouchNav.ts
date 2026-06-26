import { useCallback, useEffect, useLayoutEffect, useRef } from "react";
import {
  selectEffectiveReadingDirection,
  useReaderStore,
} from "@/store/readerStore";
import {
  classifyTapZone,
  isHorizontalDrag,
  isTap,
  TAP_TOLERANCE,
} from "./swipeGesture";

/**
 * Live horizontal-drag (swipe) callbacks. When provided and `enabled`, the hook
 * tracks pointer movement and drives a finger-following page turn (see
 * `SwipePager`). A drag only *arms* once it crosses the activation threshold and
 * is horizontal-dominant; `onStart` may veto it (e.g. when the page is pannable)
 * so the gesture falls through to native panning/scroll. While unarmed or
 * vetoed, the existing tap behavior is unaffected.
 */
export interface SwipeHandlers {
  /** Master switch; when false the drag path is inert. */
  enabled: boolean;
  /**
   * Called once when a drag is about to arm. Return false to veto it (the
   * gesture then behaves like today: native pan/scroll, no page turn).
   */
  onStart?: () => boolean;
  /** Called on every move while armed, with the signed offset from the origin. */
  onMove?: (dragPx: number, dragY: number) => void;
  /** Called on release while armed, with the final offset and release velocity. */
  onEnd?: (dragPx: number, dragY: number, velocityPxPerMs: number) => void;
  /** Called when the gesture is cancelled mid-drag (system interruption). */
  onCancel?: () => void;
}

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
  /** Optional live swipe (finger-drag paging) handlers. */
  swipe?: SwipeHandlers;
}

interface GestureState {
  pointerId: number | null;
  startX: number;
  startY: number;
  /** True once a horizontal drag has armed (swipe in progress). */
  armed: boolean;
  /** True once a drag was vetoed (pannable/zoomed) — suppresses re-arming. */
  vetoed: boolean;
  /** Most recent sample, for release-velocity estimation. */
  lastX: number;
  lastT: number;
  /** Previous sample (the one before `last`). */
  prevX: number;
  prevT: number;
}

const INITIAL_GESTURE: GestureState = {
  pointerId: null,
  startX: 0,
  startY: 0,
  armed: false,
  vetoed: false,
  lastX: 0,
  lastT: 0,
  prevX: 0,
  prevT: 0,
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
  swipe,
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
    swipe,
  });
  useLayoutEffect(() => {
    configRef.current = {
      enabled,
      readingDirection,
      nextPage,
      prevPage,
      onTap,
      tapZones,
      swipe,
    };
  });

  const handlePointerDown = useCallback((e: PointerEvent) => {
    const cfg = configRef.current;
    if (!cfg.enabled) return;
    if (!e.isPrimary) return;
    if (e.pointerType === "mouse" && e.button !== 0) return;

    gestureState.current = {
      ...INITIAL_GESTURE,
      pointerId: e.pointerId,
      startX: e.clientX,
      startY: e.clientY,
      lastX: e.clientX,
      lastT: e.timeStamp,
      prevX: e.clientX,
      prevT: e.timeStamp,
    };
  }, []);

  const handlePointerMove = useCallback((e: PointerEvent) => {
    const cfg = configRef.current;
    const state = gestureState.current;
    if (state.pointerId === null || state.pointerId !== e.pointerId) return;
    const sw = cfg.swipe;
    if (!cfg.enabled || !sw?.enabled || state.vetoed) return;

    const deltaX = e.clientX - state.startX;
    const deltaY = e.clientY - state.startY;

    // Roll the velocity sample window forward.
    state.prevX = state.lastX;
    state.prevT = state.lastT;
    state.lastX = e.clientX;
    state.lastT = e.timeStamp;

    if (!state.armed) {
      if (!isHorizontalDrag(deltaX, deltaY)) return;
      // Crossed the activation threshold horizontally: try to arm. A veto (e.g.
      // the page is pannable/zoomed) leaves native panning untouched.
      if (sw.onStart && !sw.onStart()) {
        state.vetoed = true;
        return;
      }
      state.armed = true;
    }

    sw.onMove?.(deltaX, deltaY);
    // Stop native horizontal scroll/back-swipe from fighting the drag. Requires
    // a non-passive listener (registered below).
    if (e.cancelable) e.preventDefault();
  }, []);

  const handlePointerUp = useCallback((e: PointerEvent) => {
    const cfg = configRef.current;
    const state = gestureState.current;
    if (state.pointerId === null || state.pointerId !== e.pointerId) return;

    const deltaX = e.clientX - state.startX;
    const deltaY = e.clientY - state.startY;
    const wasArmed = state.armed;
    // Release velocity from the last two samples (px/ms); 0 if we lack a window.
    const dt = state.lastT - state.prevT;
    const velocity = dt > 0 ? (state.lastX - state.prevX) / dt : 0;

    gestureState.current = { ...INITIAL_GESTURE };
    if (!cfg.enabled) return;

    if (wasArmed) {
      cfg.swipe?.onEnd?.(deltaX, deltaY, velocity);
      return;
    }
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
    const cfg = configRef.current;
    const state = gestureState.current;
    if (state.pointerId === e.pointerId) {
      const wasArmed = state.armed;
      gestureState.current = { ...INITIAL_GESTURE };
      if (wasArmed) cfg.swipe?.onCancel?.();
    }
  }, []);

  const setRef = useCallback(
    (element: HTMLElement | null) => {
      if (elementRef.current && elementRef.current !== element) {
        elementRef.current.removeEventListener(
          "pointerdown",
          handlePointerDown,
        );
        elementRef.current.removeEventListener(
          "pointermove",
          handlePointerMove,
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
        // Non-passive so the drag can preventDefault native scroll while armed.
        element.addEventListener("pointermove", handlePointerMove, {
          passive: false,
        });
        element.addEventListener("pointerup", handlePointerUp);
        element.addEventListener("pointercancel", handlePointerCancel);
      }
    },
    [
      handlePointerDown,
      handlePointerMove,
      handlePointerUp,
      handlePointerCancel,
    ],
  );

  useEffect(() => {
    return () => {
      if (elementRef.current) {
        elementRef.current.removeEventListener(
          "pointerdown",
          handlePointerDown,
        );
        elementRef.current.removeEventListener(
          "pointermove",
          handlePointerMove,
        );
        elementRef.current.removeEventListener("pointerup", handlePointerUp);
        elementRef.current.removeEventListener(
          "pointercancel",
          handlePointerCancel,
        );
      }
    };
  }, [
    handlePointerDown,
    handlePointerMove,
    handlePointerUp,
    handlePointerCancel,
  ]);

  return { touchRef: setRef };
}
