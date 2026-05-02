/**
 * Echo Plugin - A minimal plugin for testing the Codex plugin protocol
 *
 * This plugin demonstrates the plugin SDK usage and serves as a protocol
 * validation tool. It echoes back search parameters and provides predictable
 * responses for testing.
 *
 * Supports both series and book metadata to demonstrate multi-content-type plugins.
 */

import {
  type BookMatchParams,
  type BookMetadataProvider,
  type BookSearchParams,
  createLogger,
  createMetadataPlugin,
  type InitializeParams,
  type MetadataGetParams,
  type MetadataMatchParams,
  type MetadataMatchResponse,
  type MetadataProvider,
  type MetadataSearchParams,
  type MetadataSearchResponse,
  type PluginBookMetadata,
  type PluginSeriesMetadata,
} from "@ashdev/codex-plugin-sdk";
import { DEFAULT_MAX_RESULTS, manifest } from "./manifest.js";

const logger = createLogger({ name: "echo", level: "debug" });

// Plugin configuration (set during initialization)
const config = {
  maxResults: DEFAULT_MAX_RESULTS,
};

// Generate echo results based on config
function generateEchoResults(query: string, maxResults: number) {
  const results = [];
  const count = Math.min(Math.max(1, maxResults), 20); // Clamp between 1-20

  for (let i = 1; i <= count; i++) {
    results.push({
      externalId: `echo-${i}`,
      title: i === 1 ? `Echo: ${query}` : `Echo Result ${i} for: ${query}`,
      alternateTitles: i === 1 ? [`Echoed Query: ${query}`] : [],
      year: new Date().getFullYear(),
      relevanceScore: Math.max(0.1, 1.0 - (i - 1) * 0.1), // Decreasing relevance
      preview: {
        status: i % 2 === 0 ? "ongoing" : "ended",
        genres: i === 1 ? ["Test", "Echo"] : ["Test"],
        rating: i === 1 ? 10.0 : undefined,
        description:
          i === 1
            ? `Search query echoed: "${query}"`
            : `Result ${i} for testing (maxResults=${count})`,
      },
    });
  }

  return results;
}

const provider: MetadataProvider = {
  async search(params: MetadataSearchParams): Promise<MetadataSearchResponse> {
    // Echo back the query as search results, respecting maxResults config
    return {
      results: generateEchoResults(params.query, config.maxResults),
    };
  },

  async get(params: MetadataGetParams): Promise<PluginSeriesMetadata> {
    // Return metadata based on the external ID with all fields populated for testing
    return {
      externalId: params.externalId,
      externalUrl: `https://echo.example.com/series/${params.externalId}`,
      title: `Echo Series: ${params.externalId}`,
      alternateTitles: [
        { title: `Echo Series: ${params.externalId}`, language: "en", titleType: "english" },
        { title: `エコーシリーズ: ${params.externalId}`, language: "ja", titleType: "native" },
        { title: `Echo Romanized: ${params.externalId}`, language: "ja-Latn", titleType: "romaji" },
      ],
      summary: `This is the full metadata for external ID: ${params.externalId}. It includes a detailed description to test summary handling.`,
      status: "ended",
      year: 2024,

      // Extended metadata fields
      totalVolumeCount: 10,
      language: "en",
      ageRating: 13,
      readingDirection: "ltr",

      // Taxonomy
      genres: ["Action", "Comedy", "Test", "Echo"],
      tags: ["plugin-test", "echo", "automation", "development"],

      // Credits
      authors: [
        { name: "Echo Author", role: "author" },
        { name: "Test Writer", role: "author" },
      ],
      artists: ["Echo Artist"],
      publisher: "Echo Publisher",

      // Media
      coverUrl: "https://picsum.photos/300/450",
      bannerUrl: "https://picsum.photos/800/200",

      // Primary rating
      rating: {
        score: 85,
        voteCount: 100,
        source: "echo",
      },

      // Multiple external ratings for testing aggregation
      externalRatings: [
        {
          score: 85,
          voteCount: 100,
          source: "echo",
        },
        {
          score: 92,
          voteCount: 5000,
          source: "anilist",
        },
        {
          score: 88,
          voteCount: 2500,
          source: "mal",
        },
      ],

      // External links
      externalLinks: [
        {
          url: `https://echo.example.com/series/${params.externalId}`,
          label: "Echo Provider",
          linkType: "provider",
        },
        {
          url: "https://official-echo.example.com",
          label: "Official Site",
          linkType: "official",
        },
        {
          url: "https://twitter.com/echo_series",
          label: "Twitter",
          linkType: "social",
        },
        {
          url: "https://store.example.com/echo",
          label: "Buy",
          linkType: "purchase",
        },
      ],
    };
  },

  async match(params: MetadataMatchParams): Promise<MetadataMatchResponse> {
    // Return a match based on the title
    const normalizedTitle = params.title.toLowerCase().replace(/\s+/g, "-");
    return {
      match: {
        externalId: `match-${normalizedTitle}`,
        title: params.title,
        alternateTitles: [],
        year: params.year,
        relevanceScore: 0.9,
        preview: {
          status: "ended",
          genres: ["Matched"],
          description: `Matched from title: "${params.title}"`,
        },
      },
      confidence: 0.85,
      alternatives: [
        {
          externalId: "alt-1",
          title: `Alternative: ${params.title}`,
          alternateTitles: [],
          relevanceScore: 0.6,
        },
      ],
    };
  },
};

