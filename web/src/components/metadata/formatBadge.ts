/**
 * Maps a plugin-supplied `format` discriminator (e.g. `manga`, `novel`,
 * `light_novel`, `manhwa`) to a Mantine badge color and a human-readable
 * label so the metadata search modal can render visually distinct badges
 * for visually-identical results.
 *
 * The mapping is intentionally small: it covers the recommended vocabulary
 * documented on the plugin protocol. Anything outside the known set falls
 * back to a neutral `gray` badge with the raw value title-cased so unknown
 * formats from new plugins still render sensibly.
 */

export interface FormatBadgeStyle {
  color: string;
  label: string;
}

const KNOWN_FORMATS: Record<string, FormatBadgeStyle> = {
  manga: { color: "grape", label: "Manga" },
  manhwa: { color: "grape", label: "Manhwa" },
  manhua: { color: "grape", label: "Manhua" },
  webtoon: { color: "grape", label: "Webtoon" },
  one_shot: { color: "grape", label: "One Shot" },
  novel: { color: "teal", label: "Novel" },
  light_novel: { color: "teal", label: "Light Novel" },
  comic: { color: "orange", label: "Comic" },
};

/**
 * Title-case a snake_case or lowercase string for display in the fallback
 * badge: `light_novel` → `Light Novel`, `oel` → `Oel`.
 */
function titleCase(raw: string): string {
  return raw
    .split(/[_\s]+/)
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1).toLowerCase())
    .join(" ");
}

/**
 * Resolve a plugin-supplied format string to a badge color/label pair.
 * Lookups are case-insensitive against the known-set; unknown values get a
 * neutral `gray` badge with the raw string title-cased.
 *
 * Returns `null` for empty/whitespace-only input so the caller can omit
 * the badge entirely.
 */
export function resolveFormatBadge(
  format: string | null | undefined,
): FormatBadgeStyle | null {
  if (!format) return null;
  const trimmed = format.trim();
  if (trimmed.length === 0) return null;

  const known = KNOWN_FORMATS[trimmed.toLowerCase()];
  if (known) return known;

  return { color: "gray", label: titleCase(trimmed) };
}
