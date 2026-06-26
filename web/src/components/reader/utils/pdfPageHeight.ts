import type { PdfZoomLevel } from "../PdfReader";

export interface PdfPageOrig {
  /** Intrinsic page width in PDF points (from react-pdf's onLoadSuccess). */
  width: number;
  /** Intrinsic page height in PDF points. */
  height: number;
}

/** Scale factor applied by each fixed-zoom level (fit modes are width-driven). */
const ZOOM_SCALE: Partial<Record<PdfZoomLevel, number>> = {
  "50%": 0.5,
  "75%": 0.75,
  "100%": 1,
  "125%": 1.25,
  "150%": 1.5,
  "200%": 2,
};

/**
 * Compute the exact rendered height (px) a PDF page will occupy in the
 * continuous reader, given its intrinsic dimensions and the current zoom.
 *
 * react-pdf renders a page either at a fixed width (fit modes → the available
 * content width) or at a scale of its intrinsic point size (1 point ≈ 1px at
 * scale 1). Reserving that height before the canvas paints means a page
 * scrolling into the render window — and finishing its draw — never changes the
 * box size, so content above the viewport (and the scroll position) stays put.
 *
 * Returns `null` when it can't be determined (missing/invalid dimensions, or a
 * fit mode before the container width is known).
 */
export function pdfPageReservedHeight({
  zoomLevel,
  availableWidth,
  orig,
}: {
  zoomLevel: PdfZoomLevel;
  /** Width available to the page: container width minus horizontal padding, px. */
  availableWidth: number;
  orig: PdfPageOrig;
}): number | null {
  if (!(orig.width > 0) || !(orig.height > 0)) return null;

  const scale = ZOOM_SCALE[zoomLevel];
  if (scale != null) {
    // Fixed-zoom: height scales with the intrinsic point height.
    return orig.height * scale;
  }

  // Fit modes (fit-page / fit-width): page is drawn at the available width,
  // height follows the aspect ratio.
  if (!(availableWidth > 0)) return null;
  return availableWidth * (orig.height / orig.width);
}
