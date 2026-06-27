/**
 * Pure math for content-only page zoom. Input-agnostic (numbers in, transform
 * out), mirroring the `swipeGesture`/`spreadCalculation` helpers so the zoom
 * behavior can be unit-tested without the DOM.
 *
 * Coordinate convention: the page element fills the viewport at fit (scale 1).
 * `tx`/`ty` translate the element in CSS px; the focal point passed to
 * {@link focalZoom} is a screen point measured *relative to the element center*
 * (so 0,0 is dead center). The applied transform is
 * `translate(tx, ty) scale(scale)` with a centered transform-origin.
 */

/** Minimum (fit-to-screen) and maximum zoom factors. */
export const MIN_SCALE = 1;
export const MAX_SCALE = 4;

export interface ZoomTransform {
  scale: number;
  tx: number;
  ty: number;
}

export interface Size {
  width: number;
  height: number;
}

export interface Point {
  x: number;
  y: number;
}

/** The fit (un-zoomed) transform. */
export const IDENTITY: ZoomTransform = { scale: 1, tx: 0, ty: 0 };

const clamp = (value: number, min: number, max: number): number =>
  Math.min(Math.max(value, min), max);

/** Clamp a raw scale into the allowed `[MIN_SCALE, MAX_SCALE]` range. */
export function clampScale(scale: number): number {
  return clamp(scale, MIN_SCALE, MAX_SCALE);
}

/**
 * Clamp a translation so the scaled content can't be panned past its edges into
 * empty space. At fit (scale 1) there is no overflow, so translation pins to 0.
 */
export function clampPan(
  t: { tx: number; ty: number },
  scale: number,
  viewport: Size,
): { tx: number; ty: number } {
  const maxX = Math.max(0, (viewport.width * (scale - 1)) / 2);
  const maxY = Math.max(0, (viewport.height * (scale - 1)) / 2);
  return {
    tx: clamp(t.tx, -maxX, maxX),
    ty: clamp(t.ty, -maxY, maxY),
  };
}

/**
 * Zoom toward a focal point, keeping the content under that point stationary.
 *
 * The focus is a screen point relative to the element center. For a content
 * point `p` that maps to screen `f = p*s0 + t0`, keeping `f` fixed at the new
 * scale gives `t1 = f - (f - t0) * (s1 / s0)`. Scale is clamped; callers should
 * follow with {@link clampPan} once they know the viewport size.
 */
export function focalZoom(
  prev: ZoomTransform,
  focus: Point,
  nextScaleRaw: number,
): ZoomTransform {
  const scale = clampScale(nextScaleRaw);
  const ratio = scale / prev.scale;
  return {
    scale,
    tx: focus.x - (focus.x - prev.tx) * ratio,
    ty: focus.y - (focus.y - prev.ty) * ratio,
  };
}
