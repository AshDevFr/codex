/**
 * Reader prefetch-window helper (Phase 12 T11).
 *
 * The Comic reader's preload-pages setting (`useReaderStore.settings.preloadPages`)
 * defaults to 1 and is user-clamped to 0-10. That default is fine for desktop
 * with a wired connection but punishes mobile readers on cellular: a tap to
 * the next page hits the network rather than a primed image cache.
 *
 * This helper widens the effective window in two cases:
 *
 * - The book is downloaded (per the IDB downloads store). Every page is in
 *   the SW's CacheFirst route already, so we can preload aggressively at
 *   zero network cost — primes the browser's image decoder.
 * - The book is not downloaded but the user is reading on cellular. Force
 *   a minimum window so the in-session experience is responsive even when
 *   `preloadPages` is set low.
 *
 * Pure function so the React effect in `ComicReader.tsx` can call it inline
 * without an extra hook, and so the unit test can exercise it without a
 * full reader render.
 */

/**
 * Maximum effective preload size. Matches the user-facing slider cap in
 * `readerStore.ts` so the helper never asks for a wider window than the
 * existing UI exposes.
 */
export const MAX_PREFETCH_PAGES = 10;

/**
 * Minimum window when the book is not downloaded. Per the Phase 12 plan:
 * "extend the existing prefetch logic from the current small window to 5-10
 * pages so the in-session experience improves on cellular regardless of
 * download status."
 */
export const MIN_PREFETCH_NOT_DOWNLOADED = 5;

/**
 * Minimum window when the book *is* downloaded. SW cache hits are free, so
 * prime aggressively up to the cap.
 */
export const MIN_PREFETCH_DOWNLOADED = MAX_PREFETCH_PAGES;

export function getEffectivePreloadWindow(
  userSetting: number,
  isDownloaded: boolean,
): number {
  const floor = isDownloaded
    ? MIN_PREFETCH_DOWNLOADED
    : MIN_PREFETCH_NOT_DOWNLOADED;
  const effective = Math.max(userSetting, floor);
  return Math.min(MAX_PREFETCH_PAGES, Math.max(0, effective));
}
