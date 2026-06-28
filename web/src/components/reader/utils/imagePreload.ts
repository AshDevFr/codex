/**
 * Preload an image and, crucially, **decode** it off-screen so it can be painted
 * without a delay later.
 *
 * Merely fetching an image (`img.src = url` + waiting for `onload`) puts its
 * bytes in the cache but leaves it undecoded. When such an image is then
 * revealed, e.g. a filmstrip neighbour sliding into view during a swipe, the
 * browser decodes it on the spot and paints the page background (black) for a
 * frame first. Awaiting `HTMLImageElement.decode()` warms the decoded-bitmap
 * cache so the reveal is instant.
 *
 * `decode()` is feature-detected and best-effort: if it is unavailable or
 * rejects (it can reject on `src` churn), we fall back to the load event. The
 * returned promise resolves with the (loaded/decoded) image, or rejects if the
 * image fails to load.
 *
 * Pass `{ decode: false }` to fetch only (no off-screen decode). Decoding holds
 * a full uncompressed bitmap in memory, so callers preloading a wide window
 * should decode only the immediate neighbors to avoid exhausting memory on
 * constrained devices.
 */
export function preloadImage(
  url: string,
  options: { decode?: boolean } = {},
): Promise<HTMLImageElement> {
  const { decode = true } = options;
  return new Promise((resolve, reject) => {
    const img = new Image();
    const done = () => resolve(img);
    img.onerror = () => reject(new Error(`Failed to preload image: ${url}`));
    img.src = url;

    if (decode && typeof img.decode === "function") {
      img.decode().then(done, () => {
        // decode() rejected (e.g. aborted by a src change). If the image already
        // finished loading, use it; otherwise wait for the load event.
        if (img.complete) done();
        else img.onload = done;
      });
    } else {
      img.onload = done;
    }
  });
}
