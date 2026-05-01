//! Bulk metadata editing DTOs
//!
//! Request and response types for bulk metadata editing operations:
//! - Bulk PATCH for scalar metadata fields
//! - Bulk tag/genre add/remove
//! - Bulk lock toggling

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::book::{BookAuthorDto, UpdateBookMetadataLocksRequest};
use super::series::UpdateMetadataLocksRequest;

// ============================================================================
// Shared Response
// ============================================================================

/// Response for bulk metadata update operations
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkMetadataUpdateResponse {
    /// Number of items that were updated
    #[schema(example = 5)]
    pub updated_count: usize,

    /// Descriptive message about the operation
    #[schema(example = "Updated metadata for 5 series")]
    pub message: String,
}

// ============================================================================
// Bulk Metadata PATCH - Series
// ============================================================================

/// Bulk PATCH request for series metadata
///
/// Applies the same partial update to all specified series.
/// Only provided fields will be updated. Absent fields are unchanged.
/// Explicitly null fields will be cleared.
/// Title, title_sort, and summary are excluded (too unique per item).
/// Max 100 series per request.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkPatchSeriesMetadataRequest {
    /// Series IDs to update (max 100)
    pub series_ids: Vec<Uuid>,

    /// Publisher name
    #[serde(default)]
    #[schema(value_type = Option<String>, nullable = true)]
    pub publisher: super::patch::PatchValue<String>,

    /// Imprint (sub-publisher)
    #[serde(default)]
    #[schema(value_type = Option<String>, nullable = true)]
    pub imprint: super::patch::PatchValue<String>,

    /// Series status (ongoing, ended, hiatus, abandoned, unknown)
    #[serde(default)]
    #[schema(value_type = Option<String>, nullable = true)]
    pub status: super::patch::PatchValue<String>,

    /// Age rating (e.g., 13, 16, 18)
    #[serde(default)]
    #[schema(value_type = Option<i32>, nullable = true)]
    pub age_rating: super::patch::PatchValue<i32>,

    /// Language (BCP47 format: "en", "ja", "ko")
    #[serde(default)]
    #[schema(value_type = Option<String>, nullable = true)]
    pub language: super::patch::PatchValue<String>,

    /// Reading direction (ltr, rtl, ttb or webtoon)
    #[serde(default)]
    #[schema(value_type = Option<String>, nullable = true)]
    pub reading_direction: super::patch::PatchValue<String>,

    /// Release year
    #[serde(default)]
    #[schema(value_type = Option<i32>, nullable = true)]
    pub year: super::patch::PatchValue<i32>,

    /// Expected total book count (for ongoing series).
    ///
    /// DEPRECATED: kept for one phase of backward-compat with API clients
    /// pinned to the legacy field. Sets `total_volume_count` under the hood.
    /// Removed in Phase 9 of the metadata-count-split plan.
    #[serde(default)]
    #[schema(value_type = Option<i32>, nullable = true)]
    pub total_book_count: super::patch::PatchValue<i32>,

    /// Expected total volume count (for volume-organized series).
    #[serde(default)]
    #[schema(value_type = Option<i32>, nullable = true)]
    pub total_volume_count: super::patch::PatchValue<i32>,

    /// Expected total chapter count (for chapter-organized series). May be fractional.
    #[serde(default)]
    #[schema(value_type = Option<f32>, nullable = true)]
    pub total_chapter_count: super::patch::PatchValue<f32>,

    /// Custom JSON metadata (uses RFC 7386 JSON Merge Patch semantics)
    #[serde(default)]
    #[schema(value_type = Option<Object>, nullable = true)]
    pub custom_metadata: super::patch::PatchValue<serde_json::Value>,

    /// Structured author information
    #[serde(default)]
    #[schema(value_type = Option<Vec<BookAuthorDto>>, nullable = true)]
    pub authors: super::patch::PatchValue<Vec<BookAuthorDto>>,
}

// ============================================================================
// Bulk Metadata PATCH - Books
// ============================================================================

