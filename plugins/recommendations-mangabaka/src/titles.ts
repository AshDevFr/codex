/**
 * Title normalisation, used to recognise that two entries belong to the same
 * work when MangaBaka's relationship data does not say so.
 *
 * The output is a comparison key only. It is never shown to the user, and it is
 * only ever compared for exact equality: prefix or fuzzy matching would collapse
 * genuinely distinct series ("Solo Leveling" and "Solo Leveling Ragnarok") into
 * one, which is a worse failure than missing a duplicate.
 *
 * Punctuation becomes a space rather than being deleted, so "Re:Zero" and
 * "Re: Zero" agree without also merging distinct words across a separator.
 */

/**
 * Words that introduce a volume or instalment marker.
 *
 * Only counts when followed by a number, so "The Final Chapter" keeps its
 * title while "Chapter 4: The Sanctuary" is recognised as an instalment of
 * whatever came before it.
 */
const VOLUME_MARKER =
  /[,\s\-:]*\b(?:chapter|chapitre|volume|vol|part|season|arc|book)\b\.?\s*\d+.*$/i;

/** Characters that separate words but are not themselves meaningful. */
const SEPARATORS = /[-–—_/\\|~:;,.!?"'`´’‘“”()[\]{}<>*&^%$#@+=]/g;

/**
 * Reduce a title to a comparison key.
 *
 * Steps, in order: strip diacritics, cut any trailing volume or chapter marker,
 * drop punctuation, lowercase, and collapse whitespace.
 */
export function normalizeTitle(raw: string | null | undefined): string {
  if (typeof raw !== "string") return "";

  // Decompose so accents become separate combining marks, then drop them. This
  // makes competing romanisations ("Ōkami" and "Okami") agree.
  //
  // Recomposing with NFC afterwards is essential, not cosmetic: NFD also splits
  // Hangul syllables into jamo, so a Korean title would otherwise compare
  // unequal to its own composed form despite rendering identically.
  const deaccented = raw.normalize("NFD").replace(/[̀-ͯ]/g, "").normalize("NFC");

  // Cut the instalment suffix before removing punctuation, because the marker
  // pattern relies on the surrounding commas, colons, and dashes.
  const withoutVolume = deaccented.replace(VOLUME_MARKER, "");

  const cleaned = withoutVolume.replace(SEPARATORS, " ").toLowerCase().replace(/\s+/g, " ").trim();

  // A title that is *only* a volume marker ("Chapter 1", "Vol. 5") would reduce
  // to nothing and bucket every such entry together. Fall back to the raw form.
  if (cleaned.length === 0) {
    return deaccented.replace(SEPARATORS, " ").toLowerCase().replace(/\s+/g, " ").trim();
  }

  return cleaned;
}
