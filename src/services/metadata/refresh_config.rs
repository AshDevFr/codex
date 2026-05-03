//! Configuration for the scheduled per-library metadata refresh.
//!
//! Stored as JSON in `libraries.metadata_refresh_config`. NULL in the database
//! means "feature off" — readers get [`MetadataRefreshConfig::default`] which
//! is the safe, opt-in default.
//!
//! # Shape (JSON)
//!
//! ```json
//! {
//!   "enabled": false,
//!   "cron_schedule": "0 0 4 * * *",
//!   "timezone": null,
//!   "field_groups": ["ratings", "status", "counts"],
//!   "extra_fields": [],
//!   "providers": [],
//!   "existing_source_ids_only": true,
//!   "skip_recently_synced_within_s": 3600,
//!   "max_concurrency": 4,
//!   "per_provider_overrides": null
//! }
//! ```
//!
//! All fields have safe defaults so a partial document round-trips cleanly.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Default cron schedule: every day at 04:00 (server timezone).
///
/// 6-field cron expression matching the syntax used by `tokio_cron_scheduler`
/// for the rest of the codebase.
pub const DEFAULT_CRON_SCHEDULE: &str = "0 0 4 * * *";

/// Default safety window: skip series whose external IDs were synced within
/// this many seconds. 1 hour is a reasonable floor for daily cadences.
pub const DEFAULT_SKIP_RECENTLY_SYNCED_SECS: u32 = 3600;

/// Default fan-out per task. Conservative because providers may share rate
/// limits per host.
pub const DEFAULT_MAX_CONCURRENCY: u8 = 4;

/// Per-library scheduled metadata-refresh configuration.
///
/// Stored as JSON. Use [`Self::merge_partial`] to apply a PATCH body without
/// clobbering unspecified fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MetadataRefreshConfig {
    /// Whether the schedule fires automatically. When `false`, the scheduler
    /// has no entry for this library; manual `run-now` still works.
    #[serde(default)]
    pub enabled: bool,

    /// 6-field cron expression. Defaults to [`DEFAULT_CRON_SCHEDULE`].
    #[serde(default = "default_cron_schedule")]
    pub cron_schedule: String,

    /// Optional IANA timezone name (e.g. `"Europe/Paris"`). When `None`, the
    /// scheduler falls back to the server timezone.
    #[serde(default)]
    pub timezone: Option<String>,

    /// User-facing field groups to refresh. Translated to a concrete field
    /// list by `field_groups::fields_for_groups` in Phase 3.
    #[serde(default = "default_field_groups")]
    pub field_groups: Vec<String>,

    /// Extra individual field names not covered by any group. Power-user
    /// hatch; usually empty.
    #[serde(default)]
    pub extra_fields: Vec<String>,

    /// Plugin IDs (e.g. `"plugin:mangabaka"`) to query. An empty list means
    /// "no providers configured" — the task short-circuits.
    #[serde(default)]
    pub providers: Vec<String>,

    /// When true, the planner skips any series without a stored external ID
    /// for the chosen provider. Default `true` keeps user-curated matches
    /// safe from re-matching.
    #[serde(default = "default_existing_source_ids_only")]
    pub existing_source_ids_only: bool,

    /// Skip series whose `series_external_ids.last_synced_at` is younger
    /// than this many seconds. `0` disables the guard.
    #[serde(default = "default_skip_recently_synced")]
    pub skip_recently_synced_within_s: u32,

    /// Per-task fan-out. Bounded `JoinSet` size in Phase 2.
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: u8,

    /// Phase 8 hatch: per-provider field allowlist overrides. `None` until
    /// Phase 8 lands; preserved through round-trips.
    #[serde(default)]
    pub per_provider_overrides: Option<BTreeMap<String, ProviderOverride>>,
}

/// Per-provider override placeholder for Phase 8. Kept here so the config
/// schema is forward-compatible without future migrations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProviderOverride {
    #[serde(default)]
    pub field_groups: Vec<String>,
    #[serde(default)]
    pub extra_fields: Vec<String>,
}

fn default_cron_schedule() -> String {
    DEFAULT_CRON_SCHEDULE.to_string()
}

fn default_field_groups() -> Vec<String> {
    vec![
        "ratings".to_string(),
        "status".to_string(),
        "counts".to_string(),
    ]
}

fn default_existing_source_ids_only() -> bool {
    true
}

fn default_skip_recently_synced() -> u32 {
    DEFAULT_SKIP_RECENTLY_SYNCED_SECS
}

fn default_max_concurrency() -> u8 {
    DEFAULT_MAX_CONCURRENCY
}

impl Default for MetadataRefreshConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            cron_schedule: default_cron_schedule(),
            timezone: None,
            field_groups: default_field_groups(),
            extra_fields: Vec::new(),
            providers: Vec::new(),
            existing_source_ids_only: default_existing_source_ids_only(),
            skip_recently_synced_within_s: default_skip_recently_synced(),
            max_concurrency: default_max_concurrency(),
            per_provider_overrides: None,
        }
    }
}

