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
// Book types
// =============================================================================
export type Book = Schemas["BookDto"];
/** Full book response including complete metadata with locks */
export type FullBook = Schemas["FullBookResponse"];
/** Book metadata with all fields and lock states */
export type BookFullMetadata = Schemas["BookFullMetadata"];
export type ReadProgress = Schemas["ReadProgressResponse"];

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

// =============================================================================
// Re-export the raw generated types for advanced use cases
// =============================================================================
export type { components, operations, paths } from "./api.generated";
// =============================================================================
// Filter types
// =============================================================================
export * from "./filters";
