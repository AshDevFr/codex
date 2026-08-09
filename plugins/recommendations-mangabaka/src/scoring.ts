/**
 * Score adjustments and the blend between the two recommendation signals.
 */

import type { Candidate } from "./filters.js";
import { logger } from "./logger.js";
import { buildReason, summariseTitles } from "./mappers.js";
import type { MbSharedTag } from "./types.js";

/**
 * Multiplier applied to a recommendation that shares an author or artist with a
 * seed.
 *
 * Chosen so that a strong same-author match still outranks a weak unrelated
 * one. Same-author results are deliberately not dropped: unlike a spin-off, an
 * author's *unrelated* other work is often exactly what someone who liked their
 * writing wants next. What it should not do is monopolise the list, which is
 * what an unpenalised score lets it do, since prose style and recurring themes
 * make an author's own catalogue score highly on tag similarity.
 */
export const SAME_AUTHOR_PENALTY = 0.6;

export interface AuthorAdjustmentOptions {
  /** Remove same-author results entirely instead of de-ranking them. */
  excludeSameAuthor?: boolean;
  /** Override the de-ranking multiplier. */
  penalty?: number;
}

/** Round and clamp to the two-decimal 0-1 range used throughout. */
function normalizeScore(score: number): number {
  const bounded = Math.max(0, Math.min(score, 1));
  return Math.round(bounded * 100) / 100;
}

/**
 * De-rank (or optionally drop) recommendations sharing an author with a seed.
 *
 * Returns new candidate objects; the inputs are left untouched so the raw
 * upstream score stays available for debugging.
 */
export function applyAuthorAdjustment<T extends Candidate>(
  candidates: T[],
  options: AuthorAdjustmentOptions = {},
): T[] {
  const penalty = options.penalty ?? SAME_AUTHOR_PENALTY;
  const adjusted: T[] = [];
  let penalised = 0;
  let excluded = 0;

  for (const candidate of candidates) {
    if (candidate.entry.matched_author !== true) {
      adjusted.push(candidate);
      continue;
    }

    if (options.excludeSameAuthor) {
      excluded++;
      continue;
    }

    penalised++;
    adjusted.push({
      ...candidate,
      recommendation: {
        ...candidate.recommendation,
        score: normalizeScore(candidate.recommendation.score * penalty),
      },
    });
  }

  if (penalised > 0) logger.debug(`De-ranked ${penalised} same-author candidates`);
  if (excluded > 0) logger.debug(`Excluded ${excluded} same-author candidates`);

  return adjusted;
}

// =============================================================================
// Signal blending
// =============================================================================

/**
 * How much of a blended score comes from content similarity.
 *
 * Neutral by default. The two signals answer different questions, "this is like
 * what you read" versus "people who read that read this", and there is no
 * evidence yet that either deserves precedence. A content-leaning default was
 * tried first and left the collaborative signal contributing roughly one result
 * in ten, which is not a blend.
 *
 * Note this weight ranks; it does not switch a signal off. At 1 a series is
 * scored purely on content, but one that *also* appears in the collaborative
 * lists still gets the agreement headroom described on
 * {@link SINGLE_SIGNAL_CEILING}. To drop the collaborative signal entirely, set
 * `collaborativeSeeds` to 0, which skips the requests altogether.
 */
export const DEFAULT_CONTENT_WEIGHT = 0.5;

/**
 * Ceiling for a series only one signal returned.
 *
 * Content similarity and reader overlap are derived independently, one from tag
 * vectors and one from library co-occurrence, so agreement between them is the
 * strongest evidence available and should outrank either alone. Reserving the
 * top of the range for agreement is how that is expressed.
 *
 * The obvious alternative, multiplying agreeing entries by a bonus, does not
 * work here: per-response normalisation puts the best entry of each signal at
 * exactly 1.0, so a boosted score clamps straight back to 1.0 and ties with the
 * top single-signal entry. The bonus was unreachable in every case that
 * mattered. Capping single-signal entries instead leaves real headroom.
 */
export const SINGLE_SIGNAL_CEILING = 0.85;

