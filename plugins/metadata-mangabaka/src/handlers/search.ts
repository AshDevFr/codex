import {
  createLogger,
  type MetadataSearchParams,
  type MetadataSearchResponse,
} from "@ashdev/codex-plugin-sdk";
import type { MangaBakaClient } from "../api.js";
import { mapSearchResult } from "../mappers.js";

const logger = createLogger({ name: "mangabaka-search", level: "debug" });

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

  // Map results - API already returns them sorted by relevance
  const results = response.data.map(mapSearchResult);

  // Calculate next cursor (next page number) if there are more results
  const hasNextPage = response.page < response.totalPages;
  const nextCursor = hasNextPage ? String(response.page + 1) : undefined;

  return {
    results,
    nextCursor,
  };
}
