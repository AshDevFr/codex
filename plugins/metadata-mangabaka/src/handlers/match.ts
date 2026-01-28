import {
  createLogger,
  type MetadataMatchParams,
  type MetadataMatchResponse,
  type SearchResult,
} from "@codex/plugin-sdk";
import type { MangaBakaClient } from "../api.js";
import { mapSearchResult } from "../mappers.js";

const logger = createLogger({ name: "mangabaka-match", level: "info" });

/**
 * Calculate string similarity using word overlap
 * Returns a value between 0 and 1
 */
function similarity(a: string, b: string): number {
  const aLower = a.toLowerCase().trim();
  const bLower = b.toLowerCase().trim();

  if (aLower === bLower) return 1.0;
  if (aLower.length === 0 || bLower.length === 0) return 0;

  // Check if one contains the other
  if (aLower.includes(bLower) || bLower.includes(aLower)) {
    return 0.8;
  }

  // Simple word overlap scoring
  const aWords = new Set(aLower.split(/\s+/));
  const bWords = new Set(bLower.split(/\s+/));
  const intersection = [...aWords].filter((w) => bWords.has(w));
  const union = new Set([...aWords, ...bWords]);

  return intersection.length / union.size;
}

/**
 * Score a search result against the match parameters
 * Returns a value between 0 and 1
 */
function scoreResult(result: SearchResult, params: MetadataMatchParams): number {
  let score = 0;

  // Title similarity (up to 0.6)
  const titleScore = similarity(result.title, params.title);
  score += titleScore * 0.6;

  // Year match (up to 0.2)
  if (params.year && result.year) {
    if (result.year === params.year) {
      score += 0.2;
    } else if (Math.abs(result.year - params.year) <= 1) {
      score += 0.1;
    }
  }

  // Boost for exact title match (up to 0.2)
  if (result.title.toLowerCase() === params.title.toLowerCase()) {
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
