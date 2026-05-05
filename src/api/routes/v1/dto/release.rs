//! DTOs for the release ledger and release-source admin endpoints.
//!
//! - `ReleaseLedgerEntryDto` mirrors a row in `release_ledger`. Used by the
//!   per-series and inbox views.
//! - `ReleaseSourceDto` mirrors a row in `release_sources`. Used by the
//!   admin source management UI.
//!
//! Note: this module deliberately does NOT introduce a new `ReleaseAnnounced`
//! event variant - that lands in Phase 7 along with the frontend inbox UI.
//! State-change endpoints in this module emit `SeriesUpdated` events with a
//! `releases` field marker so the existing event broadcaster carries them.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::db::entities::{release_ledger, release_sources};

// =============================================================================
// Release ledger DTOs
// =============================================================================

/// A single release announcement. Sources write these; the inbox reads them.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseLedgerEntryDto {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440a00")]
    pub id: Uuid,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub series_id: Uuid,
    /// Series title at the time of the response. Joined from the `series`
    /// table so the inbox UI can render a human-readable label without a
    /// follow-up fetch. Falls back to the empty string only if the series
    /// row was hard-deleted between the join and the read.
    #[schema(example = "Chainsaw Man")]
    pub series_title: String,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440b00")]
    pub source_id: Uuid,
    /// Plugin-stable identity for the release (used for dedup).
    #[schema(example = "nyaa:1234567")]
    pub external_release_id: String,
    /// Torrent info_hash, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info_hash: Option<String>,
    /// Decimal supports `12.5` etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapter: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Sparse `{ "jxl": true, "container": "cbz", ... }`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format_hints: Option<serde_json::Value>,
    /// Group/scanlator/uploader attribution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_or_uploader: Option<String>,
    /// Where to acquire the release. Conventionally a human-readable
    /// landing page (Nyaa view page, MangaUpdates release page).
    pub payload_url: String,
    /// Optional second URL for direct fetch (`.torrent`, `magnet:`, DDL
    /// link). Travels paired with [`Self::media_url_kind`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_url: Option<String>,
    /// Classifies what `media_url` points at: `torrent` | `magnet` |
    /// `direct` | `other`. The frontend uses this to pick a kind-specific
    /// icon next to the standard external-link icon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_url_kind: Option<String>,
    pub confidence: f64,
    /// `announced` | `dismissed` | `marked_acquired` | `hidden`.
    pub state: String,
    /// Source-specific extras (free-form).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    pub observed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

impl ReleaseLedgerEntryDto {
    /// Build a DTO from a ledger row plus the joined series title. The title
    /// must be looked up by the caller (typically a batch query in the
    /// handler) since `From<Model>` alone can't carry it.
    pub fn from_model_with_series_title(m: release_ledger::Model, series_title: String) -> Self {
        Self {
            id: m.id,
            series_id: m.series_id,
            series_title,
            source_id: m.source_id,
            external_release_id: m.external_release_id,
            info_hash: m.info_hash,
            chapter: m.chapter,
            volume: m.volume,
            language: m.language,
            format_hints: m.format_hints,
            group_or_uploader: m.group_or_uploader,
            payload_url: m.payload_url,
            media_url: m.media_url,
            media_url_kind: m.media_url_kind,
            confidence: m.confidence,
            state: m.state,
            metadata: m.metadata,
            observed_at: m.observed_at,
            created_at: m.created_at,
        }
    }
}

/// PATCH payload for ledger row state transitions.
///
/// Only `state` is patchable from the API today; the rest of the row is
/// source-controlled. `state` is validated against the canonical set:
/// `announced` | `dismissed` | `marked_acquired` | `hidden`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReleaseLedgerEntryRequest {
    /// New state. See [`ReleaseLedgerEntryDto::state`] for allowed values.
    pub state: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseLedgerListResponse {
    pub entries: Vec<ReleaseLedgerEntryDto>,
}

// =============================================================================
// Release source DTOs
// =============================================================================