/// Bulk PATCH request for book metadata
///
/// Applies the same partial update to all specified books.
/// Only provided fields will be updated. Absent fields are unchanged.
/// Title, title_sort, number, summary, subtitle are excluded (too unique per item).
/// Max 500 books per request.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkPatchBookMetadataRequest {
    /// Book IDs to update (max 500)
    pub book_ids: Vec<Uuid>,

    /// Publisher name
    #[serde(default)]
    #[schema(value_type = Option<String>, nullable = true)]
    pub publisher: super::patch::PatchValue<String>,

    /// Imprint (sub-publisher)
    #[serde(default)]
    #[schema(value_type = Option<String>, nullable = true)]
    pub imprint: super::patch::PatchValue<String>,

    /// Genre (legacy single genre field)
    #[serde(default)]
    #[schema(value_type = Option<String>, nullable = true)]
    pub genre: super::patch::PatchValue<String>,

    /// Language ISO code
    #[serde(default)]
    #[schema(value_type = Option<String>, nullable = true)]
    pub language_iso: super::patch::PatchValue<String>,

    /// Book type (comic, manga, novel, etc.)
    #[serde(default)]
    #[schema(value_type = Option<String>, nullable = true)]
    pub book_type: super::patch::PatchValue<String>,

    /// Translator name
    #[serde(default)]
    #[schema(value_type = Option<String>, nullable = true)]
    pub translator: super::patch::PatchValue<String>,

    /// Edition (e.g., "Deluxe", "Omnibus")
    #[serde(default)]
    #[schema(value_type = Option<String>, nullable = true)]
    pub edition: super::patch::PatchValue<String>,

    /// Original title (if translated)
    #[serde(default)]
    #[schema(value_type = Option<String>, nullable = true)]
    pub original_title: super::patch::PatchValue<String>,

    /// Original publication year
    #[serde(default)]
    #[schema(value_type = Option<i32>, nullable = true)]
    pub original_year: super::patch::PatchValue<i32>,

    /// Whether the book is in black and white
    #[serde(default)]
    #[schema(value_type = Option<bool>, nullable = true)]
    pub black_and_white: super::patch::PatchValue<bool>,

    /// Whether the book is manga format
    #[serde(default)]
    #[schema(value_type = Option<bool>, nullable = true)]
    pub manga: super::patch::PatchValue<bool>,

    /// Custom JSON metadata (uses RFC 7386 JSON Merge Patch semantics)
    #[serde(default)]
    #[schema(value_type = Option<Object>, nullable = true)]
    pub custom_metadata: super::patch::PatchValue<serde_json::Value>,

    /// Structured author information
    #[serde(default)]
    #[schema(value_type = Option<Vec<BookAuthorDto>>, nullable = true)]
    pub authors: super::patch::PatchValue<Vec<BookAuthorDto>>,
}

// ============================================================================
// Bulk Tags/Genres Add/Remove
// ============================================================================

/// Request to add/remove tags for multiple series
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkModifySeriesTagsRequest {
    /// Series IDs to modify (max 100)
    pub series_ids: Vec<Uuid>,

    /// Tag names to add to all specified series
    #[serde(default)]
    #[schema(example = json!(["Action", "Completed"]))]
    pub add: Vec<String>,

    /// Tag names to remove from all specified series
    #[serde(default)]
    #[schema(example = json!(["Dropped"]))]
    pub remove: Vec<String>,
}

/// Request to add/remove genres for multiple series
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkModifySeriesGenresRequest {
    /// Series IDs to modify (max 100)
    pub series_ids: Vec<Uuid>,

    /// Genre names to add to all specified series
    #[serde(default)]
    #[schema(example = json!(["Action", "Comedy"]))]
    pub add: Vec<String>,

    /// Genre names to remove from all specified series
    #[serde(default)]
    #[schema(example = json!(["Romance"]))]
    pub remove: Vec<String>,
}

/// Request to add/remove tags for multiple books
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkModifyBookTagsRequest {
    /// Book IDs to modify (max 500)
    pub book_ids: Vec<Uuid>,

    /// Tag names to add to all specified books
    #[serde(default)]
    #[schema(example = json!(["Favorite", "Reread"]))]
    pub add: Vec<String>,

    /// Tag names to remove from all specified books
    #[serde(default)]
    #[schema(example = json!(["Dropped"]))]
    pub remove: Vec<String>,
}

/// Request to add/remove genres for multiple books
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkModifyBookGenresRequest {
    /// Book IDs to modify (max 500)
    pub book_ids: Vec<Uuid>,

    /// Genre names to add to all specified books
    #[serde(default)]
    #[schema(example = json!(["Action", "Comedy"]))]
    pub add: Vec<String>,

    /// Genre names to remove from all specified books
    #[serde(default)]
    #[schema(example = json!(["Romance"]))]
    pub remove: Vec<String>,
}

// ============================================================================
// Bulk Lock Updates
// ============================================================================

/// Request to update metadata locks for multiple series
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkUpdateSeriesLocksRequest {
    /// Series IDs to update locks for (max 100)
    pub series_ids: Vec<Uuid>,

    /// Lock states to apply to all specified series
    #[serde(flatten)]
    pub locks: UpdateMetadataLocksRequest,
}

/// Request to update metadata locks for multiple books
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkUpdateBookLocksRequest {
    /// Book IDs to update locks for (max 500)
    pub book_ids: Vec<Uuid>,

    /// Lock states to apply to all specified books
    #[serde(flatten)]
    pub locks: UpdateBookMetadataLocksRequest,
}
