//! DTOs for release-tracking config and aliases endpoints.
//!
//! Maps the `series_tracking` sidecar and `series_aliases` table onto the v1
//! HTTP API. Distinct from `series_alternate_titles` — aliases here are
//! arbitrary matcher strings, not labelled localized titles.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::db::entities::{series_aliases, series_tracking};

// =============================================================================
// Tracking config DTOs
// =============================================================================

/// Per-series release-tracking configuration.
///
/// Returned even for untracked series — the row defaults to `tracked: false`
/// with conservative defaults so the frontend can render the panel without
/// special-casing missing rows.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesTrackingDto {
    /// Series ID this config belongs to.
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub series_id: Uuid,
    /// Whether release tracking is enabled.
    pub tracked: bool,
    /// Publication status: `ongoing` | `complete` | `hiatus` | `cancelled` | `unknown`.
    #[schema(example = "ongoing")]
    pub tracking_status: String,
    /// Whether to announce new chapters.
    pub track_chapters: bool,
    /// Whether to announce new volumes.
    pub track_volumes: bool,
    /// Latest known external chapter (supports decimals like 12.5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_known_chapter: Option<f64>,
    /// Latest known external volume.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_known_volume: Option<i32>,
    /// Sparse map of `{ "<volume>": { "first": ch, "last": ch } }`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_chapter_map: Option<serde_json::Value>,
    /// Per-series override of the source poll interval (seconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval_override_s: Option<i32>,
    /// Per-series override of the server's confidence threshold (0.0 - 1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence_threshold_override: Option<f64>,
    /// Per-series language preference (ISO 639-1 codes, e.g. `["en", "es"]`).
    /// `null` means "fall back to the server-wide default (`release_tracking.default_languages`)."
    /// Used by aggregation feeds (e.g. MangaUpdates) that emit candidates in many languages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub languages: Option<Vec<String>>,
    /// When the row was created (epoch when virtual).
    pub created_at: DateTime<Utc>,
    /// When the row was last updated (epoch when virtual).
    pub updated_at: DateTime<Utc>,
}

impl From<series_tracking::Model> for SeriesTrackingDto {
    fn from(m: series_tracking::Model) -> Self {
        Self {
            series_id: m.series_id,
            tracked: m.tracked,
            tracking_status: m.tracking_status,
            track_chapters: m.track_chapters,
            track_volumes: m.track_volumes,
            latest_known_chapter: m.latest_known_chapter,
            latest_known_volume: m.latest_known_volume,
            volume_chapter_map: m.volume_chapter_map,
            poll_interval_override_s: m.poll_interval_override_s,
            confidence_threshold_override: m.confidence_threshold_override,
            languages: m.languages.and_then(|v| serde_json::from_value(v).ok()),
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

/// PATCH payload for tracking config. All fields are optional:
/// omit a field to leave it untouched. Use a JSON `null` on a nullable field
/// to clear it explicitly.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSeriesTrackingRequest {
    pub tracked: Option<bool>,
    /// `ongoing` | `complete` | `hiatus` | `cancelled` | `unknown`.
    pub tracking_status: Option<String>,
    pub track_chapters: Option<bool>,
    pub track_volumes: Option<bool>,
    /// Use `Some(null)` to clear, `Some(<value>)` to set, omit to leave alone.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub latest_known_chapter: Option<Option<f64>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub latest_known_volume: Option<Option<i32>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub volume_chapter_map: Option<Option<serde_json::Value>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub poll_interval_override_s: Option<Option<i32>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub confidence_threshold_override: Option<Option<f64>>,
    /// ISO 639-1 codes; `null` clears (falls back to server-wide default).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub languages: Option<Option<Vec<String>>>,
}

/// `Option<Option<T>>` SerDe helper: distinguishes "field omitted" from "field
/// present and null". The default `Option<T>` flattens both, which collapses
/// the "leave alone vs. clear" distinction we need for PATCH semantics.
mod double_option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S, T>(value: &Option<Option<T>>, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
    {
        match value {
            Some(Some(v)) => v.serialize(ser),
            Some(None) => ser.serialize_none(),
            None => ser.serialize_none(),
        }
    }

    pub fn deserialize<'de, D, T>(de: D) -> Result<Option<Option<T>>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        // Field is present (otherwise serde would call `default`); read it as
        // `Option<T>` so explicit null becomes `Some(None)` and a present value
        // becomes `Some(Some(v))`.
        Option::<T>::deserialize(de).map(Some)
    }
}

// =============================================================================
// Aliases DTOs
// =============================================================================

/// Title alias used by release-source plugins to match incoming releases by
/// title (Nyaa, MangaUpdates without an external ID, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesAliasDto {
    /// Alias row ID.
    #[schema(example = "550e8400-e29b-41d4-a716-446655440100")]
    pub id: Uuid,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub series_id: Uuid,
    /// Alias as entered (preserves casing/punctuation).
    #[schema(example = "My Hero Academia")]
    pub alias: String,
    /// Lowercased + punctuation-stripped form used for matching.
    #[schema(example = "my hero academia")]
    pub normalized: String,
    /// `metadata` (auto-derived) | `manual` (user-entered).
    #[schema(example = "manual")]
    pub source: String,
    pub created_at: DateTime<Utc>,
}

impl From<series_aliases::Model> for SeriesAliasDto {
    fn from(m: series_aliases::Model) -> Self {
        Self {
            id: m.id,
            series_id: m.series_id,
            alias: m.alias,
            normalized: m.normalized,
            source: m.source,
            created_at: m.created_at,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesAliasListResponse {
    pub aliases: Vec<SeriesAliasDto>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSeriesAliasRequest {
    /// Alias text. Will be trimmed; must normalize to non-empty.
    #[schema(example = "Boku no Hero Academia")]
    pub alias: String,
    /// Optional explicit source. Defaults to `manual` when called from the API.
    /// Plugin-internal flows write `metadata`; we don't expose that to HTTP.
    #[serde(default)]
    pub source: Option<String>,
}
