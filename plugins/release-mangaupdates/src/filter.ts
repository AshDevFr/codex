/**
 * Filtering: language allowlist + group blocklist.
 *
 * Filters are applied client-side in the plugin (before recording) for two
 * reasons:
 *   1. Keeps the ledger small. Out-of-language items would be dropped by the
 *      host anyway via the latest_known_* gate, but writing them to the
 *      ledger pollutes the inbox and wastes write IO.
 *   2. Keeps the inbox clean. Users who configure `["en"]` don't want to see
 *      Spanish entries hidden behind a state flag — they want them gone.
 */

import { type ParsedRssItem, UNKNOWN_LANGUAGE } from "./parser.js";

/**
 * Resolved, normalized filter inputs for a single series. Both lists are
 * lowercased + trimmed. Empty `languages` is interpreted as "no filter"
 * (everything passes), but the caller is expected to pass at least the
 * server-wide default to avoid that footgun.
 */
export interface ResolvedFilters {
  /** Lowercased ISO 639-1 codes; empty = no filter. */
  languages: string[];
  /** Lowercased group names; case-insensitive exact match against `group`. */
  blockedGroups: Set<string>;
  /**
   * Whether to include items whose language couldn't be detected
   * (`UNKNOWN_LANGUAGE` sentinel). Default false — be conservative.
   */
  includeUnknownLanguage: boolean;
}

/**
 * Build resolved filters from raw config strings + lists. Centralizes the
 * normalization so the poll handler doesn't have to care about casing or
 * whitespace.
 */
export function resolveFilters(input: {
  languages: string[];
  blockedGroups: string[];
  includeUnknownLanguage?: boolean;
}): ResolvedFilters {
  const languages = dedupePreserveOrder(
    input.languages.map((s) => s.trim().toLowerCase()).filter((s) => s.length > 0),
  );
  const blockedGroups = new Set(
    input.blockedGroups.map((s) => s.trim().toLowerCase()).filter((s) => s.length > 0),
  );
  return {
    languages,
    blockedGroups,
    includeUnknownLanguage: input.includeUnknownLanguage ?? false,
  };
}

/**
 * Parse a comma-separated string into a clean list (trim, drop empties).
 * Helper for `blockedGroups` which is admin-config typed as a single string.
 */
export function parseCommaList(raw: unknown): string[] {
  if (typeof raw !== "string") return [];
  return raw
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

/**
 * Returns true if the item should be kept.
 *
 * Language filter:
 *   - If `languages` is empty → pass.
 *   - Otherwise, item.language must be in the list (case-insensitive).
 *   - `unknown` language is rejected unless `includeUnknownLanguage` is true.
 *
 * Group filter:
 *   - If `group` is null → pass (we have nothing to match against).
 *   - Otherwise, group must NOT be in `blockedGroups`.
 */
export function passesFilters(item: ParsedRssItem, filters: ResolvedFilters): boolean {
  // Language gate.
  if (item.language === UNKNOWN_LANGUAGE) {
    if (!filters.includeUnknownLanguage) return false;
  } else if (filters.languages.length > 0) {
    if (!filters.languages.includes(item.language.toLowerCase())) return false;
  }

  // Group blocklist.
  if (item.group !== null && filters.blockedGroups.size > 0) {
    if (filters.blockedGroups.has(item.group.trim().toLowerCase())) return false;
  }

  return true;
}

function dedupePreserveOrder(xs: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const x of xs) {
    if (!seen.has(x)) {
      seen.add(x);
      out.push(x);
    }
  }
  return out;
}
