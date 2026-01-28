/**
 * Echo Plugin - A minimal plugin for testing the Codex plugin protocol
 *
 * This plugin demonstrates the plugin SDK usage and serves as a protocol
 * validation tool. It echoes back search parameters and provides predictable
 * responses for testing.
 */

import {
  createMetadataPlugin,
  type MetadataContentType,
  type MetadataGetParams,
  type MetadataMatchParams,
  type MetadataMatchResponse,
  type MetadataProvider,
  type MetadataSearchParams,
  type MetadataSearchResponse,
  type PluginManifest,
  type PluginSeriesMetadata,
} from "@codex/plugin-sdk";

const manifest = {
  name: "metadata-echo",
  displayName: "Echo Metadata Plugin",
  version: "1.0.0",
  description: "Test metadata plugin that echoes back search queries",
  author: "Codex",
  homepage: "https://github.com/AshDevFr/codex",
  protocolVersion: "1.0",
  capabilities: {
    metadataProvider: ["series"] as MetadataContentType[],
  },
} as const satisfies PluginManifest & {
  capabilities: { metadataProvider: MetadataContentType[] };
};

const provider: MetadataProvider = {
  async search(params: MetadataSearchParams): Promise<MetadataSearchResponse> {
    // Echo back the query as search results
    return {
      results: [
        {
          externalId: "echo-1",
          title: `Echo: ${params.query}`,
          alternateTitles: [`Echoed Query: ${params.query}`],
          year: new Date().getFullYear(),
          relevanceScore: 1.0, // Perfect match for echo
          preview: {
            status: "ended",
            genres: ["Test", "Echo"],
            rating: 10.0,
            description: `Search query echoed: "${params.query}"`,
          },
        },
        {
          externalId: "echo-2",
          title: `Echo Result 2 for: ${params.query}`,
          alternateTitles: [],
          relevanceScore: 0.8,
          preview: {
            status: "ongoing",
            genres: ["Test"],
            description: "A second result for testing pagination",
          },
        },
      ],
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
});
