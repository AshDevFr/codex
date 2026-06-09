/**
 * Open Library Metadata Plugin for Codex
 *
 * Fetches book metadata from Open Library (openlibrary.org), a free and open
 * book database with extensive ISBN coverage.
 *
 * Features:
 * - ISBN lookup for direct, accurate matching
 * - Title/author search for fuzzy matching
 * - Cover image fetching in multiple sizes
 * - Author resolution with proper names
 * - Subject/genre extraction
 *
 * @see https://openlibrary.org/developers/api
 */

import {
  type BookMatchParams,
  type BookMetadataProvider,
  type BookSearchParams,
  createMetadataPlugin,
  type InitializeParams,
  type MetadataGetParams,
  type MetadataMatchResponse,
  type MetadataSearchResponse,
  type PluginBookMetadata,
} from "@ashdev/codex-plugin-sdk";

import { getEditionByIsbn, getWork, isValidIsbn, searchBooks } from "./api.js";
import { logger } from "./logger.js";
import { DEFAULT_MAX_RESULTS, manifest } from "./manifest.js";
import {
  getFullBookMetadata,
  mapEditionToBookMetadata,
  mapSearchDocToSearchResult,
} from "./mapper.js";

// Plugin configuration (set during initialization)
const config = {
  maxResults: DEFAULT_MAX_RESULTS,
};

/**
 * Book metadata provider implementation
 */
