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

// =============================================================================
// Swipe paging (finger-drag page navigation)
//
// These helpers are pure and input-agnostic (numbers in, decision out), mirroring
// `isTap` / `classifyTapZone`. They drive the `SwipePager` filmstrip: when to arm a
// horizontal drag, when the drag must yield to native panning, how to resist past an
// edge, and whether a release commits a page turn.
// =============================================================================

/**
 * Minimum horizontal movement (px) before a pointer drag is treated as a swipe
 * rather than a tap. Larger than {@link TAP_TOLERANCE} is unnecessary: the drag
 * only *arms* here; the commit decision happens at release in {@link decideSnap}.
 */
export const SWIPE_ACTIVATION_PX = 8;

/**
 * Fraction of the viewport width a drag must cover to commit a page turn on
 * release (when not flicked fast). 0.25 = a quarter-screen drag turns the page.
 */
export const SWIPE_COMMIT_FRACTION = 0.25;

/**
 * Release velocity (px/ms) at or above which a flick commits a page turn even if
 * the drag distance is short. Roughly a quick finger flick.
 */
export const SWIPE_VELOCITY_THRESHOLD = 0.4;

/** Above this zoom scale we treat the page as pinch-zoomed (and thus pannable). */
const ZOOM_EPSILON = 0.01;

/**
 * True when the visual viewport is pinch-zoomed in (scale meaningfully above 1).
 * A small epsilon absorbs sub-pixel zoom jitter reported by some browsers.
 */
export function isPinchZoomed(
  visualViewportScale: number,
  epsilon: number = ZOOM_EPSILON,
): boolean {
  return visualViewportScale > 1 + epsilon;
}

/**
 * True when a pointer drag should be treated as a horizontal swipe: it has moved
 * past {@link SWIPE_ACTIVATION_PX} horizontally and the horizontal component
 * strictly dominates the vertical one. A vertical-dominant drag falls through so
 * native scroll / pull-to-refresh / edge-back gestures keep working.
 */
export function isHorizontalDrag(
  deltaX: number,
  deltaY: number,
  activation: number = SWIPE_ACTIVATION_PX,
): boolean {
  return Math.abs(deltaX) >= activation && Math.abs(deltaX) > Math.abs(deltaY);
}

export interface HorizontallyPannableInput {
  /** `window.visualViewport.scale` (1 when not pinch-zoomed). */
  visualViewportScale: number;
  /** Rendered width of the current page content in CSS px. */
  contentWidth: number;
  /** Width of the reader viewport in CSS px. */
  viewportWidth: number;
}

/**
 * True when the page can be panned horizontally and swipe-to-turn must therefore
 * yield to native panning. That is the case when the user has pinch-zoomed in, or
 * when the rendered page is wider than the viewport (e.g. fit modes `original` /
 * `width-shrink` on an over-wide page, which the user pans to read). A 1px slack
 * absorbs sub-pixel rounding.
 */
export function isHorizontallyPannable({
  visualViewportScale,
  contentWidth,
  viewportWidth,
}: HorizontallyPannableInput): boolean {
  if (isPinchZoomed(visualViewportScale)) return true;
  if (contentWidth > viewportWidth + 1) return true;
  return false;
}

/**
 * Edge resistance: damps a drag that pulls past the first/last spread so it slows
 * and never exceeds the viewport width, then snaps back. Near-identity for small
 * drags, asymptotic to `viewportWidth`. Preserves the sign of `dragPx`.
 */
export function rubberBand(dragPx: number, viewportWidth: number): number {
  if (viewportWidth <= 0) return 0;
  const sign = Math.sign(dragPx);
  const distance = Math.abs(dragPx);
  return sign * viewportWidth * (1 - 1 / (distance / viewportWidth + 1));
}

export interface SnapDecisionInput {
  /** Signed horizontal drag at release (px). Positive = finger moved right. */
  dragPx: number;
  /** Signed release velocity (px/ms). Positive = moving right. */
  velocityPxPerMs: number;
  /** Reader viewport width (px), used for the distance threshold. */
  viewportWidth: number;
  /** Whether a previous spread exists (false at the first spread). */
  hasPrev: boolean;
  /** Whether a next spread exists (false at the last spread). */
  hasNext: boolean;
  /** Reading direction; flips swipe polarity for RTL. */
  readingDirection: "ltr" | "rtl" | "ttb" | "webtoon";
}

