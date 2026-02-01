/**
 * Re-export all types
 */

// From capabilities - interface contracts
export type {
  // Primary types
  MetadataContentType,
  MetadataProvider,
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
  AlternateTitle,
  ExternalLink,
  ExternalLinkType,
  ExternalRating,
  MetadataGetParams,
  MetadataMatchParams,
  MetadataMatchResponse,
  MetadataSearchParams,
  MetadataSearchResponse,
  PluginSeriesMetadata,
  ReadingDirection,
  SearchResult,
  SearchResultPreview,
  SeriesStatus,
} from "./protocol.js";
export * from "./rpc.js";
