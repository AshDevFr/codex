import type { FitMode } from "@/store/readerStore";

export interface PageDimension {
  /** Real page width in pixels (from backend analysis). */
  width: number;
  /** Real page height in pixels (from backend analysis). */
  height: number;
}

export interface ReservedPageHeightParams {
  fitMode: FitMode;
  /** Width available to the image: container content box minus side padding, px. */
  contentWidth: number;
  /** Scroll-container height, used by viewport-relative fit modes (height/screen), px. */
  viewportHeight: number;
  /** The page's real pixel dimensions. */
  dimension: PageDimension;
}

/**
 * Compute the exact rendered height (px) a page image will occupy in the
 * continuous (webtoon) reader, given its real pixel dimensions and the current
 * layout/fit mode.
 *
 * Reserving this height *before* the image loads means the placeholder box is
 * already the right size, so the image loads into space that is already held —
 * zero layout shift, and therefore no scroll-position jump. This is what lets
 * us drop the fragile after-the-fact `scrollTop` compensation: there is simply
 * nothing to compensate for.
 *
 * Returns `null` when the height can't be determined (missing/invalid
 * dimensions, or a mode whose height needs a measurement we don't have yet),
 * in which case the caller falls back to a measured/estimated height.
 */
export function reservedPageHeight({
  fitMode,
  contentWidth,
  viewportHeight,
  dimension,
}: ReservedPageHeightParams): number | null {
  const { width: w, height: h } = dimension;
  if (!(w > 0) || !(h > 0)) return null;
  const aspect = h / w; // rendered height per unit of rendered width

  switch (fitMode) {
    case "width":
      // `width: 100%` — image always fills the content width (scales up/down).
      return contentWidth > 0 ? contentWidth * aspect : null;

    case "width-shrink": {
      // `max-width: 100%; height: auto` — natural width, capped to content width.
      if (!(contentWidth > 0)) return null;
      const renderedWidth = Math.min(w, contentWidth);
      return renderedWidth * aspect;
    }

    case "original":
      // Natural size: 1 image pixel maps to 1 CSS pixel.
      return h;

    case "height":
      // `height: 100vh` — height is pinned to the viewport regardless of aspect.
      return viewportHeight > 0 ? viewportHeight : null;

    case "screen": {
      // `max-width: 100%; max-height: 100vh; object-fit: contain` — scaled down
      // (never up) to fit within both constraints, preserving aspect ratio.
      if (!(contentWidth > 0) || !(viewportHeight > 0)) return null;
      const scale = Math.min(1, contentWidth / w, viewportHeight / h);
      return h * scale;
    }

    default:
      // Same geometry as width-shrink (the default <img> styling).
      return contentWidth > 0 ? Math.min(w, contentWidth) * aspect : null;
  }
}
