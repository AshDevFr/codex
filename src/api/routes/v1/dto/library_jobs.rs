//! DTOs for `/api/v1/libraries/{id}/jobs` (Phase 9).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::api::routes::v1::dto::patch::PatchValue;
use crate::services::library_jobs::{LibraryJobConfig, MetadataRefreshJobConfig, RefreshScope};

/// Type-discriminated job config exposed over the wire.
///
/// Phase 9 only ships the `metadata_refresh` variant; future job types
/// extend the enum.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LibraryJobConfigDto {
    MetadataRefresh(MetadataRefreshJobConfigDto),
}

/// Wire shape for the metadata-refresh job config.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRefreshJobConfigDto {
    /// Plugin reference, e.g. `"plugin:mangabaka"`.
    pub provider: String,
    /// Refresh scope. Phase 9 only honours `series_only` at runtime.
    #[serde(default)]
    pub scope: RefreshScope,
    /// Series-side field groups (snake_case identifiers).
    #[serde(default)]
    pub field_groups: Vec<String>,
    /// Series-side individual field overrides (camelCase).
    #[serde(default)]
    pub extra_fields: Vec<String>,
    /// Reserved for the book-scope future work.
    #[serde(default)]
    pub book_field_groups: Vec<String>,
    /// Reserved for the book-scope future work.
    #[serde(default)]
    pub book_extra_fields: Vec<String>,
    /// When true, the planner skips series with no stored external ID.
    #[serde(default = "default_existing_source_ids_only")]
    pub existing_source_ids_only: bool,
    /// Skip series whose `last_synced_at` is younger than this many seconds.
    #[serde(default = "default_skip_recently_synced")]
    pub skip_recently_synced_within_s: u32,
    /// Per-task fan-out; clamped at run time.
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: u8,
}

fn default_existing_source_ids_only() -> bool {
    true
}
fn default_skip_recently_synced() -> u32 {
    3600
}
fn default_max_concurrency() -> u8 {
    4
}

impl From<MetadataRefreshJobConfig> for MetadataRefreshJobConfigDto {
    fn from(c: MetadataRefreshJobConfig) -> Self {
        Self {
            provider: c.provider,
            scope: c.scope,
            field_groups: c.field_groups,
            extra_fields: c.extra_fields,
            book_field_groups: c.book_field_groups,
            book_extra_fields: c.book_extra_fields,
            existing_source_ids_only: c.existing_source_ids_only,
            skip_recently_synced_within_s: c.skip_recently_synced_within_s,
            max_concurrency: c.max_concurrency,
        }
    }
}

impl From<MetadataRefreshJobConfigDto> for MetadataRefreshJobConfig {
    fn from(c: MetadataRefreshJobConfigDto) -> Self {
        Self {
            provider: c.provider,
            scope: c.scope,
            field_groups: c.field_groups,
            extra_fields: c.extra_fields,
            book_field_groups: c.book_field_groups,
            book_extra_fields: c.book_extra_fields,
            existing_source_ids_only: c.existing_source_ids_only,
            skip_recently_synced_within_s: c.skip_recently_synced_within_s,
            max_concurrency: c.max_concurrency,
        }
    }
}

impl From<LibraryJobConfig> for LibraryJobConfigDto {
    fn from(c: LibraryJobConfig) -> Self {
        match c {
            LibraryJobConfig::MetadataRefresh(c) => LibraryJobConfigDto::MetadataRefresh(c.into()),
        }
    }
}

impl From<LibraryJobConfigDto> for LibraryJobConfig {
    fn from(c: LibraryJobConfigDto) -> Self {
        match c {
            LibraryJobConfigDto::MetadataRefresh(c) => LibraryJobConfig::MetadataRefresh(c.into()),
        }
    }
}

/// Library job row exposed via GET / list / response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryJobDto {
    pub id: Uuid,
    pub library_id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub cron_schedule: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    pub config: LibraryJobConfigDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request body for `POST /api/v1/libraries/{id}/jobs`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateLibraryJobRequest {
    /// Optional user-facing name. Auto-generated when missing or empty.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    pub cron_schedule: String,
    #[serde(default)]
    pub timezone: Option<String>,
    pub config: LibraryJobConfigDto,
}

/// Request body for `PATCH /api/v1/libraries/{id}/jobs/{job_id}`.
///
/// All fields are optional. Top-level fields use [`PatchValue`] when their
/// underlying type is `Option<...>` so an explicit `null` clears the value
/// distinct from "not present".
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct PatchLibraryJobRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub cron_schedule: Option<String>,
    #[serde(default, skip_serializing_if = "PatchValue::is_absent")]
    #[schema(value_type = Option<String>, nullable = true)]
    pub timezone: PatchValue<String>,
    /// Replaces the type-specific config wholesale; the type discriminator
    /// must match the existing row's type.
    #[serde(default)]
    pub config: Option<LibraryJobConfigDto>,
}

/// Response for `GET /libraries/{id}/jobs`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListLibraryJobsResponse {
    pub jobs: Vec<LibraryJobDto>,
}

/// Response for `POST .../run-now`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunNowResponse {
    pub task_id: Uuid,
}

/// Request body for `POST .../dry-run`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DryRunRequest {
    /// Override the saved config for this preview only. Must match the
    /// row's `type`.
    #[serde(default)]
    pub config_override: Option<LibraryJobConfigDto>,
    /// Sample size, capped at 20 server-side.
    #[serde(default)]
    pub sample_size: Option<u32>,
}

/// One series's preview of would-be field changes.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DryRunSeriesDelta {
    pub series_id: Uuid,
    pub series_name: String,
    /// Field name → `(before, after)` JSON values.
    pub changes: HashMap<String, DryRunFieldChange>,
    /// Fields that would have been written but were skipped (locks, all-locked, etc.)
    pub skipped: Vec<DryRunSkippedFieldDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DryRunFieldChange {
    pub before: serde_json::Value,
    pub after: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DryRunSkippedFieldDto {
    pub field: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DryRunResponse {
    /// Total number of series eligible to be refreshed (all of them, not
    /// just the sample).
    pub total_eligible: u32,
    /// Per-series deltas for the first N eligible series.
    pub sample: Vec<DryRunSeriesDelta>,
    /// Estimated count of series that would be skipped because they have no
    /// external ID for the chosen provider.
    pub est_skipped_no_id: u32,
    /// Estimated count of series that would be skipped because they were
    /// recently synced.
    pub est_skipped_recently_synced: u32,
    /// Provider resolution failure reason, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_failure: Option<String>,
}

/// Static field-group catalog row exposed for the editor UI.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FieldGroupDto {
    pub id: String,
    pub label: String,
    pub fields: Vec<String>,
}
