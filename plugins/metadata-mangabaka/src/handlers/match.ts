import {
  createLogger,
  type MetadataMatchParams,
  type MetadataMatchResponse,
  type SearchResult,
} from "@ashdev/codex-plugin-sdk";
import type { MangaBakaClient } from "../api.js";
import { mapSearchResult } from "../mappers.js";

const logger = createLogger({ name: "mangabaka-match", level: "info" });

/**
 * Calculate string similarity using word overlap and containment scoring
 * Returns a value between 0 and 1
 */
export function similarity(a: string, b: string): number {
  const aLower = a.toLowerCase().trim();
  const bLower = b.toLowerCase().trim();

  if (aLower === bLower) return 1.0;
  if (aLower.length === 0 || bLower.length === 0) return 0;

  let score = 0;

  // Containment check with length-ratio penalty
  // Prevents short queries like "Air" from matching "Air Gear" too strongly
  const shorter = aLower.length <= bLower.length ? aLower : bLower;
  const longer = aLower.length <= bLower.length ? bLower : aLower;

  if (longer.includes(shorter)) {
    const lengthRatio = shorter.length / longer.length;
    score = Math.max(score, 0.8 * lengthRatio);
  }

  // Word overlap scoring (Jaccard similarity)
  const aWords = new Set(aLower.split(/\s+/));
  const bWords = new Set(bLower.split(/\s+/));
  const intersection = [...aWords].filter((w) => bWords.has(w));
  const union = new Set([...aWords, ...bWords]);

  if (union.size > 0) {
    score = Math.max(score, intersection.length / union.size);
  }

  return score;
}

/**
 * Score a search result against the match parameters
 * Returns a value between 0 and 1
 */
export function scoreResult(result: SearchResult, params: MetadataMatchParams): number {
  let score = 0;

  // Find best title similarity across primary and alternate titles
  let bestTitleSimilarity = similarity(result.title, params.title);
  for (const alt of result.alternateTitles) {
    bestTitleSimilarity = Math.max(bestTitleSimilarity, similarity(alt, params.title));
  }

  // Title similarity (up to 0.6)
  score += bestTitleSimilarity * 0.6;

  // Year match (up to 0.2)
  if (params.year && result.year) {
    if (result.year === params.year) {
      score += 0.2;
    } else if (Math.abs(result.year - params.year) <= 1) {
      score += 0.1;
    }
  }

  // Boost for exact title match across primary and alternate titles (up to 0.2)
  const searchLower = params.title.toLowerCase();
  const hasExactMatch =
    result.title.toLowerCase() === searchLower ||
    result.alternateTitles.some((alt) => alt.toLowerCase() === searchLower);

  if (hasExactMatch) {
    score += 0.2;
  }

  return Math.min(1.0, score);
}

export async function handleMatch(
  params: MetadataMatchParams,
  client: MangaBakaClient,
): Promise<MetadataMatchResponse> {
  logger.debug(`Matching: "${params.title}"`);

  // Search for the title
  const response = await client.search(params.title, 1, 10);

  if (response.data.length === 0) {
    return {
      match: null,
      confidence: 0,
    };
  }

  // Map and score results
  const scoredResults = response.data
    .map((series) => {
      const result = mapSearchResult(series);
      const score = scoreResult(result, params);
      return { result, score };
    })
    .sort((a, b) => b.score - a.score);

  const best = scoredResults[0];

  if (!best) {
    return {
      match: null,
      confidence: 0,
    };
  }

  // If confidence is low, include alternatives
  const alternatives =
    best.score < 0.8
      ? scoredResults.slice(1, 4).map((s) => ({
          ...s.result,
          relevanceScore: s.score,
        }))
      : undefined;

  return {
    match: {
      ...best.result,
      relevanceScore: best.score,
    },
    confidence: best.score,
    alternatives,
  };
}
