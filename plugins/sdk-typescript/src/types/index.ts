/**
 * Re-export all types
 */

// From capabilities - all provider interfaces
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
  SyncProvider,
} from "./capabilities.js";

// From manifest - plugin configuration types
export type {
  ConfigField,
  ConfigSchema,
  CredentialField,
  OAuthConfig,
  PluginCapabilities,
  PluginManifest,
} from "./manifest.js";
export { hasBookMetadataProvider, hasSeriesMetadataProvider } from "./manifest.js";

// From protocol - metadata protocol types (these match Rust exactly)
export type {
  AlternateTitle,
  BookAuthor,
  BookAuthorRole,
  BookAward,
  BookCover,
  BookCoverSize,
  BookMatchParams,
  BookSearchParams,
  ExternalId,
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
// From recommendations - recommendation protocol types (these match Rust exactly)
export type {
  DismissReason,
  ProfileUpdateRequest,
  ProfileUpdateResponse,
  Recommendation,
  RecommendationClearResponse,
  RecommendationDismissRequest,
  RecommendationDismissResponse,
  RecommendationRequest,
  RecommendationResponse,
  UserLibraryEntry,
} from "./recommendations.js";
// From rpc - JSON-RPC primitives
export * from "./rpc.js";
// From sync - sync protocol types (these match Rust exactly)
export type {
  ExternalUserInfo,
  SyncEntry,
  SyncEntryResult,
  SyncEntryResultStatus,
  SyncProgress,
  SyncPullRequest,
  SyncPullResponse,
  SyncPushRequest,
  SyncPushResponse,
  SyncReadingStatus,
  SyncStatusResponse,
} from "./sync.js";