export interface BlendInput {
  /** Normalised content-similarity score, if the content probe returned it. */
  content?: number;
  /** Normalised collaborative score, if any seed's readers endorsed it. */
  collaborative?: number;
}

export interface BlendOptions {
  /** Share of the blend attributed to content similarity, 0-1. */
  contentWeight?: number;
}

function clampUnit(value: number): number {
  return Math.max(0, Math.min(value, 1));
}

function round2(value: number): number {
  return Math.round(value * 100) / 100;
}

/**
 * Rescale a set of scores so the strongest becomes 1.
 *
 * Both signals need this before they can be blended, and for the same reason
 * even though their raw ranges differ: each is only meaningful relative to the
 * other entries in its own response.
 *
 * Skipping it on the content side is not a harmless simplification. Live mix
 * scores cluster around 0.34-0.36 while normalised collaborative scores reach
 * 1.0, so leaving content raw let the collaborative signal take every slot in a
 * ten-result set and silently reduced the blend to one signal.
 */
export function normalizeByMax(scores: Map<number, number>): Map<number, number> {
  const max = Math.max(0, ...scores.values());
  if (max <= 0) return new Map();

  const normalized = new Map<number, number>();
  for (const [key, value] of scores) {
    if (value > 0) normalized.set(key, value / max);
  }
  return normalized;
}

/**
 * Combine the content and collaborative scores into one ranking value.
 *
 * A series found by only one signal keeps that signal's score, scaled into the
 * range reserved for single-signal results. The absent term is deliberately not
 * treated as a zero: it does not mean "readers disliked this", it means the
 * other signal never looked at it, and averaging in a zero for that would bury
 * genuine recommendations.
 */
export function blendScores(input: BlendInput, options: BlendOptions = {}): number {
  const hasContent = typeof input.content === "number";
  const hasCollaborative = typeof input.collaborative === "number";

  if (!hasContent && !hasCollaborative) return 0;

  const weight = clampUnit(options.contentWeight ?? DEFAULT_CONTENT_WEIGHT);

  if (hasContent && hasCollaborative) {
    const blended =
      weight * clampUnit(input.content as number) +
      (1 - weight) * clampUnit(input.collaborative as number);
    return round2(clampUnit(blended));
  }

  // Single-signal entries are scaled by their signal's share of the weight,
  // relative to whichever signal the weight favours. Without this the weight
  // would only ever move entries both signals found, which are a small minority
  // of any result set, leaving the setting effectively inert. Dividing by the
  // dominant share keeps the favoured signal at the full ceiling rather than
  // dragging every score down.
  const dominant = Math.max(weight, 1 - weight);
  if (dominant === 0) return 0;

  const share = hasContent ? weight : 1 - weight;
  const signal = hasContent ? (input.content as number) : (input.collaborative as number);

  return round2(clampUnit(signal) * SINGLE_SIGNAL_CEILING * (share / dominant));
}

export interface BlendedReasonInput {
  /** Tags this series shares with the seeds, from the content probe. */
  sharedTags: MbSharedTag[] | null | undefined;
  /** Seed titles that contributed via content similarity. */
  contentSeedTitles: string[];
  /** Seed titles whose readers also read this. */
  collaborativeSeedTitles: string[];
}

/**
 * Compose a justification that says which signal produced the recommendation.
 *
 * The protocol offers only a free-text `reason`, so provenance has to be
 * expressed in prose. It matters here: "shares these tags" and "other readers
 * liked it" are different claims, and a reader deciding what to start next
 * weighs them differently.
 */
export function buildBlendedReason(input: BlendedReasonInput): string {
  const collaborative = input.collaborativeSeedTitles;
  const hasCollaborative = collaborative.length > 0;
  const hasContent = (input.sharedTags?.length ?? 0) > 0 || input.contentSeedTitles.length > 0;

  if (hasCollaborative && !hasContent) {
    return `Readers of ${summariseTitles(collaborative)} also read this`;
  }

  const contentReason = buildReason(input.sharedTags, input.contentSeedTitles);
  if (!hasCollaborative) return contentReason;

  return `${contentReason}, and readers of ${summariseTitles(collaborative)} also read it`;
}
