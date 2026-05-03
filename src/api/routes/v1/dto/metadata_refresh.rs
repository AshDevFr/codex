//! DTOs for the scheduled metadata-refresh API (Phase 6).
//!
//! Wraps [`MetadataRefreshConfig`] for the wire protocol, plus dedicated
//! request/response types for run-now, dry-run, and field-group enumeration.
//!
//! [`MetadataRefreshConfig`]: crate::services::metadata::MetadataRefreshConfig

use crate::services::metadata::{MetadataRefreshConfig, ProviderOverride};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;
use uuid::Uuid;

use super::patch::PatchValue;
use super::plugins::FieldChangeDto;

// ---------------------------------------------------------------------------
// Config CRUD
// ---------------------------------------------------------------------------

/// Per-provider override placeholder (Phase 8). Mirrors
/// [`crate::services::metadata::ProviderOverride`] for the wire format.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderOverrideDto {
    #[serde(default)]
    pub field_groups: Vec<String>,
    #[serde(default)]
    pub extra_fields: Vec<String>,
}

impl From<ProviderOverride> for ProviderOverrideDto {
    fn from(o: ProviderOverride) -> Self {
        Self {
            field_groups: o.field_groups,
            extra_fields: o.extra_fields,
        }
    }
}

impl From<ProviderOverrideDto> for ProviderOverride {
    fn from(o: ProviderOverrideDto) -> Self {
        Self {
            field_groups: o.field_groups,
            extra_fields: o.extra_fields,
        }
    }
}

/// Full read response for a library's scheduled metadata-refresh config.
///
/// When the library has no stored config, the server returns
/// [`MetadataRefreshConfig::default`] so clients always render something.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRefreshConfigDto {
    pub enabled: bool,
    pub cron_schedule: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    pub field_groups: Vec<String>,
    pub extra_fields: Vec<String>,
    pub providers: Vec<String>,
    pub existing_source_ids_only: bool,
    pub skip_recently_synced_within_s: u32,
    pub max_concurrency: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_provider_overrides: Option<BTreeMap<String, ProviderOverrideDto>>,
}

impl From<MetadataRefreshConfig> for MetadataRefreshConfigDto {
    fn from(c: MetadataRefreshConfig) -> Self {
        Self {
            enabled: c.enabled,
            cron_schedule: c.cron_schedule,
            timezone: c.timezone,
            field_groups: c.field_groups,
            extra_fields: c.extra_fields,
            providers: c.providers,
            existing_source_ids_only: c.existing_source_ids_only,
            skip_recently_synced_within_s: c.skip_recently_synced_within_s,
            max_concurrency: c.max_concurrency,
            per_provider_overrides: c
                .per_provider_overrides
                .map(|m| m.into_iter().map(|(k, v)| (k, v.into())).collect()),
        }
    }
}

/// Partial PATCH body. Uses [`PatchValue`] for nullable fields so clients can
/// distinguish "leave alone" from "explicit clear".
///
/// All other fields use plain `Option<T>` because clearing a non-nullable
/// field doesn't make sense (e.g. you can't "unset" `enabled` — you can only
/// flip it).
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct MetadataRefreshConfigPatchDto {
    pub enabled: Option<bool>,
    pub cron_schedule: Option<String>,
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub timezone: PatchValue<String>,
    pub field_groups: Option<Vec<String>>,
    pub extra_fields: Option<Vec<String>>,
    pub providers: Option<Vec<String>>,
    pub existing_source_ids_only: Option<bool>,
    pub skip_recently_synced_within_s: Option<u32>,
    pub max_concurrency: Option<u8>,
    #[serde(default)]
    #[schema(value_type = Option<Object>)]
    pub per_provider_overrides: PatchValue<BTreeMap<String, ProviderOverrideDto>>,
}

// ---------------------------------------------------------------------------
// Run now / dry run
// ---------------------------------------------------------------------------

/// Response for `POST /libraries/{id}/metadata-refresh/run-now`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunNowResponse {
    /// Background task ID. Subscribe to events on `/api/v1/events/stream` to
    /// follow progress.
    pub task_id: Uuid,
}

/// Body for `POST /libraries/{id}/metadata-refresh/dry-run`.
///
/// `configOverride` lets the UI preview a config that hasn't been saved yet —
/// "what would happen if I clicked Save right now?". When absent, the saved
/// config is used.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct DryRunRequest {
    /// Number of series to preview. Defaults to 5, capped at 20.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_size: Option<u32>,
    /// Optional unsaved config to preview. When absent, the library's saved
    /// config is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_override: Option<MetadataRefreshConfigDto>,
}

/// One series' would-be deltas in a dry-run preview.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DryRunSeriesDelta {
    pub series_id: Uuid,
    pub series_title: String,
    /// Plugin id (`"plugin:<name>"`) that produced this delta.
    pub provider: String,
    /// Fields that would be written.
    pub changes: Vec<FieldChangeDto>,
    /// Fields that would be skipped (locked, no permission, etc.).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<DryRunSkippedFieldDto>,
}

/// A field skipped during a dry-run apply, with the reason.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DryRunSkippedFieldDto {
    pub field: String,
    pub reason: String,
}

/// Full dry-run response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DryRunResponse {
    /// Per-series deltas, capped at the requested sample size.
    pub sample: Vec<DryRunSeriesDelta>,
    /// Total number of `(series, provider)` pairs the planner produced before
    /// the sample cap.
    pub total_eligible: u32,
    /// Estimated `(series, provider)` pairs the planner skipped because the
    /// series has no stored external ID for the provider (strict mode only).
    pub est_skipped_no_id: u32,
    /// Estimated pairs skipped because their `last_synced_at` is younger than
    /// the recency cutoff.
    pub est_skipped_recently_synced: u32,
    /// Provider strings from the config that don't resolve to an enabled
    /// plugin. Surfaced verbatim so the UI can highlight typos or disabled
    /// plugins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved_providers: Vec<String>,
}

// ---------------------------------------------------------------------------
// Field group enumeration
// ---------------------------------------------------------------------------

/// One entry from `GET /api/v1/metadata-refresh/field-groups`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FieldGroupDto {
    /// Snake_case identifier stored in [`MetadataRefreshConfig::field_groups`].
    pub id: String,
    /// Human-readable label for UI display.
    pub label: String,
    /// camelCase field names this group expands into. Match the
    /// `should_apply_field` call sites in `MetadataApplier`.
    pub fields: Vec<String>,
}
