/**
 * Gesture classification helpers shared by `useTouchNav` (outer-container
 * pointer events) and `EpubReader`'s inside-iframe pointer hook.
 *
 * Kept input-agnostic on purpose: callers pass deltas in pixels and ms; the
 * helper has no knowledge of pointer events, touch events, or React.
 */

export type GestureKind = "tap" | "next" | "prev" | "none";

/**
 * Which zone of a reader surface a tap landed in.
 *
 * - `prev`: outer slice in the "go back" direction (left for LTR / right for
 *   RTL / top for TTB / webtoon).
 * - `center`: middle third of the surface; reserved for revealing the toolbar.
 * - `next`: outer slice in the "go forward" direction.
 */
export type TapZone = "prev" | "center" | "next";

export interface ClassifyTapZoneOptions {
  /** Reading direction; determines the tap axis and prev/next polarity. */
  readingDirection?: "ltr" | "rtl" | "ttb" | "webtoon";
}

/**
 * Map a tap location inside a reader surface to a {@link TapZone}.
 *
 * Splits the active axis into thirds:
 * - LTR/RTL: horizontal thirds (left | center | right).
 * - TTB/webtoon: vertical thirds (top | center | bottom).
 *
 * The center third always returns `"center"` so center taps reveal the toolbar
 * instead of navigating, regardless of reading direction. Edge thirds map to
 * `prev` / `next` based on direction:
 * - LTR: left → prev, right → next.
 * - RTL: left → next, right → prev.
 * - TTB / webtoon: top → prev, bottom → next.
 */
export function classifyTapZone(
  x: number,
  y: number,
  width: number,
  height: number,
  options: ClassifyTapZoneOptions = {},
): TapZone {
  const { readingDirection = "ltr" } = options;
  const isVerticalMode =
    readingDirection === "ttb" || readingDirection === "webtoon";

  if (isVerticalMode) {
    if (height <= 0) return "center";
    const third = height / 3;
    if (y < third) return "prev";
    if (y > 2 * third) return "next";
    return "center";
  }

  if (width <= 0) return "center";
  const third = width / 3;
  // Horizontal axis: which physical edge is "prev" depends on RTL.
  if (readingDirection === "rtl") {
    if (x < third) return "next";
    if (x > 2 * third) return "prev";
    return "center";
  }
  if (x < third) return "prev";
  if (x > 2 * third) return "next";
  return "center";
}

export interface ClassifySwipeOptions {
  /** Minimum swipe distance in pixels to register as a swipe (default: 50). */
  minSwipeDistance?: number;
  /** Maximum gesture duration in ms; longer means no swipe (default: 300). */
  maxSwipeTime?: number;
  /** Maximum movement in pixels (any direction) still considered a tap (default: 10). */
  tapTolerance?: number;
  /** Reading direction; controls whether the gesture is horizontal or vertical
   *  and whether left/right are reversed. */
  readingDirection?: "ltr" | "rtl" | "ttb" | "webtoon";
}

/**
 * Classify a pointer/touch gesture into `tap`, `next`, `prev`, or `none`.
 *
 * Direction semantics:
 * - `ltr`: swipe left → next, swipe right → prev
 * - `rtl`: swipe left → prev, swipe right → next
 * - `ttb` / `webtoon`: swipe up → next, swipe down → prev
 *
 * Returns `"none"` if the gesture exceeded `maxSwipeTime` without being a tap,
 * or if it moved enough to disqualify as a tap but not enough to qualify as a
 * swipe.
 */
export function classifySwipe(
  deltaX: number,
  deltaY: number,
  deltaTime: number,
  options: ClassifySwipeOptions = {},
): GestureKind {
  const {
    minSwipeDistance = 50,
    maxSwipeTime = 300,
    tapTolerance = 10,
    readingDirection = "ltr",
  } = options;

  const absX = Math.abs(deltaX);
  const absY = Math.abs(deltaY);

  // Tap: minimal movement regardless of timing — a slow finger-down/up in
  // place is still a tap.
  if (absX < tapTolerance && absY < tapTolerance) {
    return "tap";
  }

  // Too slow to count as a swipe.
  if (deltaTime > maxSwipeTime) {
    return "none";
  }

  const isVerticalMode =
    readingDirection === "ttb" || readingDirection === "webtoon";

  if (isVerticalMode) {
    const isVerticalSwipe = absY > absX && absY >= minSwipeDistance;
    if (!isVerticalSwipe) return "none";
    return deltaY < 0 ? "next" : "prev";
  }

  const isHorizontalSwipe = absX > absY && absX >= minSwipeDistance;
  if (!isHorizontalSwipe) return "none";

  const isRtl = readingDirection === "rtl";
  if (isRtl) {
    return deltaX < 0 ? "prev" : "next";
  }
  return deltaX < 0 ? "next" : "prev";
}
