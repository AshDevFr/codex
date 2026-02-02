/**
 * Echo Plugin - A minimal plugin for testing the Codex plugin protocol
 *
 * This plugin demonstrates the plugin SDK usage and serves as a protocol
 * validation tool. It echoes back search parameters and provides predictable
 * responses for testing.
 */

import {
  createMetadataPlugin,
  type InitializeParams,
  type MetadataContentType,
  type MetadataGetParams,
  type MetadataMatchParams,
  type MetadataMatchResponse,
  type MetadataProvider,
  type MetadataSearchParams,
  type MetadataSearchResponse,
  type PluginManifest,
  type PluginSeriesMetadata,
} from "@ashdev/codex-plugin-sdk";
import packageJson from "../package.json" with { type: "json" };

// Default config values
const DEFAULT_MAX_RESULTS = 5;

// Plugin configuration (set during initialization)
const config = {
  maxResults: DEFAULT_MAX_RESULTS,
};

const manifest = {
  name: "metadata-echo",
  displayName: "Echo Metadata Plugin",
  version: packageJson.version,
  description: "Test metadata plugin that echoes back search queries",
  author: "Codex",
  homepage: "https://github.com/AshDevFr/codex",
  protocolVersion: "1.0",
  capabilities: {
    metadataProvider: ["series"] as MetadataContentType[],
  },
  configSchema: {
    description: "Configuration options for the Echo test plugin",
    fields: [
      {
        key: "maxResults",
        label: "Maximum Results",
        description: "Maximum number of results to return for search queries (1-20)",
        type: "number" as const,
        required: false,
        default: DEFAULT_MAX_RESULTS,
        example: 10,
      },
    ],
  },
} as const satisfies PluginManifest & {
  capabilities: { metadataProvider: MetadataContentType[] };
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
      totalBookCount: 10,
      language: "en",
      ageRating: 13,
      readingDirection: "ltr",

      // Taxonomy
      genres: ["Action", "Comedy", "Test", "Echo"],
      tags: ["plugin-test", "echo", "automation", "development"],

      // Credits
      authors: ["Echo Author", "Test Writer"],
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

createMetadataPlugin({
  manifest,
  provider,
  logLevel: "debug",
  onInitialize(params: InitializeParams) {
    // Read config from initialization params
    const maxResults = params.config?.maxResults as number | undefined;
    if (maxResults !== undefined) {
      config.maxResults = Math.min(Math.max(1, maxResults), 20); // Clamp 1-20
    }
    console.log(`Echo plugin initialized (maxResults: ${config.maxResults})`);
  },
});