// =============================================================================
// Book Metadata Provider
// =============================================================================

// Generate echo results for book search
function generateBookEchoResults(params: BookSearchParams, maxResults: number) {
  const results = [];
  const count = Math.min(Math.max(1, maxResults), 20);
  const searchTerm = params.isbn || params.query || "unknown";

  for (let i = 1; i <= count; i++) {
    const isIsbnSearch = !!params.isbn;
    results.push({
      externalId: `echo-book-${i}`,
      title: i === 1 ? `Echo Book: ${searchTerm}` : `Echo Book Result ${i} for: ${searchTerm}`,
      alternateTitles: i === 1 ? [`Book Query: ${searchTerm}`] : [],
      year: params.year || new Date().getFullYear(),
      relevanceScore: isIsbnSearch ? 1.0 : Math.max(0.1, 1.0 - (i - 1) * 0.1),
      preview: {
        status: i % 2 === 0 ? "ongoing" : "ended",
        genres: i === 1 ? ["Test", "Echo", "Book"] : ["Test", "Book"],
        rating: i === 1 ? 9.0 : undefined,
        description:
          i === 1
            ? `Book search ${isIsbnSearch ? "by ISBN" : "by query"}: "${searchTerm}"`
            : `Book result ${i} for testing`,
      },
    });
  }

  return results;
}

