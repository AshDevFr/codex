import { type RefObject, useCallback, useRef } from "react";
import {
  clampPan,
  focalZoom,
  IDENTITY,
  MIN_SCALE,
  type Point,
  type Size,
  type ZoomTransform,
} from "../utils/zoomMath";

export interface UsePinchZoomArgs {
  /** Element that defines the viewport (used to measure size for pan bounds). */
  viewportRef: RefObject<HTMLElement | null>;
  /** Element the zoom transform is written to (the current page content). */
  contentRef: RefObject<HTMLElement | null>;
}

export interface PinchZoomController {
  /** Apply an incremental pinch step toward `focus` (relative to element center). */
  pinch: (scaleRatio: number, focus: Point) => void;
  /** Pan by an incremental delta (no-op at fit scale). */
  panBy: (dx: number, dy: number) => void;
  /** Reset to fit (scale 1, centered). */
  reset: () => void;
  /** Whether the page is currently zoomed in (read synchronously, ref-backed). */
  isZoomedNow: () => boolean;
}

const ZOOM_EPSILON = 0.01;

/**
 * Content-only zoom controller. Holds the page transform in a ref and writes it
 * to the content element imperatively (no per-frame React re-render), so pinch
 * and pan stay smooth. The transform math is the pure `zoomMath` module; this
 * hook is the thin DOM glue that measures the viewport and applies the result.
 *
 * `isZoomedNow` reads the ref synchronously so the gesture layer can decide, mid
 * event, whether a one-finger drag should pan (zoomed) or turn the page (fit).
 */
export function usePinchZoom({
  viewportRef,
  contentRef,
}: UsePinchZoomArgs): PinchZoomController {
  const transformRef = useRef<ZoomTransform>(IDENTITY);

  const write = useCallback(
    (next: ZoomTransform) => {
      transformRef.current = next;
      const el = contentRef.current;
      if (el) {
        el.style.transformOrigin = "center center";
        el.style.transform = `translate3d(${next.tx}px, ${next.ty}px, 0) scale(${next.scale})`;
      }
    },
    [contentRef],
  );

  const measure = useCallback((): Size => {
    const el = viewportRef.current;
    if (!el) return { width: 0, height: 0 };
    const rect = el.getBoundingClientRect();
    return { width: rect.width, height: rect.height };
  }, [viewportRef]);

  const pinch = useCallback(
    (scaleRatio: number, focus: Point) => {
      const prev = transformRef.current;
      const zoomed = focalZoom(prev, focus, prev.scale * scaleRatio);
      write({
        scale: zoomed.scale,
        ...clampPan(zoomed, zoomed.scale, measure()),
      });
    },
    [measure, write],
  );

  const panBy = useCallback(
    (dx: number, dy: number) => {
      const prev = transformRef.current;
      if (prev.scale <= MIN_SCALE) return;
      write({
        scale: prev.scale,
        ...clampPan(
          { tx: prev.tx + dx, ty: prev.ty + dy },
          prev.scale,
          measure(),
        ),
      });
    },
    [measure, write],
  );

  const reset = useCallback(() => write(IDENTITY), [write]);

  const isZoomedNow = useCallback(
    () => transformRef.current.scale > MIN_SCALE + ZOOM_EPSILON,
    [],
  );

  return { pinch, panBy, reset, isZoomedNow };
}
