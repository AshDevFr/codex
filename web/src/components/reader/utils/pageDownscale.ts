/**
 * Helpers for the optional "downscale pages" setting: request display-sized page
 * images from the server (`?width=`) instead of full-resolution scans, so page
 * turns are cheaper to render (notably on WebKit). See `get_page_image` on the
 * backend for the matching resize.
 */

/** Round requested widths to this bucket so minor viewport changes reuse the
 *  same URL (and thus the browser cache, and the server's per-request resize). */
const WIDTH_BUCKET = 256;
/** Never request a downscaled page narrower than this (keeps small viewports legible). */
const MIN_WIDTH = 640;
/** Cap the requested width; beyond a typical scan there's nothing to gain. */
const MAX_WIDTH = 2560;

/**
 * Device-pixel width to request for a downscaled page, bucketed and clamped.
 *
 * @param viewportCssWidth  The reader viewport width in CSS px.
 * @param devicePixelRatio  `window.devicePixelRatio` (so retina gets enough detail).
 * @param doublePage        True when two pages share the viewport (each gets ~half).
 */
export function downscaleWidth(
  viewportCssWidth: number,
  devicePixelRatio: number,
  doublePage: boolean,
): number {
  const perPageCss = doublePage ? viewportCssWidth / 2 : viewportCssWidth;
  const devicePx = perPageCss * Math.max(1, devicePixelRatio || 1);
  const bucketed = Math.ceil(devicePx / WIDTH_BUCKET) * WIDTH_BUCKET;
  return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, bucketed));
}
