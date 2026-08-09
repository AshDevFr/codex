/**
 * Seed resolution: Codex library entries to MangaBaka series IDs.
 *
 * The host curates which library entries become seeds (highest-rated first,
 * then most recently read, capped at its `max_seeds` setting). This module only
 * has to turn those entries into MangaBaka IDs.
 */

import type { UserLibraryEntry } from "@ashdev/codex-plugin-sdk";
import { logger } from "./logger.js";
import { EXTERNAL_ID_SOURCE_MANGABAKA } from "./manifest.js";

/**
 * Legacy source name, written before the `api:<service>` convention landed.
 * Still accepted so libraries matched by an older build keep working.
 */
const LEGACY_EXTERNAL_ID_SOURCE = "mangabaka";

/** A library entry successfully resolved to a MangaBaka series. */
export interface ResolvedSeed {
  /** MangaBaka series ID. */
  id: number;
  /** Codex title, used for `basedOn` and reason text. */
  title: string;
  /** User's rating on Codex's 0-100 scale, or 0 when unrated. */
  rating: number;
}

export interface SeedResolution {
  /**
   * Resolved seeds, in the host's order. That order is meaningful: the host
   * sorts rated entries by rating descending, so taking the first K yields the
   * user's favourites without re-sorting.
   */
  seeds: ResolvedSeed[];
  /** How many entries carried no usable MangaBaka ID. */
  unresolved: number;
  /**
   * Every MangaBaka ID present in the library, resolvable as a seed or not.
   *
   * Distinct from `seeds` because the host only excludes series the user has
   * *read*; series owned but unread still surface as recommendations and need
   * to be flagged rather than dropped.
   */
  libraryIds: Set<number>;
}

/**
 * Parse an external ID string into a positive MangaBaka series ID.
 *
 * `Number.parseInt` is deliberately avoided: it accepts "12abc" as 12, which
 * would silently seed the probe with a wrong series.
 */
function parseSeriesId(raw: string | undefined): number | null {
  if (!raw) return null;
  const trimmed = raw.trim();
  if (!/^\d+$/.test(trimmed)) return null;

  const id = Number(trimmed);
  return Number.isSafeInteger(id) && id > 0 ? id : null;
}

/** Extract the MangaBaka series ID from a library entry, if it has one. */
function mangaBakaIdOf(entry: UserLibraryEntry): number | null {
  const externalIds = entry?.externalIds;
  if (!Array.isArray(externalIds)) return null;

  // Prefer the canonical source; only fall back to the legacy name if the
  // canonical one is absent or unparseable.
  const canonical = externalIds.find((e) => e?.source === EXTERNAL_ID_SOURCE_MANGABAKA);
  const fromCanonical = parseSeriesId(canonical?.externalId);
  if (fromCanonical !== null) return fromCanonical;

  const legacy = externalIds.find((e) => e?.source === LEGACY_EXTERNAL_ID_SOURCE);
  return parseSeriesId(legacy?.externalId);
}

/**
 * Resolve library entries to MangaBaka seeds.
 *
 * There is deliberately **no title-search fallback**. The AniList provider can
 * afford one because each seed produces its own independent recommendation
 * list, so a bad match costs one list. Here every seed is folded into a single
 * probe vector, so one wrong match skews every result. Skipping unmatched
 * entries yields fewer recommendations; guessing yields wrong ones.
 */
export function resolveSeeds(library: UserLibraryEntry[]): SeedResolution {
  const seeds: ResolvedSeed[] = [];
  const libraryIds = new Set<number>();
  // Index into `seeds`, so a duplicate can update the existing entry in place
  // without disturbing the host's ordering.
  const seenSeeds = new Map<number, number>();
  let unresolved = 0;

  for (const entry of library) {
    const id = mangaBakaIdOf(entry);
    if (id === null) {
      unresolved++;
      continue;
    }

    libraryIds.add(id);

    const rating = entry.userRating ?? 0;
    const existingIndex = seenSeeds.get(id);
    if (existingIndex !== undefined) {
      // Two Codex series can point at the same MangaBaka entry. Sending the ID
      // twice would double its pull on the probe vector, so merge instead and
      // keep the stronger signal.
      const existing = seeds[existingIndex];
      if (rating > existing.rating) {
        seeds[existingIndex] = { ...existing, rating };
      }
      continue;
    }

    seenSeeds.set(id, seeds.length);
    seeds.push({ id, title: entry.title, rating });
  }

  if (unresolved > 0) {
    logger.info(
      `${unresolved} of ${library.length} library entries have no MangaBaka ID and were skipped as seeds. ` +
        "Run a metadata match with the MangaBaka Metadata plugin to use them.",
    );
  }

  return { seeds, unresolved, libraryIds };
}
