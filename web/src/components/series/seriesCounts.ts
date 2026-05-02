/**
 * Pure helpers that format series count strings for the detail header.
 *
 * Inputs come from `series.bookCount` (local count) and `series.metadata`
 * (`totalVolumeCount`, `totalChapterCount`). Either total may be null/undefined
 * when the metadata provider didn't expose it.
 */

export interface SeriesCountInputs {
  /** Local count of books on disk (i.e., `series.bookCount`). */
  localCount: number | null | undefined;
  /** Provider's expected volume total. */
  totalVolumeCount: number | null | undefined;
  /** Provider's expected chapter total (may be fractional). */
  totalChapterCount: number | null | undefined;
}

/**
 * Format a chapter count: drop trailing `.0` so `109` shows as `109` and
 * `109.5` shows as `109.5`.
 */
export function formatChapterCount(value: number): string {
  if (Number.isInteger(value)) {
    return value.toString();
  }
  return value.toString();
}

/**
 * Build the human-readable count string for the series detail header.
 *
 * Rules (per the metadata-count-split plan, Phase 6):
 *  - Both totals known: `<local>/<vol> vol · <chap> ch`
 *  - Volume total only: `<local>/<vol> vol` (or `<local> vol` if local missing)
 *  - Chapter total only: `<local>/<chap> ch` (the bug-fix case for
 *    chapter-organized libraries; previously showed `<local>/<vol>` and was
 *    incoherent)
 *  - Neither total known: `<local> books` (legacy display)
 *  - No local + no totals: `null` (caller can hide the line)
 */
export function formatSeriesCounts(inputs: SeriesCountInputs): string | null {
  const { localCount, totalVolumeCount, totalChapterCount } = inputs;

  const hasLocal = typeof localCount === "number";
  const hasVolume = typeof totalVolumeCount === "number";
  const hasChapter = typeof totalChapterCount === "number";

  if (hasVolume && hasChapter) {
    const volumePart = hasLocal
      ? `${localCount}/${totalVolumeCount} vol`
      : `${totalVolumeCount} vol`;
    return `${volumePart} · ${formatChapterCount(totalChapterCount)} ch`;
  }

  if (hasVolume) {
    return hasLocal
      ? `${localCount}/${totalVolumeCount} vol`
      : `${totalVolumeCount} vol`;
  }

  if (hasChapter) {
    return hasLocal
      ? `${localCount}/${formatChapterCount(totalChapterCount)} ch`
      : `${formatChapterCount(totalChapterCount)} ch`;
  }

  if (hasLocal) {
    return `${localCount} books`;
  }

  return null;
}
