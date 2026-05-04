/**
 * Re-export generated API types with convenient aliases
 *
 * This module provides type aliases that match the previous manual type names
 * for easier migration and cleaner imports.
 */

import type { components } from "./api.generated";

// =============================================================================
// Schema type shorthand
// =============================================================================
type Schemas = components["schemas"];

// =============================================================================
// User types
// =============================================================================
export type User = Schemas["UserInfo"];
export type UserDto = Schemas["UserDto"];
export type ApiKeyDto = Schemas["ApiKeyDto"];

// =============================================================================
// Auth types
// =============================================================================
export type LoginRequest = Schemas["LoginRequest"];
export type LoginResponse = Schemas["LoginResponse"];
export type RegisterRequest = Schemas["RegisterRequest"];
export type RegisterResponse = Schemas["RegisterResponse"];

// =============================================================================
// Library types
// =============================================================================
export type Library = Schemas["LibraryDto"];
export type ScanningConfig = Schemas["ScanningConfigDto"];
export type CreateLibraryRequest = Schemas["CreateLibraryRequest"];

// =============================================================================
// Strategy types
// =============================================================================
export type SeriesStrategy = Schemas["SeriesStrategy"];
export type BookStrategy = Schemas["BookStrategy"];
export type NumberStrategy = Schemas["NumberStrategy"];
export type PreviewScanRequest = Schemas["PreviewScanRequest"];
export type PreviewScanResponse = Schemas["PreviewScanResponse"];
export type DetectedSeries = Schemas["DetectedSeriesDto"];
export type DetectedSeriesMetadata = Schemas["DetectedSeriesMetadataDto"];

// =============================================================================
// Series types
// =============================================================================
export type Series = Schemas["SeriesDto"];
/** Full series response including complete metadata, genres, tags, etc. */
export type FullSeries = Schemas["FullSeriesResponse"];
/** Full series metadata response with locks */
export type FullSeriesMetadata = Schemas["FullSeriesMetadataResponse"];
/** Series metadata with nested metadata and locks */
export type SeriesFullMetadata = Schemas["SeriesFullMetadata"];

// =============================================================================
// Series Context types (for template evaluation)
// =============================================================================
/**
 * Series context for template and condition evaluation.
 * Used by CustomMetadataDisplay and template editors.
 */
export type SeriesContext = Schemas["SeriesContextDto"];
/** Metadata context within SeriesContext */
export type MetadataContext = Schemas["MetadataContextDto"];
/** External ID context within SeriesContext */
export type ExternalIdContext = Schemas["ExternalIdContextDto"];

// =============================================================================
// Book types
// =============================================================================
export type Book = Schemas["BookDto"];
/** Full book response including complete metadata with locks */
export type FullBook = Schemas["FullBookResponse"];
/** Book metadata with all fields and lock states */
export type BookFullMetadata = Schemas["BookFullMetadata"];
export type ReadProgress = Schemas["ReadProgressResponse"];
export type BookTypeDto = Schemas["BookTypeDto"];

// =============================================================================
// Book Context types (for template evaluation)
// =============================================================================
/**
 * Book context for template and condition evaluation.
 * Used by CustomMetadataDisplay and template editors.
 */
export type BookContext = Schemas["BookContextDto"];
/** Book metadata context within BookContext */
export type BookMetadataContext = Schemas["BookMetadataContextDto"];
/** Book award context within BookMetadataContext */
export type BookAwardContext = Schemas["BookAwardContextDto"];

// =============================================================================
// Filesystem types
// =============================================================================
export type FileSystemEntry = Schemas["FileSystemEntry"];
export type BrowseResponse = Schemas["BrowseResponse"];

// =============================================================================
// Pagination types
// =============================================================================

/** HATEOAS pagination links for navigating paginated responses (RFC 8288) */
export type PaginationLinks = Schemas["PaginationLinks"];

export type PaginatedResponse<T> = Omit<
  Schemas["PaginatedResponse"],
  "data"
