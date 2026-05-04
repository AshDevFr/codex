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
  RecommendationProvider,
  ReleaseSourceProvider,
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
  ReleaseSourceCapability,
  ReleaseSourceKind,
} from "./manifest.js";
export {
  hasBookMetadataProvider,
  hasReleaseSource,
  hasSeriesMetadataProvider,
} from "./manifest.js";

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
// From releases - release-source protocol types (these match Rust exactly)
export type {
  ListTrackedRequest,
  ListTrackedResponse,
  RecordRequest,
  RecordResponse,
  ReleaseCandidate,
  ReleasePollRequest,
  ReleasePollResponse,
  SeriesMatch,
  SourceStateGetRequest,
  SourceStateSetRequest,
  SourceStateView,
  TrackedSeriesEntry,
} from "./releases.js";
export { RELEASES_METHODS } from "./releases.js";
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
