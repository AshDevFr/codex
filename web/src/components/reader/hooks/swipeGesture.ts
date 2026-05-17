/**
 * Tap-gesture helpers shared by `useTouchNav` (outer-container pointer events)
 * and `EpubReader`'s inside-iframe pointer hook.
 *
 * Click-only navigation: we intentionally do not classify swipes. A pointer
 * movement above `TAP_TOLERANCE` is ignored so the browser keeps its native
 * pan/scroll/back-swipe behavior intact.
 *
 * Kept input-agnostic on purpose: callers pass deltas in pixels; the helper
 * has no knowledge of pointer events, touch events, or React.
 */

/** Maximum movement in pixels still treated as a tap (not a drag/swipe). */
export const TAP_TOLERANCE = 10;

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
 * Returns true when the pointer barely moved between down and up. We treat
 * anything within `TAP_TOLERANCE` (default 10px) as an intentional tap and
 * ignore anything larger so the browser handles pan / scroll / back-swipe.
 */
export function isTap(
  deltaX: number,
  deltaY: number,
  tapTolerance: number = TAP_TOLERANCE,
): boolean {
  return Math.abs(deltaX) < tapTolerance && Math.abs(deltaY) < tapTolerance;
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
  if (readingDirection === "rtl") {
    if (x < third) return "next";
    if (x > 2 * third) return "prev";
    return "center";
  }
  if (x < third) return "prev";
  if (x > 2 * third) return "next";
  return "center";
}