/// Partial PATCH body for the refresh-config CRUD endpoint (Phase 6).
///
/// `Option<Option<T>>` semantics:
/// - `None` (`#[serde(default)]`): field absent from the body, do not touch.
/// - `Some(None)`: explicitly clear/null the field.
/// - `Some(Some(x))`: set the field to `x`.
///
/// Fields that are intrinsically non-nullable (`enabled`, `cron_schedule`,
/// numeric counters) use plain `Option<T>` because clearing them doesn't make
/// sense. `timezone` and `per_provider_overrides` use the double-Option form
/// since they are nullable in storage.
///
/// Consumed by the Phase 6 PATCH endpoint; allowed to be unused until then.
#[allow(dead_code)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", default)]
pub struct MetadataRefreshConfigPatch {
    pub enabled: Option<bool>,
    pub cron_schedule: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub timezone: Option<Option<String>>,
    pub field_groups: Option<Vec<String>>,
    pub extra_fields: Option<Vec<String>>,
    pub providers: Option<Vec<String>>,
    pub existing_source_ids_only: Option<bool>,
    pub skip_recently_synced_within_s: Option<u32>,
    pub max_concurrency: Option<u8>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub per_provider_overrides: Option<Option<BTreeMap<String, ProviderOverride>>>,
}

/// Distinguish "field absent" from "field present with value null" during
/// deserialization. Without this, serde collapses both to `None`.
#[allow(dead_code)]
fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

impl MetadataRefreshConfig {
    /// Apply a PATCH body, leaving fields not mentioned by the patch alone.
    ///
    /// Consumed by the Phase 6 PATCH endpoint; allowed to be unused until then.
    #[allow(dead_code)]
    pub fn merge_partial(&mut self, patch: MetadataRefreshConfigPatch) {
        if let Some(v) = patch.enabled {
            self.enabled = v;
        }
        if let Some(v) = patch.cron_schedule {
            self.cron_schedule = v;
        }
        if let Some(v) = patch.timezone {
            self.timezone = v;
        }
        if let Some(v) = patch.field_groups {
            self.field_groups = v;
        }
        if let Some(v) = patch.extra_fields {
            self.extra_fields = v;
        }
        if let Some(v) = patch.providers {
            self.providers = v;
        }
        if let Some(v) = patch.existing_source_ids_only {
            self.existing_source_ids_only = v;
        }
        if let Some(v) = patch.skip_recently_synced_within_s {
            self.skip_recently_synced_within_s = v;
        }
        if let Some(v) = patch.max_concurrency {
            self.max_concurrency = v;
        }
        if let Some(v) = patch.per_provider_overrides {
            self.per_provider_overrides = v;
        }
    }
}

