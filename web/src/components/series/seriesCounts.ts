/**
 * Pure helpers that format series count strings for the detail header.
 *
 * Inputs come from `series.bookCount` (local count) and `series.metadata`
 * (`totalVolumeCount`, `totalChapterCount`). Either total may be null/undefined
 * when the metadata provider didn't expose it.
 *
 * `localMaxVolume` and `localMaxChapter` are per-series aggregates (Phase 13)
 * derived from `book_metadata.volume` / `book_metadata.chapter`. When present,
 * the numerator switches from "files on disk" to "highest known unit number"
 * so a series with `v01..v14 + v15-c126` correctly displays `14/17 vol` rather
 * than `15/17 vol`.
 */

export interface SeriesCountInputs {
  /** Local count of books on disk (i.e., `series.bookCount`). */
  localCount: number | null | undefined;
  /** Provider's expected volume total. */
  totalVolumeCount: number | null | undefined;
  /** Provider's expected chapter total (may be fractional). */
  totalChapterCount: number | null | undefined;
  /**
   * Highest `book_metadata.volume` across the series's books, or null when
   * none of the books have `volume` populated. When present, replaces
   * `localCount` as the numerator on the volume axis.
   */
  localMaxVolume?: number | null | undefined;
  /**
   * Highest `book_metadata.chapter` across the series's books, or null when
   * none of the books have `chapter` populated. When present, replaces the
   * unconditional `<total> ch` chapter part with `<localMaxChapter>/<total> ch`.
   */
  localMaxChapter?: number | null | undefined;
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
 * Rules (per the metadata-count-split plan, Phases 6 + 13):
 *  - Both totals known: `<local>/<vol> vol · <chap> ch` (chapter part gains a
 *    numerator only when `localMaxChapter` is provided)
 *  - Volume total only: `<local>/<vol> vol` (or `<local> vol` if local missing).
 *    `localMaxVolume` overrides the local-file-count numerator when present.
 *  - Chapter total only: `<local>/<chap> ch`. `localMaxChapter` overrides the
 *    local-file-count numerator when present.
 *  - Neither total known: `<local> books` (legacy display)
 *  - No local + no totals: `null` (caller can hide the line)
 */
export function formatSeriesCounts(inputs: SeriesCountInputs): string | null {
  const {
    localCount,
    totalVolumeCount,
    totalChapterCount,
    localMaxVolume,
    localMaxChapter,
  } = inputs;

  const hasLocal = typeof localCount === "number";
  const hasVolume = typeof totalVolumeCount === "number";
  const hasChapter = typeof totalChapterCount === "number";
  const hasMaxVolume = typeof localMaxVolume === "number";
  const hasMaxChapter = typeof localMaxChapter === "number";

  // Choose the volume numerator: prefer the structured max-volume aggregate
  // over the file count. Falls back to "no numerator" when neither is known.
  const volumeNumerator: number | null = hasMaxVolume
    ? (localMaxVolume as number)
    : hasLocal
      ? (localCount as number)
      : null;

  if (hasVolume && hasChapter) {
    const volumePart =
      volumeNumerator !== null
        ? `${volumeNumerator}/${totalVolumeCount} vol`
        : `${totalVolumeCount} vol`;
    const chapterPart = hasMaxChapter
      ? `${formatChapterCount(localMaxChapter as number)}/${formatChapterCount(totalChapterCount)} ch`
      : `${formatChapterCount(totalChapterCount)} ch`;
    return `${volumePart} · ${chapterPart}`;
  }

  if (hasVolume) {
    return volumeNumerator !== null
      ? `${volumeNumerator}/${totalVolumeCount} vol`
      : `${totalVolumeCount} vol`;
  }

  if (hasChapter) {
    // Chapter-only branch: the bug-fix case for chapter-organized libraries.
    // Prefer the structured max-chapter aggregate; otherwise fall back to the
    // local file count (legacy) so the line is not entirely numerator-less.
    const chapterNumerator: number | null = hasMaxChapter
      ? (localMaxChapter as number)
      : hasLocal
        ? (localCount as number)
        : null;
    return chapterNumerator !== null
      ? `${formatChapterCount(chapterNumerator)}/${formatChapterCount(totalChapterCount)} ch`
      : `${formatChapterCount(totalChapterCount)} ch`;
  }

  if (hasLocal) {
    return `${localCount} books`;
  }

  return null;
}