export type SnapResult = "next" | "prev" | "stay";

/**
 * Decide what a released swipe does: turn to the next/previous page or snap back.
 *
 * A turn commits when the drag covered at least {@link SWIPE_COMMIT_FRACTION} of
 * the viewport width OR the release was a fast flick ({@link SWIPE_VELOCITY_THRESHOLD}).
 * Direction comes from the flick velocity when fast, otherwise from the drag sign:
 * a leftward gesture turns forward in LTR (mirrored in RTL). If the resulting turn
 * has no page to go to (at an edge), it stays.
 */
export function decideSnap({
  dragPx,
  velocityPxPerMs,
  viewportWidth,
  hasPrev,
  hasNext,
  readingDirection,
}: SnapDecisionInput): SnapResult {
  const fast = Math.abs(velocityPxPerMs) >= SWIPE_VELOCITY_THRESHOLD;
  const far = Math.abs(dragPx) >= SWIPE_COMMIT_FRACTION * viewportWidth;
  if (!fast && !far) return "stay";

  // A fast flick wins the direction even if the drag was reversed; otherwise the
  // drag distance decides.
  const movingLeft = fast ? velocityPxPerMs < 0 : dragPx < 0;

  // Leftward = forward (next) in LTR; mirrored in RTL.
  const isNext = readingDirection === "rtl" ? !movingLeft : movingLeft;

  if (isNext) return hasNext ? "next" : "stay";
  return hasPrev ? "prev" : "stay";
}

// =============================================================================
// Vertical dismiss (swipe down to exit the reader)
// =============================================================================

/**
 * Fraction of the viewport height a downward drag must cover to dismiss (exit)
 * the reader on release when it was not a fast flick. ~0.18 = a fifth-screen
 * pull-down.
 */
export const SWIPE_DOWN_COMMIT_FRACTION = 0.18;

/**
 * Downward release velocity (px/ms) at or above which a flick dismisses the
 * reader even if the drag was short — provided it still cleared
 * {@link SWIPE_DOWN_MIN_PX}.
 */
export const SWIPE_DOWN_VELOCITY_THRESHOLD = 0.5;

/**
 * Minimum downward travel (px) before a fast flick may dismiss. Keeps a quick
 * tap-flick with a few pixels of downward jitter from exiting by accident.
 */
export const SWIPE_DOWN_MIN_PX = 64;

export interface DownwardExitInput {
  /** Signed horizontal drag at release (px). */
  dragX: number;
  /** Signed vertical drag at release (px). Positive = finger moved down. */
  dragY: number;
  /** Signed vertical release velocity (px/ms). Positive = moving down. */
  velocityYPxPerMs: number;
  /** Reader viewport height (px), used for the distance threshold. */
  viewportHeight: number;
}

/**
 * Decide whether a released drag is a deliberate downward fling that should
 * dismiss (exit) the reader.
 *
 * Requires the drag to move downward and be vertical-dominant (so a horizontal
 * page-turn swipe never doubles as an exit), then commits when it either covered
 * at least {@link SWIPE_DOWN_COMMIT_FRACTION} of the viewport height OR was a
 * fast downward flick that still cleared {@link SWIPE_DOWN_MIN_PX}.
 */
export function isDownwardExit({
  dragX,
  dragY,
  velocityYPxPerMs,
  viewportHeight,
}: DownwardExitInput): boolean {
  if (dragY <= 0) return false; // must travel downward
  if (dragY <= Math.abs(dragX)) return false; // vertical-dominant
  const far = dragY >= SWIPE_DOWN_COMMIT_FRACTION * viewportHeight;
  const flick =
    velocityYPxPerMs >= SWIPE_DOWN_VELOCITY_THRESHOLD &&
    dragY >= SWIPE_DOWN_MIN_PX;
  return far || flick;
}
