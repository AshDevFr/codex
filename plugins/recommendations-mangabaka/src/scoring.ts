/**
 * Score adjustments applied after filtering.
 *
 * Currently just the same-author rule. The blend between content similarity and
 * collaborative filtering will land here too.
 */

import type { Candidate } from "./filters.js";
import { logger } from "./logger.js";

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
export function applyAuthorAdjustment(
  candidates: Candidate[],
  options: AuthorAdjustmentOptions = {},
): Candidate[] {
  const penalty = options.penalty ?? SAME_AUTHOR_PENALTY;
  const adjusted: Candidate[] = [];
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
