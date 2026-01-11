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
// Series types
// =============================================================================
export type Series = Schemas["SeriesDto"];

// =============================================================================
// Book types
// =============================================================================
export type Book = Schemas["BookDto"];
export type ReadProgress = Schemas["ReadProgressResponse"];

// =============================================================================
// Filesystem types
// =============================================================================
export type FileSystemEntry = Schemas["FileSystemEntry"];
export type BrowseResponse = Schemas["BrowseResponse"];

// =============================================================================
// Pagination types
// =============================================================================
export type PaginatedResponse<T> = Omit<Schemas["PaginatedResponse"], "data"> & {
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
export function isBookEvent(
	event: EntityEvent,
): event is EntityEvent & { type: "book_created" | "book_updated" | "book_deleted" } {
	return (
		event.type === "book_created" ||
		event.type === "book_updated" ||
		event.type === "book_deleted"
	);
}

export function isSeriesEvent(
	event: EntityEvent,
): event is EntityEvent & {
	type: "series_created" | "series_updated" | "series_deleted" | "series_bulk_purged";
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
export type { components, paths, operations } from "./api.generated";