> & {
  data: T[];
};

// =============================================================================
// OIDC types
// =============================================================================

/** Information about an available OIDC provider */
export interface OidcProviderInfo {
  /** Internal name of the provider (used in URLs) */
  name: string;
  /** Display name shown to users */
  displayName: string;
  /** URL to initiate login with this provider */
  loginUrl: string;
}

/** Response listing available OIDC providers */
export interface OidcProvidersResponse {
  /** Whether OIDC authentication is enabled */
  enabled: boolean;
  /** List of available OIDC providers */
  providers: OidcProviderInfo[];
}

/** Response from initiating OIDC login */
export interface OidcLoginResponse {
  /** URL to redirect the user to for authentication */
  redirectUrl: string;
}

// =============================================================================
// Error types
// =============================================================================
export type ApiError = Schemas["ErrorResponse"];

// =============================================================================
// Setup types
// =============================================================================
export type SetupStatusResponse = Schemas["SetupStatusResponse"];
export type InitializeSetupRequest = Schemas["InitializeSetupRequest"];
export type InitializeSetupResponse = Schemas["InitializeSetupResponse"];
export type ConfigureSettingsRequest = Schemas["ConfigureSettingsRequest"];
export type ConfigureSettingsResponse = Schemas["ConfigureSettingsResponse"];

// =============================================================================
// Scan types
// =============================================================================
export type ScanStatus = Schemas["TaskStatus"];
export type ScanProgress = Schemas["ScanStatusDto"];

// =============================================================================
// Event types
// =============================================================================
export type EntityType = Schemas["EntityType"];
export type EntityEvent = Schemas["EntityEvent"];
export type EntityChangeEvent = Schemas["EntityChangeEvent"];

// =============================================================================
// Task types
// =============================================================================
export type TaskStatus = Schemas["TaskStatus"];
export type TaskProgress = Schemas["TaskProgress"];
export type TaskProgressEvent = Schemas["TaskProgressEvent"];
export type TaskResponse = Schemas["TaskResponse"];

// =============================================================================
// Type guards for entity events
// =============================================================================
export function isBookEvent(event: EntityEvent): event is EntityEvent & {
  type: "book_created" | "book_updated" | "book_deleted";
} {
  return (
    event.type === "book_created" ||
    event.type === "book_updated" ||
    event.type === "book_deleted"
  );
}

export function isSeriesEvent(event: EntityEvent): event is EntityEvent & {
  type:
    | "series_created"
    | "series_updated"
    | "series_deleted"
    | "series_bulk_purged";
} {
  return (
    event.type === "series_created" ||
    event.type === "series_updated" ||
    event.type === "series_deleted" ||
    event.type === "series_bulk_purged"
  );
}

export function isCoverEvent(
  event: EntityEvent,
): event is EntityEvent & { type: "cover_updated" } {
  return event.type === "cover_updated";
}

export function isLibraryEvent(
  event: EntityEvent,
): event is EntityEvent & { type: "library_updated" | "library_deleted" } {
  return event.type === "library_updated" || event.type === "library_deleted";
}

export function isPluginEvent(event: EntityEvent): event is EntityEvent & {
  type:
    | "plugin_created"
    | "plugin_updated"
    | "plugin_enabled"
    | "plugin_disabled"
    | "plugin_deleted";
} {
  return (
    event.type === "plugin_created" ||
    event.type === "plugin_updated" ||
    event.type === "plugin_enabled" ||
    event.type === "plugin_disabled" ||
    event.type === "plugin_deleted"
  );
}

export function isReleaseAnnouncedEvent(
  event: EntityEvent,
): event is EntityEvent & { type: "release_announced" } {
  return event.type === "release_announced";
}

// =============================================================================
// Re-export the raw generated types for advanced use cases
// =============================================================================
export type { components, operations, paths } from "./api.generated";
// =============================================================================
// Filter types
// =============================================================================
export * from "./filters";
