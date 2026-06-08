/**
 * Map a matched Tsundoku feed item to a Codex `ReleaseCandidate`.
 *
 * The feed already carries merged, gap-preserving coverage spans that line up
 * with Codex's `NumericSpan` model, so the volume/chapter axes pass through
 * verbatim. The candidate's `externalReleaseId` is keyed on the coverage
 * high-water mark, so a new ledger row (and announcement) fires only when the
 * frontier advances — re-delivery of the same coverage dedups host-side, and
 * the host's auto-ignore + `latest_known_*` gate handle "already owned".
 */

import type { ReleaseCandidate } from "@ashdev/codex-plugin-sdk";
import type { FeedCoverageSpan, FeedItem } from "./fetcher.js";
import type { MatchResult } from "./matcher.js";

// `FeedCoverageSpan` is structurally identical to the SDK's `NumericSpan`
// (`{ start, end }`), so a span list assigns directly to a candidate's
// `volumes` / `chapters` without a separate type or an SDK barrel export.

/** Inputs the candidate mapping needs beyond the feed item + match. */
export interface CandidateOptions {
  /** Tsundoku base URL (trailing slash tolerated) for building the landing link. */
  baseUrl: string;
  /** ISO 639-1 language stamped on the candidate (the feed carries none). */
  language: string;
  /** Detection timestamp (ISO-8601). Defaults to now; injectable for tests. */
  observedAt?: string;
}

/**
 * Convert a feed coverage list to a `NumericSpan[]`, or `null` when empty.
 * Coverage is already merged + sorted upstream, so this is a structural copy.
 */
export function toSpans(coverage: FeedCoverageSpan[]): FeedCoverageSpan[] | null {
  if (coverage.length === 0) return null;
  return coverage.map((s) => ({ start: s.start, end: s.end }));
}

/** Format a high-water value for the dedup key (`null` -> `-`). */
function fmtHighwater(value: number | null): string {
  return value === null ? "-" : String(value);
}

/**
 * Stable per-source dedup key. Keyed on the coverage high-water mark so the
 * same frontier re-delivers to the same `(sourceId, externalReleaseId)` ledger
 * row (a no-op dedup), while a genuine advance produces a new row.
 */
export function externalReleaseId(item: FeedItem): string {
  return `tsundoku:${item.seriesId}:v${fmtHighwater(item.highestVolume)}:c${fmtHighwater(item.highestChapter)}`;
}

/**
 * Build a `ReleaseCandidate` for a matched feed item. Confidence is 1.0 — the
 * match is an exact external-ID hit, never fuzzy.
 */
export function feedItemToCandidate(
  item: FeedItem,
  match: MatchResult,
  opts: CandidateOptions,
): ReleaseCandidate {
  const base = opts.baseUrl.replace(/\/+$/, "");
  return {
    seriesMatch: {
      codexSeriesId: match.codexSeriesId,
      confidence: 1.0,
      reason: `tsundoku:${match.provider}:${match.externalId}`,
    },
    externalReleaseId: externalReleaseId(item),
    volumes: toSpans(item.volumeCoverage),
    chapters: toSpans(item.chapterCoverage),
    language: opts.language,
    groupOrUploader: null,
    payloadUrl: `${base}/series/${item.seriesId}`,
    observedAt: opts.observedAt ?? new Date().toISOString(),
    // Tsundoku's `updatedAt` is epoch seconds; a coverage change is the closest
    // thing the feed has to a publish date. Not skew-checked host-side.
    releasedAt: new Date(item.updatedAt * 1000).toISOString(),
    metadata: {
      tsundokuSeriesId: item.seriesId,
      canonicalTitle: item.canonicalTitle,
      highestVolume: item.highestVolume,
      highestChapter: item.highestChapter,
    },
  };
}
