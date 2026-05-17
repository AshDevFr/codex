/**
 * Gesture classification helpers shared by `useTouchNav` (outer-container
 * pointer events) and `EpubReader`'s inside-iframe pointer hook.
 *
 * Kept input-agnostic on purpose: callers pass deltas in pixels and ms; the
 * helper has no knowledge of pointer events, touch events, or React.
 */

export type GestureKind = "tap" | "next" | "prev" | "none";

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
