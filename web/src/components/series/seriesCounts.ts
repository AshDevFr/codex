/**
 * Pure helper that formats the series count string shown on the detail header
 * and the library card hover panel.
 *
 * The line is built from up to three independent segments:
 *
 *     <bookCount> books · <localMaxVolume>/<totalVolumeCount> vol · <localMaxChapter>/<totalChapterCount> ch
 *
 * They measure three different things and are deliberately never reconciled:
 * `bookCount` counts files on disk, while the volume and chapter segments
 * report the highest unit number owned on that axis against the provider's
 * expected total. A series with 14 volumes plus 25 loose chapters is
 * `39 books · 14/17 vol · 150/223 ch` — the segments do not sum.
 *
 * Two rules keep the line honest for every library shape:
 *
 *  - `bookCount` is never borrowed as a numerator. A chapter-organized series
 *    with 225 files is not "225 volumes", so an axis whose books carry no
 *    `book_metadata.volume` / `.chapter` shows its total alone (`8 vol total`)
 *    rather than a position that was never measured.
 *  - No axis is inferred from the other. Owning 2 volumes says nothing about
 *    how many of the 169 upstream chapters are on disk, so that stays
 *    `2 vol · 169 ch total`.
 *
 * An axis with neither a local maximum nor a total is dropped entirely.
 */

export interface SeriesCountInputs {
  /** Local count of books on disk (i.e., `series.bookCount`). */
  bookCount: number | null | undefined;
  /**
   * Highest `book_metadata.volume` across the series's books, or null when
   * none of the books have `volume` populated.
   */
  localMaxVolume?: number | null | undefined;
  /**
   * Highest `book_metadata.chapter` across the series's books (may be
   * fractional), or null when none of the books have `chapter` populated.
   */
  localMaxChapter?: number | null | undefined;
  /** Provider's expected volume total. Null when it didn't expose one. */
  totalVolumeCount?: number | null | undefined;
  /** Provider's expected chapter total (may be fractional). */
  totalChapterCount?: number | null | undefined;
}

/**
 * Format a chapter count: `109` stays `109`, `109.5` stays `109.5`.
 *
 * JavaScript's number formatting already drops a trailing `.0`, so this is a
 * named seam for the intent rather than a conversion.
 */
export function formatChapterCount(value: number): string {
  return value.toString();
}

/**
 * Build one axis segment: `<max>/<total> unit`, `<max> unit`, or
 * `<total> unit total` when nothing local was measured. Returns null when the
 * axis is silent on both counts.
 */
function formatAxis(
  localMax: number | null | undefined,
  total: number | null | undefined,
  unit: "vol" | "ch",
): string | null {
  const hasMax = typeof localMax === "number";
  const hasTotal = typeof total === "number";

  if (hasMax) {
    return hasTotal
      ? `${formatChapterCount(localMax)}/${formatChapterCount(total)} ${unit}`
      : `${formatChapterCount(localMax)} ${unit}`;
  }
  if (hasTotal) {
    return `${formatChapterCount(total)} ${unit} total`;
  }
  return null;
}

/**
 * Build the human-readable count string for the series detail header and the
 * card hover panel, or null when no segment applies (caller hides the line).
 *
 * Callers without upstream totals (the hover panel reads the series *list*
 * response, which doesn't carry them) simply omit `totalVolumeCount` /
 * `totalChapterCount` and get the local-only form: `20 books · 14 vol · 109.5 ch`.
 */
export function formatSeriesCounts(inputs: SeriesCountInputs): string | null {
  const {
    bookCount,
    localMaxVolume,
    localMaxChapter,
    totalVolumeCount,
    totalChapterCount,
  } = inputs;

  const segments: string[] = [];

  if (typeof bookCount === "number") {
    segments.push(`${bookCount} book${bookCount === 1 ? "" : "s"}`);
  }

  const volumePart = formatAxis(localMaxVolume, totalVolumeCount, "vol");
  if (volumePart) {
    segments.push(volumePart);
  }

  const chapterPart = formatAxis(localMaxChapter, totalChapterCount, "ch");
  if (chapterPart) {
    segments.push(chapterPart);
  }

  return segments.length > 0 ? segments.join(" · ") : null;
}