/// A configured release source (one row per logical feed).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseSourceDto {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440b00")]
    pub id: Uuid,
    /// Owning plugin id, or `core` for in-core synthetic sources.
    #[schema(example = "release-nyaa")]
    pub plugin_id: String,
    /// Plugin-defined unique key.
    #[schema(example = "nyaa:user:tsuna69")]
    pub source_key: String,
    pub display_name: String,
    /// `rss-uploader` | `rss-series` | `api-feed` | `metadata-feed` | `metadata-piggyback`.
    pub kind: String,
    pub enabled: bool,
    pub poll_interval_s: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_polled_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_at: Option<DateTime<Utc>>,
    /// Opaque etag/cursor used for conditional fetches.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// Source-specific configuration (free-form).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    /// One-line summary of the most recent successful poll. Surfaced under
    /// the row's status badge so users can see *why* a poll returned no
    /// announcements without grepping logs. NULL until the first successful
    /// poll on the source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<release_sources::Model> for ReleaseSourceDto {
    fn from(m: release_sources::Model) -> Self {
        Self {
            id: m.id,
            plugin_id: m.plugin_id,
            source_key: m.source_key,
            display_name: m.display_name,
            kind: m.kind,
            enabled: m.enabled,
            poll_interval_s: m.poll_interval_s,
            last_polled_at: m.last_polled_at,
            last_error: m.last_error,
            last_error_at: m.last_error_at,
            etag: m.etag,
            config: m.config,
            last_summary: m.last_summary,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseSourceListResponse {
    pub sources: Vec<ReleaseSourceDto>,
}

/// PATCH payload for a release source. All fields optional; omit to leave alone.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReleaseSourceRequest {
    pub display_name: Option<String>,
    pub enabled: Option<bool>,
    /// Polling interval override (seconds). Must be > 0.
    pub poll_interval_s: Option<i32>,
}

/// Response shape from the `reset` endpoint.
///
/// Returns the number of ledger rows removed so callers can show a
/// confirmation toast. The source's transient poll state (etag,
/// last_polled_at, last_error, last_summary) is also cleared, but those
/// are not counted here.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResetReleaseSourceResponse {
    /// Number of `release_ledger` rows deleted for this source.
    pub deleted_ledger_entries: u64,
}

// =============================================================================
// Facets
// =============================================================================

/// One series option in the inbox facets response. Carries the joined
/// `library_id` and `library_name` so the frontend can group the dropdown
/// by library without a follow-up call.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseSeriesFacetDto {
    pub series_id: Uuid,
    pub series_title: String,
    pub library_id: Uuid,
    pub library_name: String,
    /// Number of ledger rows matching the active filter for this series.
    pub count: u64,
}

/// One library option in the inbox facets response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseLibraryFacetDto {
    pub library_id: Uuid,
    pub library_name: String,
    pub count: u64,
}

/// One language option in the inbox facets response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseLanguageFacetDto {
    pub language: String,
    pub count: u64,
}

/// Response shape for `GET /api/v1/releases/facets`.
///
/// Each list reflects the distinct values present in the ledger under the
/// **other** active filters (Solr-style facet exclusion), so dropdowns
/// never offer combinations that would yield zero results. The frontend
/// uses these to populate cascading filter Select inputs without forcing
/// the user to type UUIDs.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseFacetsResponse {
    pub languages: Vec<ReleaseLanguageFacetDto>,
    pub libraries: Vec<ReleaseLibraryFacetDto>,
    pub series: Vec<ReleaseSeriesFacetDto>,
}

// =============================================================================
// Bulk operations
// =============================================================================

/// Action requested by `POST /api/v1/releases/bulk`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BulkReleaseAction {
    /// Set state to `dismissed`.
    Dismiss,
    /// Set state to `marked_acquired`.
    MarkAcquired,
    /// Hard-delete the ledger rows. Each affected source's `etag` is
    /// cleared so the next poll re-fetches without `If-None-Match` and
    /// re-announces the deleted releases.
    Delete,
}

/// Request body for `POST /api/v1/releases/bulk`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkReleaseActionRequest {
    pub ids: Vec<Uuid>,
    pub action: BulkReleaseAction,
}

/// Response from `POST /api/v1/releases/bulk`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkReleaseActionResponse {
    /// Number of ledger rows actually affected. Less than `ids.len()` when
    /// some IDs were already deleted concurrently.
    pub affected: u64,
    /// Action that ran (echoed back for client-side confirmation toasts).
    pub action: BulkReleaseAction,
}

/// Response from `DELETE /api/v1/releases/{id}`.
///
/// Single-row delete returns a small confirmation rather than 204 so the
/// frontend can surface a toast that mentions the etag clear ("the next
/// poll will re-fetch this release"). Mirrors the bulk-delete shape with
/// `affected = 1`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteReleaseResponse {
    /// `true` if the row was deleted, `false` if it didn't exist.
    pub deleted: bool,
}

/// Response shape from the `poll-now` endpoint.
///
/// `status` is `enqueued` after a successful enqueue. The `message` carries
/// the task ID for follow-up (`tasks.id`); the task runs asynchronously, so
/// this response does not reflect poll outcome.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PollNowResponse {
    /// `enqueued` on success.
    pub status: String,
    /// Human-readable message; includes the enqueued task ID.
    pub message: String,
}
