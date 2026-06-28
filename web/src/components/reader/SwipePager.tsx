import { Box } from "@mantine/core";
import {
  type ReactNode,
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { flushSync } from "react-dom";
import type { ReadingDirection } from "@/store/readerStore";
import {
  decideSnap,
  isPinchZoomed,
  rubberBand,
  type SnapResult,
} from "./hooks/swipeGesture";
import { usePinchZoom } from "./hooks/usePinchZoom";
import { useTouchNav } from "./hooks/useTouchNav";

/** Default snap-back / page-turn animation duration (ms). */
const DEFAULT_SNAP_DURATION = 250;

export interface SwipePagerProps {
  /** The current spread (1 or 2 pages), centered in the filmstrip. */
  current: ReactNode;
  /** The previous spread, or null at the first spread. */
  prev: ReactNode | null;
  /** The next spread, or null at the last spread. */
  next: ReactNode | null;
  /**
   * Identity of the current spread (e.g. the page number, or joined page numbers
   * for a double spread). When it changes the filmstrip re-centers instantly so a
   * committed turn lands seamlessly.
   */
  pageKey: string;
  /**
   * Identity of the previous spread, used as the filmstrip slide's React key so a
   * committed turn moves the already-rendered (decoded) slide into the center
   * instead of swapping the centered slot's image source (which re-decodes and
   * briefly flashes the previous page). Falls back to a stable edge key when null.
   */
  prevKey?: string;
  /** Identity of the next spread; see {@link prevKey}. */
  nextKey?: string;
  /** Reading direction; flips the visual slot order and swipe polarity for RTL. */
  readingDirection: ReadingDirection;
  /** Turn to the next spread (e.g. the reader's paginated-next handler). */
  onNext: () => void;
  /** Turn to the previous spread. */
  onPrev: () => void;
  /** Tap callback (toolbar toggle for center taps, via the tap-zone logic). */
  onTap: () => void;
  /**
   * Exit the reader. Wired to a deliberate downward fling (swipe down to
   * dismiss). Skipped when the page is pannable/zoomed (that drag pans instead).
   */
  onExit?: () => void;
  /** Master switch for finger-drag paging. When false, only tap navigation runs. */
  enabled: boolean;
  /** Snap animation duration in ms. */
  duration?: number;
  /**
   * Returns true when the current page is horizontally pannable and swipe-to-turn
   * must yield to native panning (e.g. an over-wide fit mode). Pinch-zoom is
   * always treated as pannable in addition to this. Defaults to never (the parent
   * supplies the fit-mode-aware check).
   */
  isContentPannable?: () => boolean;
}

const clamp = (value: number, min: number, max: number): number =>
  Math.min(Math.max(value, min), max);

/** translate3d for the track given the centered slot index and a live drag offset. */
const trackTransform = (index: number, dragPx: number): string =>
  `translate3d(calc(${-index * 100}% + ${dragPx}px), 0, 0)`;

interface AnimState {
  /** Which slot is centered: 0 = visual-left, 1 = current, 2 = visual-right. */
  index: number;
  /** Whether to animate the move to `index` (false = instant snap). */
  animate: boolean;
}

const CENTERED: AnimState = { index: 1, animate: false };

/**
 * Finger-drag pager for the paged comic reader. Renders a 3-up
 * horizontal filmstrip (prev / current / next) that follows the finger and snaps
 * to a neighbor on release (by distance or flick velocity). Tap navigation and
 * toolbar toggling still flow through {@link useTouchNav}'s tap zones; this only
 * adds the live drag. Vertical-dominant drags, pinch-zoom, and pannable content
 * fall through to native behavior.
 *
 * Scoped to single/double paged modes — webtoon/continuous navigate by scroll.
 */
export function SwipePager({
  current,
  prev,
  next,
  pageKey,
  prevKey,
  nextKey,
  readingDirection,
  onNext,
  onPrev,
  onTap,
  onExit,
  enabled,
  duration = DEFAULT_SNAP_DURATION,
  isContentPannable,
}: SwipePagerProps) {
  const rootRef = useRef<HTMLElement | null>(null);
  const trackRef = useRef<HTMLDivElement | null>(null);
  const zoomContentRef = useRef<HTMLDivElement | null>(null);
  const draggingRef = useRef(false);
  const commitTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const prevPageKeyRef = useRef(pageKey);

  const [anim, setAnim] = useState<AnimState>(CENTERED);

  // Content-only zoom for the current page (pinch + pan). The transform is
  // written imperatively to the current slide; `isZoomedNow` lets the gesture
  // layer route a one-finger drag to pan (zoomed) vs page-turn (fit).
  const {
    pinch,
    panBy,
    reset: resetZoom,
    doubleTap,
    isZoomedNow,
  } = usePinchZoom({
    viewportRef: rootRef,
    contentRef: zoomContentRef,
  });

  const isRtl = readingDirection === "rtl";
  // Visual left→right order. In RTL the "next" page sits on the left. Each slide
  // is keyed by its page identity (not its fixed position) so that when a turn
  // commits, the slide that was a neighbor and becomes the new current is *moved*
  // into the center slot by React — keeping its already-decoded <img>. Keying by
  // position instead would reuse the center slot and swap its image source, which
  // re-decodes on production and briefly flashes the previous page.
  const currentSlide = { key: pageKey, node: current };
  const prevSlide = { key: prevKey ?? "edge-prev", node: prev };
  const nextSlide = { key: nextKey ?? "edge-next", node: next };
  const orderedSlides = isRtl
    ? [nextSlide, currentSlide, prevSlide]
    : [prevSlide, currentSlide, nextSlide];
  const visualLeftPresent = orderedSlides[0].node != null;
  const visualRightPresent = orderedSlides[2].node != null;

  const rootWidth = useCallback(
    () => rootRef.current?.getBoundingClientRect().width ?? 0,
    [],
  );

  const clearCommitTimer = useCallback(() => {
    if (commitTimerRef.current !== null) {
      clearTimeout(commitTimerRef.current);
      commitTimerRef.current = null;
    }
  }, []);

  // Re-center instantly whenever the current spread changes (a committed turn, or
  // a tap/keyboard/button navigation from outside). Runs before paint so there is
  // no visible jump. The previous-key ref means the mount render is a no-op.
  useLayoutEffect(() => {
    if (prevPageKeyRef.current === pageKey) return;
    prevPageKeyRef.current = pageKey;
    clearCommitTimer();
    draggingRef.current = false;
    // Force the strip back to its centered slot with the transition disabled,
    // imperatively, before the browser paints. The committed slide content is
    // already in the DOM at this point, so writing the centered transform here
    // (rather than waiting for the `setAnim` re-render to commit) guarantees the
    // new page and the re-center land in the same frame. Without this the strip
    // could paint one stale frame at the neighbor slot, briefly re-showing the
    // previous page after the new one had settled.
    const track = trackRef.current;
    if (track) {
      track.style.transition = "none";
      track.style.transform = trackTransform(1, 0);
    }
    setAnim(CENTERED);
    // A new page always starts at fit; reset before paint so it never flashes
    // the previous page's zoom.
    resetZoom();
  }, [pageKey, clearCommitTimer, resetZoom]);

  useLayoutEffect(() => () => clearCommitTimer(), [clearCommitTimer]);

  const pannable = useCallback((): boolean => {
    const vv =
      typeof window !== "undefined" ? window.visualViewport : undefined;
    if (vv && isPinchZoomed(vv.scale)) return true;
    return isContentPannable?.() ?? false;
  }, [isContentPannable]);

  const handleDragStart = useCallback((): boolean => {
    if (!enabled) return false;
    if (pannable()) return false;
    clearCommitTimer();
    return true;
  }, [enabled, pannable, clearCommitTimer]);

  const handleDragMove = useCallback(
    (dragPx: number) => {
      draggingRef.current = true;
      const track = trackRef.current;
      if (!track) return;
      const width = rootWidth();
      const revealingLeft = dragPx > 0;
      const present = revealingLeft ? visualLeftPresent : visualRightPresent;
      const eff = present
        ? clamp(dragPx, -width, width)
        : rubberBand(dragPx, width);
      track.style.transition = "none";
      track.style.transform = trackTransform(1, eff);
    },
    [rootWidth, visualLeftPresent, visualRightPresent],
  );

  // Start the snap (or snap-back) animation imperatively. The live drag writes
  // the track transform straight to the DOM (bypassing React for per-frame
  // perf), so React's model goes stale. A `setAnim` that then computes the same
  // transform string React last rendered — notably snapping back to the centered
  // slot (index 1) after a sub-threshold drag — would be skipped by
  // reconciliation, parking the track at the dragged offset (no snap-back).
  // Writing the target transform here guarantees the animation always runs;
  // `setAnim` keeps React's model in sync for subsequent renders.
  const animateTrackTo = useCallback(
    (index: number) => {
      const track = trackRef.current;
      if (track) {
        track.style.transition = `transform ${duration}ms ease-out`;
        track.style.transform = trackTransform(index, 0);
      }
      setAnim({ index, animate: true });
    },
    [duration],
  );

  const handleDragEnd = useCallback(
    (dragPx: number, _dragY: number, velocityPxPerMs: number) => {
      draggingRef.current = false;
      const width = rootWidth();
      const result: SnapResult = decideSnap({
        dragPx,
        velocityPxPerMs,
        viewportWidth: width || 1,
        hasPrev: prev != null,
        hasNext: next != null,
        readingDirection,
      });

      if (result === "stay") {
        animateTrackTo(1);
        return;
      }

      const goNext = result === "next";
      // Map the logical turn to a visual slot: next is on the right (LTR) / left (RTL).
      const visualTarget = (isRtl ? !goNext : goNext) ? 2 : 0;
      animateTrackTo(visualTarget);

      clearCommitTimer();
      commitTimerRef.current = setTimeout(() => {
        commitTimerRef.current = null;
        // Commit synchronously so the page-index advance and the resulting
        // filmstrip re-center (the pageKey layout effect) flush in a single
        // paint. On production/minified builds React can otherwise split them
        // across two frames, painting one stale frame that re-shows the page we
        // just turned away from.
        flushSync(() => {
          if (goNext) onNext();
          else onPrev();
        });
      }, duration);
    },
    [
      rootWidth,
      prev,
      next,
      readingDirection,
      isRtl,
      onNext,
      onPrev,
      duration,
      clearCommitTimer,
      animateTrackTo,
    ],
  );

  const handleDragCancel = useCallback(() => {
    draggingRef.current = false;
    clearCommitTimer();
    animateTrackTo(1);
  }, [clearCommitTimer, animateTrackTo]);

  const handleSwipeDown = useCallback(() => {
    // A downward fling exits the reader, but not while the page is pannable
    // (over-wide fit / pinch-zoom), where the same drag pans the content.
    if (pannable()) return;
    onExit?.();
  }, [pannable, onExit]);

  const { touchRef } = useTouchNav({
    enabled,
    onNextPage: onNext,
    onPrevPage: onPrev,
    onTap,
    tapZones: true,
    swipe: enabled
      ? {
          enabled: true,
          onStart: handleDragStart,
          onMove: handleDragMove,
          onEnd: handleDragEnd,
          onCancel: handleDragCancel,
          onSwipeDown: onExit ? handleSwipeDown : undefined,
        }
      : undefined,
    zoom: enabled
      ? {
          // When zoomed, a one-finger drag pans the page instead of turning it.
          panActive: isZoomedNow,
          onPan: panBy,
          onPinch: pinch,
          // Double-tap zooms in to the point (or back out to fit when zoomed).
          onDoubleTap: doubleTap,
        }
      : undefined,
  });

  const setRootRef = useCallback(
    (el: HTMLElement | null) => {
      rootRef.current = el;
      touchRef(el);
    },
    [touchRef],
  );

  return (
    <Box
      ref={setRootRef}
      style={{
        position: "relative",
        width: "100%",
        height: "100%",
        overflow: "hidden",
        // We own all touch on the page (swipe, pan, pinch), so take the gestures
        // outright. This also avoids the browser's pan-y direction-disambiguation
        // latency, making the drag track the finger immediately.
        touchAction: enabled ? "none" : "manipulation",
      }}
    >
      <Box
        ref={trackRef}
        style={{
          display: "flex",
          width: "100%",
          height: "100%",
          transform: trackTransform(anim.index, 0),
          transition: anim.animate
            ? `transform ${duration}ms ease-out`
            : "none",
          willChange: "transform",
        }}
      >
        {orderedSlides.map((slide, i) => (
          <Box
            // Keyed by page identity so a committed turn moves the slide (and its
            // decoded image) into place rather than re-sourcing a fixed slot.
            key={slide.key}
            style={{
              flex: "0 0 100%",
              width: "100%",
              height: "100%",
              overflow: "hidden",
            }}
          >
            {/* Every slide uses the same inner wrapper, so a slide moving from a
                neighbor into the center keeps its DOM node (and decoded image)
                instead of remounting. Only the centered slide carries the live
                zoom transform via the ref; neighbors render at fit. */}
            <Box
              ref={i === 1 ? zoomContentRef : undefined}
              style={{
                width: "100%",
                height: "100%",
                willChange: "transform",
              }}
            >
              {slide.node}
            </Box>
          </Box>
        ))}
      </Box>
    </Box>
  );
}
