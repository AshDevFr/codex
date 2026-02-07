/**
 * Re-export all types
 */

// From sync - sync provider protocol types (these match Rust exactly)
export type {
  ExternalUserInfo,
  SyncEntry,
  SyncEntryResult,
  SyncEntryResultStatus,
  SyncProgress,
  SyncProvider,
  SyncPullRequest,
  SyncPullResponse,
  SyncPushRequest,
  SyncPushResponse,
  SyncReadingStatus,
  SyncStatusResponse,
} from "../sync.js";

// From capabilities - interface contracts
export type {
  BookMetadataProvider,
  MetadataContentType,
  MetadataProvider,
  PartialBookMetadataProvider,
  PartialMetadataProvider,
  // Deprecated aliases
  PartialSeriesMetadataProvider,
  RecommendationProvider,
  SeriesMetadataProvider,
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
