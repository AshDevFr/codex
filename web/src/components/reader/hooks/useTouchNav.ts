import { useCallback, useEffect, useLayoutEffect, useRef } from "react";
import {
  selectEffectiveReadingDirection,
  useReaderStore,
} from "@/store/readerStore";
import {
  classifyTapZone,
  isHorizontalDrag,
  isTap,
  SWIPE_ACTIVATION_PX,
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

/**
 * Content-zoom callbacks. When provided, the hook also recognizes two-finger
 * pinch and (while `panActive` returns true, i.e. the page is zoomed) one-finger
 * pan. Pinch/pan take priority over swipe: a second finger aborts an in-flight
 * swipe, and while panning a one-finger drag never turns the page.
 */
export interface ZoomHandlers {
  /** When true, a one-finger drag pans the page (zoomed) instead of swiping. */
  panActive: () => boolean;
  /** Incremental pan delta since the previous move, in px. */
  onPan: (dx: number, dy: number) => void;
  /** The pan gesture ended (finger lifted). */
  onPanEnd?: () => void;
  /**
   * Two-finger pinch step: the incremental scale ratio since the last move and
   * the focal point (the pinch midpoint, relative to the element center).
   */
  onPinch: (scaleRatio: number, focus: { x: number; y: number }) => void;
  /** The pinch gesture ended (dropped below two fingers). */
  onPinchEnd?: () => void;
  /**
   * Double-tap with the focal point (relative to element center). When set, every
   * single tap is held briefly ({@link DOUBLE_TAP_MS}) to disambiguate, so taps
   * incur a small delay.
   */
  onDoubleTap?: (focus: { x: number; y: number }) => void;
}

/** Max gap (ms) between two taps to count as a double-tap. */
const DOUBLE_TAP_MS = 280;
/** Max distance (px) between two taps to count as a double-tap. */
const DOUBLE_TAP_DIST = 40;

interface PendingTap {
  timer: ReturnType<typeof setTimeout>;
  x: number;
  y: number;
  t: number;
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
  /** Optional content-zoom (pinch + pan) handlers. */
  zoom?: ZoomHandlers;
}

type GestureMode = "none" | "swipe" | "pan" | "pinch";

interface GestureState {
  /** Recognized gesture. `none` = a single pointer is down but not yet armed. */
  mode: GestureMode;
  /** The primary single-finger pointer (swipe/pan/tap origin). */
  pointerId: number | null;
  startX: number;
  startY: number;
  /** True once a swipe was vetoed (over-wide fit) — suppresses re-arming. */
  vetoed: boolean;
  /** Last move position (both axes) + x-velocity sample window. */
  lastX: number;
  lastY: number;
  lastT: number;
  prevX: number;
  prevT: number;
  /** The two pointers tracked during a pinch, and their last separation. */
  pinchA: number | null;
  pinchB: number | null;
  pinchLastDist: number;
}

const INITIAL_GESTURE: GestureState = {
  mode: "none",
  pointerId: null,
  startX: 0,
  startY: 0,
  vetoed: false,
  lastX: 0,
  lastY: 0,
  lastT: 0,
  prevX: 0,
  prevT: 0,
  pinchA: null,
  pinchB: null,
  pinchLastDist: 0,
};

interface Pt {
  x: number;
  y: number;
}

const distance = (a: Pt, b: Pt): number => Math.hypot(a.x - b.x, a.y - b.y);
const midpoint = (a: Pt, b: Pt): Pt => ({
  x: (a.x + b.x) / 2,
  y: (a.y + b.y) / 2,
});

/**
 * Capture the pointer so a fast drag keeps delivering `pointermove` even if the
 * finger strays over a child or briefly leaves the surface (smoother tracking).
 * Feature-detected and best-effort: jsdom and older browsers simply skip it.
 */
function capturePointer(element: HTMLElement | null, pointerId: number): void {
  if (element && typeof element.setPointerCapture === "function") {
    try {
      element.setPointerCapture(pointerId);
    } catch {
      // Ignore — capture is a smoothness optimization, not load-bearing.
    }
  }
}

function releasePointer(element: HTMLElement | null, pointerId: number): void {
  if (element && typeof element.releasePointerCapture === "function") {
    try {
      element.releasePointerCapture(pointerId);
    } catch {
      // Ignore — the pointer may already be released.
    }
  }
}

/**
 * Pointer-gesture hook for the reader page surface. A single multi-touch owner so
 * tap, swipe (page turn), pan, and pinch never fight: it tracks every active
 * pointer and recognizes one gesture at a time.
 *
 * Uses Pointer Events so a single code path covers touch (finger), mouse
 * (desktop, Chrome mobile-viewport emulation), and pen input.
 *
 * - **Tap** (no/below-threshold movement): tap zones (when `tapZones`) map
 *   LTR left→prev / middle→toolbar / right→next (mirrored RTL; vertical for
 *   TTB/webtoon); with `tapZones: false` every tap fires `onTap`.
 * - **Swipe** (`swipe` config): a horizontal-dominant one-finger drag turns the
 *   page; `onStart` may veto it.
 * - **Pan/pinch** (`zoom` config): two fingers pinch; while `panActive()` a
 *   one-finger drag pans. These pre-empt swipe.
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
  zoom,
}: UseTouchNavOptions = {}) {
  const storeNextPage = useReaderStore((state) => state.nextPage);
  const storePrevPage = useReaderStore((state) => state.prevPage);
  const readingDirection = useReaderStore(selectEffectiveReadingDirection);

  const nextPage = onNextPage ?? storeNextPage;
  const prevPage = onPrevPage ?? storePrevPage;

  const gestureState = useRef<GestureState>({ ...INITIAL_GESTURE });
  const pointersRef = useRef<Map<number, Pt>>(new Map());
  const tapPendingRef = useRef<PendingTap | null>(null);
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
    zoom,
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
      zoom,
    };
  });

  // Resolve a tap at the given client point to its action (tap zone nav, or
  // toolbar toggle). Reads fresh config so it works when deferred for double-tap.
  const fireTapAt = useCallback((clientX: number, clientY: number) => {
    const cfg = configRef.current;
    // While zoomed, a single tap never navigates — it only toggles the toolbar,
    // so an edge tap can't accidentally turn the page out from under the zoom.
    if (cfg.zoom?.panActive?.()) {
      cfg.onTap?.();
      return;
    }
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
      clientX - rect.left,
      clientY - rect.top,
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

  const handlePointerDown = useCallback((e: PointerEvent) => {
    const cfg = configRef.current;
    if (!cfg.enabled) return;
    if (e.pointerType === "mouse" && e.button !== 0) return;

    const pointers = pointersRef.current;
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    const state = gestureState.current;

    // Second finger → pinch (only when zoom is configured). Abort any one-finger
    // gesture already in progress.
    if (pointers.size >= 2 && cfg.zoom) {
      if (state.mode === "swipe") cfg.swipe?.onCancel?.();
      else if (state.mode === "pan") cfg.zoom.onPanEnd?.();

      const ids = Array.from(pointers.keys());
      const a = ids[0];
      const b = ids[1];
      const pa = pointers.get(a);
      const pb = pointers.get(b);
      gestureState.current = {
        ...INITIAL_GESTURE,
        mode: "pinch",
        pinchA: a,
        pinchB: b,
        pinchLastDist: pa && pb ? distance(pa, pb) : 0,
      };
      return;
    }

    // Single primary pointer → swipe/pan/tap origin.
    if (!e.isPrimary) return;
    gestureState.current = {
      ...INITIAL_GESTURE,
      mode: "none",
      pointerId: e.pointerId,
      startX: e.clientX,
      startY: e.clientY,
      lastX: e.clientX,
      lastY: e.clientY,
      lastT: e.timeStamp,
      prevX: e.clientX,
      prevT: e.timeStamp,
    };
  }, []);

  const handlePointerMove = useCallback((e: PointerEvent) => {
    const cfg = configRef.current;
    if (!cfg.enabled) return;
    const pointers = pointersRef.current;
    if (pointers.has(e.pointerId)) {
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    }
    const state = gestureState.current;

    // --- Pinch ---
    if (state.mode === "pinch") {
      if (!cfg.zoom || state.pinchA === null || state.pinchB === null) return;
      const pa = pointers.get(state.pinchA);
      const pb = pointers.get(state.pinchB);
      if (!pa || !pb) return;
      const dist = distance(pa, pb);
      if (state.pinchLastDist > 0 && dist > 0) {
        const mid = midpoint(pa, pb);
        const el = elementRef.current;
        let focus = { x: 0, y: 0 };
        if (el) {
          const rect = el.getBoundingClientRect();
          focus = {
            x: mid.x - (rect.left + rect.width / 2),
            y: mid.y - (rect.top + rect.height / 2),
          };
        }
        cfg.zoom.onPinch(dist / state.pinchLastDist, focus);
      }
      state.pinchLastDist = dist;
      if (e.cancelable) e.preventDefault();
      return;
    }

    // --- One finger (swipe / pan / pending) ---
    if (state.pointerId === null || state.pointerId !== e.pointerId) return;
    const sw = cfg.swipe;

    const deltaX = e.clientX - state.startX;
    const deltaY = e.clientY - state.startY;
    // Incremental pan delta uses the *previous* move position (before rolling).
    const panDx = e.clientX - state.lastX;
    const panDy = e.clientY - state.lastY;

    // Roll the velocity / last-position window forward.
    state.prevX = state.lastX;
    state.prevT = state.lastT;
    state.lastX = e.clientX;
    state.lastY = e.clientY;
    state.lastT = e.timeStamp;

    if (state.mode === "none") {
      const panActive = cfg.zoom?.panActive?.() ?? false;
      const movedEnough =
        Math.abs(deltaX) >= SWIPE_ACTIVATION_PX ||
        Math.abs(deltaY) >= SWIPE_ACTIVATION_PX;

      if (panActive && cfg.zoom) {
        if (!movedEnough) return;
        state.mode = "pan";
        capturePointer(elementRef.current, e.pointerId);
        // Start panning on the next move to avoid an initial jump.
        if (e.cancelable) e.preventDefault();
        return;
      }
      if (sw?.enabled && !state.vetoed && isHorizontalDrag(deltaX, deltaY)) {
        if (sw.onStart && !sw.onStart()) {
          state.vetoed = true;
          return;
        }
        state.mode = "swipe";
        capturePointer(elementRef.current, e.pointerId);
        // Fall through to emit this move.
      } else {
        return;
      }
    }

    if (state.mode === "pan") {
      cfg.zoom?.onPan(panDx, panDy);
    } else if (state.mode === "swipe") {
      sw?.onMove?.(deltaX, deltaY);
    }
    if (e.cancelable) e.preventDefault();
  }, []);

  const handlePointerUp = useCallback(
    (e: PointerEvent) => {
      const cfg = configRef.current;
      const pointers = pointersRef.current;
      pointers.delete(e.pointerId);
      const state = gestureState.current;

      if (state.mode === "pinch") {
        if (pointers.size < 2) {
          cfg.zoom?.onPinchEnd?.();
          // Ignore any finger still down until a full release.
          gestureState.current = { ...INITIAL_GESTURE };
        }
        return;
      }

      if (state.pointerId === null || state.pointerId !== e.pointerId) {
        if (pointers.size === 0) gestureState.current = { ...INITIAL_GESTURE };
        return;
      }

      const deltaX = e.clientX - state.startX;
      const deltaY = e.clientY - state.startY;
      const mode = state.mode;
      // Release velocity from the last two samples (px/ms); 0 if we lack a window.
      const dt = state.lastT - state.prevT;
      const velocity = dt > 0 ? (state.lastX - state.prevX) / dt : 0;

      gestureState.current = { ...INITIAL_GESTURE };
      if (!cfg.enabled) return;

      if (mode === "pan") {
        releasePointer(elementRef.current, e.pointerId);
        cfg.zoom?.onPanEnd?.();
        return;
      }
      if (mode === "swipe") {
        releasePointer(elementRef.current, e.pointerId);
        cfg.swipe?.onEnd?.(deltaX, deltaY, velocity);
        return;
      }
      if (!isTap(deltaX, deltaY)) return;

      // No double-tap configured: fire the tap action immediately (no delay).
      const onDoubleTap = cfg.zoom?.onDoubleTap;
      if (!onDoubleTap) {
        fireTapAt(e.clientX, e.clientY);
        return;
      }

      // A second tap close in time + space to the first is a double-tap: cancel the
      // first tap's deferred action and zoom instead.
      const pending = tapPendingRef.current;
      if (
        pending &&
        e.timeStamp - pending.t <= DOUBLE_TAP_MS &&
        Math.hypot(e.clientX - pending.x, e.clientY - pending.y) <=
          DOUBLE_TAP_DIST
      ) {
        clearTimeout(pending.timer);
        tapPendingRef.current = null;
        const el = elementRef.current;
        let focus = { x: 0, y: 0 };
        if (el) {
          const rect = el.getBoundingClientRect();
          focus = {
            x: e.clientX - (rect.left + rect.width / 2),
            y: e.clientY - (rect.top + rect.height / 2),
          };
        }
        onDoubleTap(focus);
        return;
      }

      // First tap: defer the action so a following tap can upgrade it to a zoom.
      const x = e.clientX;
      const y = e.clientY;
      const timer = setTimeout(() => {
        tapPendingRef.current = null;
        fireTapAt(x, y);
      }, DOUBLE_TAP_MS);
      tapPendingRef.current = { timer, x, y, t: e.timeStamp };
    },
    [fireTapAt],
  );

  const handlePointerCancel = useCallback((e: PointerEvent) => {
    const cfg = configRef.current;
    const pointers = pointersRef.current;
    pointers.delete(e.pointerId);
    const state = gestureState.current;

    if (state.mode === "pinch") {
      if (pointers.size < 2) {
        cfg.zoom?.onPinchEnd?.();
        gestureState.current = { ...INITIAL_GESTURE };
      }
      return;
    }

    if (state.pointerId !== e.pointerId) {
      if (pointers.size === 0) gestureState.current = { ...INITIAL_GESTURE };
      return;
    }

    const mode = state.mode;
    gestureState.current = { ...INITIAL_GESTURE };
    if (mode === "pan") {
      releasePointer(elementRef.current, e.pointerId);
      cfg.zoom?.onPanEnd?.();
    } else if (mode === "swipe") {
      releasePointer(elementRef.current, e.pointerId);
      cfg.swipe?.onCancel?.();
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
      if (tapPendingRef.current) {
        clearTimeout(tapPendingRef.current.timer);
        tapPendingRef.current = null;
      }
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
