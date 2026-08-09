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
import type { MbSeries } from "./types.js";

/**
 * Legacy source name, written before the `api:<service>` convention landed.
 * Still accepted so libraries matched by an older build keep working.
 */
const LEGACY_EXTERNAL_ID_SOURCE = "mangabaka";

/**
 * Translate a MangaBaka `source` block key into the Codex external ID source
 * name. `metadata-mangabaka` derives its cross-reference IDs the same way
 * (`anime_planet` becomes `api:animeplanet`), so the two agree by construction.
 */
function codexSourceName(mangaBakaSourceKey: string): string {
  return `api:${mangaBakaSourceKey.replace(/_/g, "")}`;
}

/**
 * Which MangaBaka series the user already has, by any identity Codex knows.
 *
 * Matching on the MangaBaka ID alone would miss series matched by a different
 * provider: a library entry carrying only an AniList ID is still the same work,
 * and recommending it back would be wrong. MangaBaka returns a `source` block of
 * cross-service IDs on every result, which closes that gap.
 */
export class LibraryIndex {
  private readonly mangaBakaIds = new Set<number>();
  /** Codex external ID source name to the IDs held under it. */
  private readonly bySource = new Map<string, Set<string>>();

  /** How many MangaBaka IDs are indexed. */
  get size(): number {
    return this.mangaBakaIds.size;
  }

  /** Index a MangaBaka series ID directly. */
  addMangaBakaId(id: number): void {
    this.mangaBakaIds.add(id);
  }

  /** Index every external ID a library entry carries. */
  addEntry(entry: UserLibraryEntry): void {
    const externalIds = entry?.externalIds;
    if (!Array.isArray(externalIds)) return;

    for (const external of externalIds) {
      if (typeof external?.source !== "string" || typeof external?.externalId !== "string") {
        continue;
      }
      let ids = this.bySource.get(external.source);
      if (!ids) {
        ids = new Set();
        this.bySource.set(external.source, ids);
      }
      ids.add(external.externalId);
    }
  }

  /** Whether this series is already in the user's library. */
  has(series: MbSeries): boolean {
    if (this.mangaBakaIds.has(series.id)) return true;

    for (const [key, entry] of Object.entries(series.source ?? {})) {
      if (entry?.id == null) continue;

      const ids = this.bySource.get(codexSourceName(key));
      if (ids?.has(String(entry.id))) return true;
    }

    return false;
  }
}

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
   * Everything the user already has, indexed by MangaBaka ID and by every other
   * external ID their library carries.
   *
   * Distinct from `seeds` because the host only excludes series the user has
   * *read*; series owned but unread still surface as recommendations and need
   * to be flagged rather than dropped.
   */
  library: LibraryIndex;
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
export function resolveSeeds(entries: UserLibraryEntry[]): SeedResolution {
  const seeds: ResolvedSeed[] = [];
  const library = new LibraryIndex();
  // Index into `seeds`, so a duplicate can update the existing entry in place
  // without disturbing the host's ordering.
  const seenSeeds = new Map<number, number>();
  let unresolved = 0;

  for (const entry of entries) {
    // Indexed regardless of whether it resolves to a seed: an entry matched
    // only against AniList still means the user has that series.
    library.addEntry(entry);

    const id = mangaBakaIdOf(entry);
    if (id === null) {
      unresolved++;
      continue;
    }

    library.addMangaBakaId(id);

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
      `${unresolved} of ${entries.length} library entries have no MangaBaka ID and were skipped as seeds. ` +
        "Run a metadata match with the MangaBaka Metadata plugin to use them.",
    );
  }

  return { seeds, unresolved, library };
}
