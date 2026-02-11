import {
  createLogger,
  type MetadataSearchParams,
  type MetadataSearchResponse,
  type SearchResult,
} from "@ashdev/codex-plugin-sdk";
import type { MangaBakaClient } from "../api.js";
import { mapSearchResult } from "../mappers.js";
import { similarity } from "../similarity.js";

const logger = createLogger({ name: "mangabaka-search", level: "debug" });

/**
 * Score a search result against the query using title similarity.
 * Checks both primary title and alternate titles, returning the best score.
 */
export function scoreSearchResult(result: SearchResult, query: string): number {
  let best = similarity(result.title, query);
  for (const alt of result.alternateTitles) {
    best = Math.max(best, similarity(alt, query));
  }
  return best;
}

export async function handleSearch(
  params: MetadataSearchParams,
  client: MangaBakaClient,
): Promise<MetadataSearchResponse> {
  logger.debug("Search params received:", params);

  const limit = params.limit ?? 20;

  // Parse cursor as page number (default to 1)
  const page = params.cursor ? Number.parseInt(params.cursor, 10) : 1;

  logger.debug(`Searching for: "${params.query}" (page ${page}, limit ${limit})`);

  const response = await client.search(params.query, page, limit);

  // Map results and score by similarity to the search query
  const results = response.data
    .map((series) => {
      const result = mapSearchResult(series);
      result.relevanceScore = scoreSearchResult(result, params.query);
      return result;
    })
    .sort((a, b) => (b.relevanceScore ?? 0) - (a.relevanceScore ?? 0));

  // Calculate next cursor (next page number) if there are more results
  const hasNextPage = response.page < response.totalPages;
  const nextCursor = hasNextPage ? String(response.page + 1) : undefined;

  return {
    results,
    nextCursor,
  };
}
