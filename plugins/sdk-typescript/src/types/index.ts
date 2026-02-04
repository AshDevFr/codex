/**
 * Re-export all types
 */

// From capabilities - interface contracts
export type {
  // Primary types
  BookMetadataProvider,
  MetadataContentType,
  MetadataProvider,
  PartialBookMetadataProvider,
  PartialMetadataProvider,
  PartialSeriesMetadataProvider,
  RecommendationProvider,
  // Deprecated aliases
  SeriesMetadataProvider,
  SyncProvider,
} from "./capabilities.js";

// From manifest - plugin configuration types
export type {
  ConfigField,
  ConfigSchema,
  CredentialField,
  PluginCapabilities,
  PluginManifest,
} from "./manifest.js";
export { hasBookMetadataProvider, hasSeriesMetadataProvider } from "./manifest.js";

// From protocol - JSON-RPC protocol types (these match Rust exactly)
export type {
  // Common types
  AlternateTitle,
  // Book metadata types
  BookAuthor,
  BookAuthorRole,
  BookAward,
  BookCover,
  BookCoverSize,
  BookMatchParams,
  BookSearchParams,
  ExternalLink,
  ExternalLinkType,
  ExternalRating,
  // Series metadata types
  MetadataGetParams,
  MetadataMatchParams,
  MetadataMatchResponse,
  MetadataSearchParams,
  MetadataSearchResponse,
  PluginBookMetadata,
  PluginSeriesMetadata,
  ReadingDirection,
  SearchResult,
  SearchResultPreview,
  SeriesStatus,
} from "./protocol.js";
export * from "./rpc.js";