const bookProvider: BookMetadataProvider = {
  /**
   * Search for books by ISBN or title/author query
   *
   * If ISBN is provided, it takes priority for direct lookup.
   * Otherwise, falls back to title/author search.
   */
  async search(params: BookSearchParams): Promise<MetadataSearchResponse> {
    const { isbn, query, author, limit } = params;
    const maxResults = Math.min(limit || config.maxResults, 50);
    logger.debug(
      `search: isbn=${isbn ?? "-"} query=${JSON.stringify(query ?? "")} author=${JSON.stringify(author ?? "")} maxResults=${maxResults}`,
    );

    // If ISBN is provided, try direct lookup first
    if (isbn && isValidIsbn(isbn)) {
      const edition = await getEditionByIsbn(isbn);

      if (edition) {
        // Found by ISBN - return as single result with high relevance
        const workKey = edition.works?.[0]?.key;
        const workData = workKey ? await getWork(workKey) : null;
        const metadata = await mapEditionToBookMetadata(edition, workData);

        return {
          results: [
            {
              externalId: metadata.externalId,
              title: metadata.title || "Unknown",
              alternateTitles: metadata.subtitle ? [metadata.subtitle] : [],
              year: metadata.year,
              coverUrl: metadata.coverUrl,
              relevanceScore: 1.0, // Perfect match by ISBN
              preview: {
                genres: metadata.subjects.slice(0, 5),
                authors: metadata.authors.map((a) => a.name),
              },
            },
          ],
        };
      }

      // ISBN not found, fall through to search if query is also provided
      if (!query) {
        return { results: [] };
      }
    }

    // Title/author search
    if (!query) {
      return { results: [] };
    }

    const searchResponse = await searchBooks(query, {
      author,
      limit: maxResults,
    });

    if (!searchResponse?.docs?.length) {
      logger.debug(`search: no results for query=${JSON.stringify(query)}`);
      return { results: [] };
    }

    logger.debug(
      `search: ${searchResponse.docs.length} result(s) for query=${JSON.stringify(query)}`,
    );
    return {
      results: searchResponse.docs.map(mapSearchDocToSearchResult),
    };
  },

  /**
   * Get full book metadata by external ID
   *
   * The external ID can be:
   * - A work key: "/works/OL45883W"
   * - An edition key: "/books/OL7353617M"
   */
  async get(params: MetadataGetParams): Promise<PluginBookMetadata> {
    const { externalId } = params;
    logger.debug(`get: externalId=${externalId}`);

    // Try to get full metadata
    const metadata = await getFullBookMetadata(externalId);

    if (metadata) {
      return metadata;
    }

    logger.debug(`get: no full metadata for ${externalId}, returning minimal record`);

    // Fallback: return minimal metadata
    return {
      externalId,
      externalUrl: `https://openlibrary.org${externalId.startsWith("/") ? externalId : `/${externalId}`}`,
      alternateTitles: [],
      isbns: [],
      genres: [],
      tags: [],
      subjects: [],
      authors: [],
      artists: [],
      covers: [],
      externalRatings: [],
      awards: [],
      externalLinks: [
        {
          url: `https://openlibrary.org${externalId.startsWith("/") ? externalId : `/${externalId}`}`,
          label: "Open Library",
          linkType: "provider",
        },
      ],
      externalIds: [{ source: "api:openlibrary", externalId }],
    };
  },

  /**
   * Auto-match a book using available identifiers
   *
   * Match priority:
   * 1. ISBN (if provided) - highest confidence
   * 2. Title + author search - lower confidence
   */
  async match(params: BookMatchParams): Promise<MetadataMatchResponse> {
    const { title, authors, isbn, year } = params;
    logger.debug(
      `match: title=${JSON.stringify(title)} authors=${JSON.stringify(authors ?? [])} isbn=${isbn ?? "-"} year=${year ?? "-"}`,
    );

    // Try ISBN first if available
    if (isbn && isValidIsbn(isbn)) {
      const edition = await getEditionByIsbn(isbn);

      if (edition) {
        logger.debug(`match: ISBN ${isbn} resolved directly (confidence 0.99)`);
        const workKey = edition.works?.[0]?.key;
        const workData = workKey ? await getWork(workKey) : null;
        const metadata = await mapEditionToBookMetadata(edition, workData);

        return {
          match: {
            externalId: metadata.externalId,
            title: metadata.title || "Unknown",
            alternateTitles: metadata.subtitle ? [metadata.subtitle] : [],
            year: metadata.year,
            coverUrl: metadata.coverUrl,
            relevanceScore: 1.0,
            preview: {
              genres: metadata.subjects.slice(0, 5),
              authors: metadata.authors.map((a) => a.name),
            },
          },
          confidence: 0.99, // Very high confidence for ISBN match
          alternatives: [],
        };
      }
    }

    // Fall back to title search
    const searchQuery = authors?.length ? `${title} ${authors[0]}` : title;

    const searchResponse = await searchBooks(searchQuery, {
      limit: 5,
    });

    if (!searchResponse?.docs?.length) {
      return {
        match: null,
        confidence: 0,
        alternatives: [],
      };
    }

    const results = searchResponse.docs.map(mapSearchDocToSearchResult);

    // Calculate confidence based on title similarity and other factors
    const bestMatch = results[0];
    let confidence = bestMatch.relevanceScore || 0.5;

    // Boost confidence if title matches closely
    const normalizedTitle = title.toLowerCase().trim();
    const normalizedMatchTitle = bestMatch.title.toLowerCase().trim();

    if (normalizedTitle === normalizedMatchTitle) {
      confidence = Math.min(1.0, confidence + 0.3);
    } else if (
      normalizedMatchTitle.includes(normalizedTitle) ||
      normalizedTitle.includes(normalizedMatchTitle)
    ) {
      confidence = Math.min(1.0, confidence + 0.15);
    }

    // Boost if year matches
    if (year && bestMatch.year === year) {
      confidence = Math.min(1.0, confidence + 0.1);
    }

    // Reduce confidence without ISBN
    confidence = Math.min(confidence, 0.85);

    logger.debug(
      `match: best=${JSON.stringify(bestMatch.title)} confidence=${confidence.toFixed(2)} (${results.length} candidate(s))`,
    );
    return {
      match: bestMatch,
      confidence,
      alternatives: results.slice(1),
    };
  },
};

// =============================================================================
// Plugin Initialization
// =============================================================================

createMetadataPlugin({
  manifest,
  bookProvider,
  logLevel: "info",
  onInitialize(params: InitializeParams) {
    // Honor the host-supplied log level (Codex `plugins.log_level` config).
    if (params.logLevel) logger.setLevel(params.logLevel);
    // Read config from initialization params
    const maxResults = params.adminConfig?.maxResults as number | undefined;
    if (maxResults !== undefined) {
      config.maxResults = Math.min(Math.max(1, maxResults), 50); // Clamp 1-50
    }
    logger.info(`Plugin initialized (maxResults: ${config.maxResults})`);
  },
});

logger.info("Open Library plugin started");