/// Parse a `MetadataRefreshConfig` from the JSON string stored in
/// `libraries.metadata_refresh_config`.
///
/// Returns `Default::default()` for `None` or empty input. Surfaces parse
/// errors as `Err(String)` for the caller to log; callers usually fall back
/// to the default rather than failing the request.
pub fn parse_metadata_refresh_config(json: Option<&str>) -> Result<MetadataRefreshConfig, String> {
    match json {
        None => Ok(MetadataRefreshConfig::default()),
        Some(s) if s.trim().is_empty() => Ok(MetadataRefreshConfig::default()),
        Some(s) => serde_json::from_str(s)
            .map_err(|e| format!("Failed to parse metadata refresh config: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_disabled_with_safe_values() {
        let cfg = MetadataRefreshConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.cron_schedule, DEFAULT_CRON_SCHEDULE);
        assert!(cfg.timezone.is_none());
        assert_eq!(cfg.field_groups, vec!["ratings", "status", "counts"]);
        assert!(cfg.extra_fields.is_empty());
        assert!(cfg.providers.is_empty());
        assert!(cfg.existing_source_ids_only);
        assert_eq!(
            cfg.skip_recently_synced_within_s,
            DEFAULT_SKIP_RECENTLY_SYNCED_SECS
        );
        assert_eq!(cfg.max_concurrency, DEFAULT_MAX_CONCURRENCY);
        assert!(cfg.per_provider_overrides.is_none());
    }

    #[test]
    fn serde_round_trip_full() {
        let mut overrides = BTreeMap::new();
        overrides.insert(
            "plugin:anilist".to_string(),
            ProviderOverride {
                field_groups: vec!["ratings".to_string()],
                extra_fields: vec!["coverUrl".to_string()],
            },
        );
        let cfg = MetadataRefreshConfig {
            enabled: true,
            cron_schedule: "0 30 2 * * *".to_string(),
            timezone: Some("Europe/Paris".to_string()),
            field_groups: vec!["ratings".to_string(), "counts".to_string()],
            extra_fields: vec!["language".to_string()],
            providers: vec!["plugin:mangabaka".to_string(), "plugin:anilist".to_string()],
            existing_source_ids_only: false,
            skip_recently_synced_within_s: 7200,
            max_concurrency: 8,
            per_provider_overrides: Some(overrides),
        };

        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: MetadataRefreshConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, parsed);
    }

    #[test]
    fn serde_partial_json_uses_defaults_for_missing_fields() {
        let json = r#"{"enabled": true, "cron_schedule": "0 0 5 * * *"}"#;
        let cfg: MetadataRefreshConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.cron_schedule, "0 0 5 * * *");
        // All other fields fall back to defaults
        assert!(cfg.existing_source_ids_only);
        assert_eq!(cfg.field_groups, vec!["ratings", "status", "counts"]);
        assert_eq!(cfg.max_concurrency, DEFAULT_MAX_CONCURRENCY);
    }

    #[test]
    fn parse_none_returns_default() {
        let cfg = parse_metadata_refresh_config(None).unwrap();
        assert_eq!(cfg, MetadataRefreshConfig::default());
    }

    #[test]
    fn parse_empty_string_returns_default() {
        let cfg = parse_metadata_refresh_config(Some("")).unwrap();
        assert_eq!(cfg, MetadataRefreshConfig::default());
        let cfg = parse_metadata_refresh_config(Some("   ")).unwrap();
        assert_eq!(cfg, MetadataRefreshConfig::default());
    }

    #[test]
    fn parse_invalid_json_errors() {
        let err = parse_metadata_refresh_config(Some("not json")).unwrap_err();
        assert!(err.contains("Failed to parse metadata refresh config"));
    }

    #[test]
    fn parse_valid_json_round_trip() {
        let json = r#"{
            "enabled": true,
            "cron_schedule": "0 0 6 * * *",
            "timezone": "UTC",
            "field_groups": ["ratings"],
            "providers": ["plugin:mangabaka"],
            "existing_source_ids_only": false,
            "skip_recently_synced_within_s": 0,
            "max_concurrency": 2
        }"#;
        let cfg = parse_metadata_refresh_config(Some(json)).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.timezone.as_deref(), Some("UTC"));
        assert_eq!(cfg.field_groups, vec!["ratings"]);
        assert_eq!(cfg.providers, vec!["plugin:mangabaka"]);
        assert!(!cfg.existing_source_ids_only);
        assert_eq!(cfg.skip_recently_synced_within_s, 0);
        assert_eq!(cfg.max_concurrency, 2);
    }

    #[test]
    fn merge_partial_overwrites_only_present_fields() {
        let mut cfg = MetadataRefreshConfig::default();
        let original_cron = cfg.cron_schedule.clone();

        let patch = MetadataRefreshConfigPatch {
            enabled: Some(true),
            field_groups: Some(vec!["ratings".to_string()]),
            ..Default::default()
        };
        cfg.merge_partial(patch);

        assert!(cfg.enabled);
        assert_eq!(cfg.field_groups, vec!["ratings"]);
        // Untouched fields keep their prior values.
        assert_eq!(cfg.cron_schedule, original_cron);
        assert!(cfg.existing_source_ids_only);
    }

    #[test]
    fn merge_partial_clears_nullable_fields() {
        let mut cfg = MetadataRefreshConfig {
            timezone: Some("Europe/Paris".to_string()),
            ..Default::default()
        };

        // Explicit null clears the field
        let patch: MetadataRefreshConfigPatch =
            serde_json::from_str(r#"{"timezone": null}"#).unwrap();
        cfg.merge_partial(patch);
        assert!(cfg.timezone.is_none());
    }

    #[test]
    fn merge_partial_absent_field_preserves_value() {
        let mut cfg = MetadataRefreshConfig {
            timezone: Some("UTC".to_string()),
            ..Default::default()
        };

        // Absent field should leave timezone untouched
        let patch: MetadataRefreshConfigPatch =
            serde_json::from_str(r#"{"enabled": true}"#).unwrap();
        cfg.merge_partial(patch);
        assert_eq!(cfg.timezone.as_deref(), Some("UTC"));
        assert!(cfg.enabled);
    }

    #[test]
    fn merge_partial_sets_then_clears_per_provider_overrides() {
        let mut cfg = MetadataRefreshConfig::default();

        // Set
        let patch: MetadataRefreshConfigPatch = serde_json::from_str(
            r#"{"per_provider_overrides": {"plugin:anilist": {"field_groups": ["ratings"]}}}"#,
        )
        .unwrap();
        cfg.merge_partial(patch);
        assert!(cfg.per_provider_overrides.is_some());

        // Clear via explicit null
        let patch: MetadataRefreshConfigPatch =
            serde_json::from_str(r#"{"per_provider_overrides": null}"#).unwrap();
        cfg.merge_partial(patch);
        assert!(cfg.per_provider_overrides.is_none());
    }
}