const bookProvider: BookMetadataProvider = {
  async search(params: BookSearchParams): Promise<MetadataSearchResponse> {
    // Echo back the ISBN or query as search results
    return {
      results: generateBookEchoResults(params, config.maxResults),
    };
  },

  async get(params: MetadataGetParams): Promise<PluginBookMetadata> {
    // Return book metadata based on the external ID with all fields populated for testing
    return {
      externalId: params.externalId,
      externalUrl: `https://echo.example.com/book/${params.externalId}`,
      title: `Echo Book: ${params.externalId}`,
      subtitle: "A Test Subtitle",
      alternateTitles: [
        { title: `Echo Book: ${params.externalId}`, language: "en", titleType: "english" },
        { title: `エコーブック: ${params.externalId}`, language: "ja", titleType: "native" },
      ],
      summary: `This is the full book metadata for external ID: ${params.externalId}. It includes a detailed description to test book metadata handling.`,
      bookType: "novel",

      // Book-specific fields
      volume: 1,
      pageCount: 320,
      releaseDate: "2024-01-15",
      year: 2024,

      // ISBN
      isbn: "978-0-306-40615-7",
      isbns: ["978-0-306-40615-7", "0-306-40615-2"],

      // Edition/Translation
      edition: "First Edition",
      originalTitle: "Original Echo Title",
      originalYear: 2023,
      translator: "Echo Translator",
      language: "en",

      // Series position
      seriesPosition: 1.0,
      seriesTotal: 5,

      // Taxonomy
      genres: ["Fiction", "Test", "Echo"],
      tags: ["plugin-test", "echo", "book-test"],
      subjects: ["Testing", "Plugin Development"],

      // Authors with roles
      authors: [
        { name: "Echo Author", role: "author", sortName: "Author, Echo" },
        { name: "Test Writer", role: "co_author", sortName: "Writer, Test" },
        { name: "Echo Editor", role: "editor" },
      ],
      artists: ["Echo Illustrator"],
      publisher: "Echo Publishing House",

      // Media
      coverUrl: "https://picsum.photos/300/450",
      covers: [
        { url: "https://picsum.photos/300/450", width: 300, height: 450, size: "medium" },
        { url: "https://picsum.photos/600/900", width: 600, height: 900, size: "large" },
        { url: "https://picsum.photos/150/225", width: 150, height: 225, size: "small" },
      ],

      // Ratings
      rating: {
        score: 88,
        voteCount: 500,
        source: "echo",
      },
      externalRatings: [
        { score: 88, voteCount: 500, source: "echo" },
        { score: 92, voteCount: 10000, source: "goodreads" },
      ],

      // Awards
      awards: [
        { name: "Echo Award", year: 2024, category: "Best Test Book", won: true },
        { name: "Test Prize", year: 2024, category: "Excellence in Testing", won: false },
      ],

      // External links
      externalLinks: [
        {
          url: `https://echo.example.com/book/${params.externalId}`,
          label: "Echo Provider",
          linkType: "provider",
        },
        {
          url: "https://goodreads.com/book/echo",
          label: "Goodreads",
          linkType: "other",
        },
        {
          url: "https://amazon.com/echo-book",
          label: "Amazon",
          linkType: "purchase",
        },
      ],
    };
  },

  async match(params: BookMatchParams): Promise<MetadataMatchResponse> {
    // Return a match based on ISBN (preferred) or title
    const identifier = params.isbn || params.title;
    const normalizedId = identifier.toLowerCase().replace(/[\s-]/g, "");
    const isIsbnMatch = !!params.isbn;

    return {
      match: {
        externalId: `match-book-${normalizedId}`,
        title: params.title,
        alternateTitles: [],
        year: params.year,
        relevanceScore: isIsbnMatch ? 1.0 : 0.85,
        preview: {
          status: "ended",
          genres: ["Matched", "Book"],
          description: isIsbnMatch
            ? `Matched by ISBN: ${params.isbn}`
            : `Matched from title: "${params.title}"`,
        },
      },
      confidence: isIsbnMatch ? 0.99 : 0.8,
      alternatives: isIsbnMatch
        ? []
        : [
            {
              externalId: "alt-book-1",
              title: `Alternative: ${params.title}`,
              alternateTitles: [],
              relevanceScore: 0.5,
            },
          ],
    };
  },
};

// =============================================================================
// Plugin Initialization
// =============================================================================

createMetadataPlugin({
  manifest,
  provider, // Series provider
  bookProvider, // Book provider
  logLevel: "debug",
  onInitialize(params: InitializeParams) {
    // Read config from initialization params
    const maxResults = params.adminConfig?.maxResults as number | undefined;
    if (maxResults !== undefined) {
      config.maxResults = Math.min(Math.max(1, maxResults), 20); // Clamp 1-20
    }
    logger.info(`Echo plugin initialized (maxResults: ${config.maxResults})`);
  },
});

logger.info("Echo plugin started");
