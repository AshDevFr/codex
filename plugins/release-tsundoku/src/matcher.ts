/**
 * Match Tsundoku feed items to tracked Codex series by external ID — using
 * *weighted voting* across providers rather than trusting a single ID.
 *
 * Why voting: provider IDs vary in quality. MangaBaka is an aggregation hub
 * with reliably 1:1 IDs; others (MAL, MangaUpdates, …) occasionally share or
 * merge IDs across distinct series, so a lone matching ID can be a false
 * positive. So for each candidate we tally the providers the feed item and the
 * Codex series *both* carry: a shared ID that agrees adds its weight, one that
 * disagrees subtracts it. A series matches only when agreement outweighs
 * disagreement — a trusted disagreement (e.g. different MangaBaka IDs) vetoes a
 * sloppy agreement (e.g. a shared MAL ID).
 *
 * Codex's `releases/record` keys on a `codexSeriesId`, so matching is done
 * here, plugin-side, over the full ID sets both the host and the feed expose.
 */

import type { TrackedSeriesEntry } from "@ashdev/codex-plugin-sdk";
import type { FeedItem } from "./fetcher.js";

/**
 * Vote weight per provider — higher means more trusted as a match signal.
 * MangaBaka leads (its IDs are reliably 1:1), AniList next; the rest default
 * to 1. Tune here if real data shows a source is noisier than assumed.
 */
export const PROVIDER_WEIGHTS: Record<string, number> = {
  mangabaka: 3,
  anilist: 2,
};
const DEFAULT_WEIGHT = 1;

function weightOf(provider: string): number {
  return PROVIDER_WEIGHTS[provider] ?? DEFAULT_WEIGHT;
}

/** Result of resolving a feed item to a tracked Codex series. */
export interface MatchResult {
  /** The Codex series UUID the candidate should be recorded against. */
  codexSeriesId: string;
  /** Net vote score (agreeing weights minus disagreeing). Always `> 0`. */
  score: number;
  /** Host confidence in `[0.8, 1.0]`, derived from the score. */
  confidence: number;
  /** Providers that agreed, highest-weight first — used for the candidate `reason`. */
  agreeingProviders: string[];
}

/** Pre-computed lookup over the tracked series for matching. */
export interface MatchContext {
  /** `provider:id` -> codex series ids carrying it (usually one). */
  byKey: Map<string, string[]>;
  /** codex series id -> its `provider -> id` map (for the conflict tally). */
  series: Map<string, Map<string, string>>;
}

/** Compose the lookup key for a `(provider, externalId)` pair. */
function indexKey(provider: string, externalId: string): string {
  return `${provider}:${externalId}`;
}

/**
 * Build the match context from the host's tracked-series rows. Entries without
 * external IDs contribute nothing.
 */
export function buildMatchContext(entries: TrackedSeriesEntry[]): MatchContext {
  const byKey = new Map<string, string[]>();
  const series = new Map<string, Map<string, string>>();

  for (const entry of entries) {
    const ids = entry.externalIds;
    if (!ids) continue;
    const map = new Map<string, string>();
    for (const [provider, externalId] of Object.entries(ids)) {
      if (!externalId) continue;
      map.set(provider, externalId);
      const key = indexKey(provider, externalId);
      const arr = byKey.get(key);
      if (arr) {
        arr.push(entry.seriesId);
      } else {
        byKey.set(key, [entry.seriesId]);
      }
    }
    if (map.size > 0) {
      series.set(entry.seriesId, map);
    }
  }

  return { byKey, series };
}

/**
 * The full set of `provider:id` keys across all tracked series. This is the
 * filter set posted to Tsundoku's `POST /series/feed` so the feed is narrowed
 * to the consumer's catalog.
 */
export function externalIdFilter(ctx: MatchContext): string[] {
  return [...ctx.byKey.keys()];
}

/** Map a net score to a host confidence in `[0.8, 1.0]` (gate is 0.7). */
function confidenceForScore(score: number): number {
  return Math.min(1, Math.max(0.7, 0.7 + 0.1 * score));
}

/**
 * Resolve a feed item to the single best-matching tracked series, or `null`
 * when nothing matches net-positive or the top two candidates tie (ambiguous —
 * the item's IDs point at two series equally well, so we can't safely pick).
 */
export function matchItem(item: FeedItem, ctx: MatchContext): MatchResult | null {
  const itemMap = new Map<string, string>();
  for (const ext of item.externalIds) {
    if (ext.externalId) {
      itemMap.set(ext.provider, ext.externalId);
    }
  }

  // Candidate Codex series: any that shares at least one id with the item.
  const candidates = new Set<string>();
  for (const [provider, id] of itemMap) {
    const arr = ctx.byKey.get(indexKey(provider, id));
    if (arr) {
      for (const sid of arr) candidates.add(sid);
    }
  }
  if (candidates.size === 0) return null;

  let best: MatchResult | null = null;
  let tiedAtBest = false;

  for (const cid of candidates) {
    const cSeries = ctx.series.get(cid);
    if (!cSeries) continue;

    let agree = 0;
    let disagree = 0;
    const agreeing: Array<{ provider: string; weight: number }> = [];
    for (const [provider, idVal] of itemMap) {
      const cVal = cSeries.get(provider);
      if (cVal === undefined) continue; // provider not shared by both
      const w = weightOf(provider);
      if (cVal === idVal) {
        agree += w;
        agreeing.push({ provider, weight: w });
      } else {
        disagree += w;
      }
    }

    const score = agree - disagree;
    if (score <= 0) continue; // disagreement outweighs (or ties) agreement

    if (!best || score > best.score) {
      agreeing.sort((a, b) => b.weight - a.weight || a.provider.localeCompare(b.provider));
      best = {
        codexSeriesId: cid,
        score,
        confidence: confidenceForScore(score),
        agreeingProviders: agreeing.map((a) => a.provider),
      };
      tiedAtBest = false;
    } else if (score === best.score) {
      tiedAtBest = true;
    }
  }

  // No net-positive candidate, or two series matched equally well → don't guess.
  if (!best || tiedAtBest) return null;
  return best;
}
