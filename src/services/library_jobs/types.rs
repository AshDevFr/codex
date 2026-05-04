//! Typed configs for [`library_jobs`].
//!
//! [`LibraryJobConfig`] is a discriminated union keyed on the `type` row
//! column. Phase 9 ships with `metadata_refresh`; future variants extend
//! the enum.
//!
//! [`library_jobs`]: crate::db::entities::library_jobs

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Default safety window: skip series whose external IDs were synced within
/// this many seconds.
pub const DEFAULT_SKIP_RECENTLY_SYNCED_SECS: u32 = 3600;

/// Default fan-out per task.
pub const DEFAULT_MAX_CONCURRENCY: u8 = 4;

/// Hard cap on `max_concurrency` accepted from user input.
pub const MAX_CONCURRENCY_HARD_CAP: u8 = 16;

/// Stable string discriminators for [`LibraryJobConfig`]. Mirrors the
/// `library_jobs.type` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryJobType {
    /// Scheduled metadata refresh, scoped to a single provider.
    MetadataRefresh,
}

impl LibraryJobType {
    /// Stable wire identifier for storage and PATCH bodies.
    pub fn as_str(&self) -> &'static str {
        match self {
            LibraryJobType::MetadataRefresh => "metadata_refresh",
        }
    }

    #[cfg(test)]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "metadata_refresh" => Some(LibraryJobType::MetadataRefresh),
            _ => None,
        }
    }
}

/// Type-discriminated payload stored in [`library_jobs.config`].
///
/// The serde representation is **internally tagged** under the JSON key
/// `type`. Each variant carries its own typed payload. Phase 9 only ships
/// the `metadata_refresh` variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LibraryJobConfig {
    MetadataRefresh(MetadataRefreshJobConfig),
}

impl LibraryJobConfig {
    /// Discriminator for this variant. Matches the row's `type` column.
    pub fn job_type(&self) -> LibraryJobType {
        match self {
            LibraryJobConfig::MetadataRefresh(_) => LibraryJobType::MetadataRefresh,
        }
    }
}

/// Scope of a metadata refresh job.
///
/// Phase 9 only honours [`RefreshScope::SeriesOnly`] at runtime. The
/// other variants are schema-accepted but rejected by the validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum RefreshScope {
    #[default]
    SeriesOnly,
    BooksOnly,
    SeriesAndBooks,
}

impl RefreshScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            RefreshScope::SeriesOnly => "series_only",
            RefreshScope::BooksOnly => "books_only",
            RefreshScope::SeriesAndBooks => "series_and_books",
        }
    }

    /// Whether this scope writes to series metadata.
    pub fn writes_series(&self) -> bool {
        matches!(
            self,
            RefreshScope::SeriesOnly | RefreshScope::SeriesAndBooks
        )
    }

    /// Whether this scope writes to book metadata.
    pub fn writes_books(&self) -> bool {
        matches!(self, RefreshScope::BooksOnly | RefreshScope::SeriesAndBooks)
    }
}

/// Payload for a `metadata_refresh` job.
///
/// One job = one (library, single provider, single cron, field selection,
/// safety options) tuple. The library and cron live on the row; this struct
/// captures the type-specific fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MetadataRefreshJobConfig {
    /// Plugin reference, e.g. `"plugin:mangabaka"`. The validator must
    /// resolve this to an installed plugin (disabled is fine).
    pub provider: String,

    /// Refresh scope. Phase 9 only allows [`RefreshScope::SeriesOnly`].
    #[serde(default)]
    pub scope: RefreshScope,

    /// Series-side field groups (snake_case). Resolved to the applier's
    /// camelCase field names by `field_groups::fields_for_groups`.
    #[serde(default)]
    pub field_groups: Vec<String>,

    /// Series-side individual field overrides not covered by any group.
    #[serde(default)]
    pub extra_fields: Vec<String>,

    /// Reserved for the book-scope future work. Phase 9 rejects non-empty
    /// values when [`Self::scope`] is `series_only`.
    #[serde(default)]
    pub book_field_groups: Vec<String>,

    /// Reserved for the book-scope future work. Phase 9 rejects non-empty
    /// values when [`Self::scope`] is `series_only`.
    #[serde(default)]
    pub book_extra_fields: Vec<String>,

    /// When true, the planner skips any series without a stored external ID
    /// for the chosen provider.
    #[serde(default = "default_existing_source_ids_only")]
    pub existing_source_ids_only: bool,

    /// Skip series whose `series_external_ids.last_synced_at` is younger
    /// than this many seconds. `0` disables the guard.
    #[serde(default = "default_skip_recently_synced")]
    pub skip_recently_synced_within_s: u32,

    /// Per-task fan-out; clamped to `[1, MAX_CONCURRENCY_HARD_CAP]` by the
    /// handler.
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: u8,
}

impl Default for MetadataRefreshJobConfig {
    fn default() -> Self {
        Self {
            provider: String::new(),
            scope: RefreshScope::SeriesOnly,
            field_groups: vec![
                "ratings".to_string(),
                "status".to_string(),
                "counts".to_string(),
            ],
            extra_fields: Vec::new(),
            book_field_groups: Vec::new(),
            book_extra_fields: Vec::new(),
            existing_source_ids_only: true,
            skip_recently_synced_within_s: DEFAULT_SKIP_RECENTLY_SYNCED_SECS,
            max_concurrency: DEFAULT_MAX_CONCURRENCY,
        }
    }
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

/// Decode a stored config JSON string. The `type` discriminator on the row
/// is used to verify the payload matches.
pub fn parse_job_config(
    row_type: &str,
    config_json: &str,
) -> Result<LibraryJobConfig, anyhow::Error> {
    // We require the JSON to carry a `type` field that matches the row's
    // discriminator. This is belt-and-suspenders: if a future migration
    // re-types a row, we won't silently parse it as the wrong variant.
    let value: serde_json::Value = serde_json::from_str(config_json)?;
    let mut obj = value
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("library_jobs.config must be a JSON object"))?;
    if let Some(t) = obj.get("type").and_then(|v| v.as_str()) {
        if t != row_type {
            anyhow::bail!("library_jobs.type='{row_type}' but config carries type='{t}'");
        }
    } else {
        // Inject the discriminator from the row so the enum's `type`
        // tag still resolves. This is safe because we just verified
        // the row type is the source of truth.
        obj.insert(
            "type".to_string(),
            serde_json::Value::String(row_type.to_string()),
        );
    }
    let with_type = serde_json::Value::Object(obj);
    let cfg: LibraryJobConfig = serde_json::from_value(with_type)?;
    Ok(cfg)
}

/// Encode a [`LibraryJobConfig`] for storage. The `type` tag is included
/// so reads via [`parse_job_config`] cross-check correctly.
#[cfg(test)]
pub fn serialize_job_config(cfg: &LibraryJobConfig) -> Result<String, anyhow::Error> {
    Ok(serde_json::to_string(cfg)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_metadata_refresh() {
        let cfg = LibraryJobConfig::MetadataRefresh(MetadataRefreshJobConfig {
            provider: "plugin:mangabaka".to_string(),
            scope: RefreshScope::SeriesOnly,
            field_groups: vec!["ratings".to_string()],
            extra_fields: vec!["language".to_string()],
            book_field_groups: Vec::new(),
            book_extra_fields: Vec::new(),
            existing_source_ids_only: true,
            skip_recently_synced_within_s: 1800,
            max_concurrency: 8,
        });

        let json = serialize_job_config(&cfg).unwrap();
        assert!(json.contains("\"type\":\"metadata_refresh\""));
        assert!(json.contains("\"scope\":\"series_only\""));

        let parsed = parse_job_config("metadata_refresh", &json).unwrap();
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn parse_injects_missing_type_from_row() {
        // Without a type tag, the parser uses the row's type. Useful for
        // older rows or PATCHes where the body omits the redundant tag.
        let json = r#"{"provider":"plugin:x","scope":"series_only","field_groups":[]}"#;
        let parsed = parse_job_config("metadata_refresh", json).unwrap();
        match parsed {
            LibraryJobConfig::MetadataRefresh(c) => {
                assert_eq!(c.provider, "plugin:x");
            }
        }
    }

    #[test]
    fn parse_rejects_type_mismatch() {
        let json = r#"{"type":"scan","provider":"plugin:x"}"#;
        let err = parse_job_config("metadata_refresh", json).unwrap_err();
        assert!(err.to_string().contains("library_jobs.type"));
    }

    #[test]
    fn default_metadata_config_is_safe() {
        let cfg = MetadataRefreshJobConfig::default();
        assert_eq!(cfg.scope, RefreshScope::SeriesOnly);
        assert!(cfg.existing_source_ids_only);
        assert_eq!(cfg.max_concurrency, 4);
        assert_eq!(cfg.skip_recently_synced_within_s, 3600);
        assert_eq!(
            cfg.field_groups,
            vec![
                "ratings".to_string(),
                "status".to_string(),
                "counts".to_string()
            ]
        );
    }

    #[test]
    fn refresh_scope_helpers() {
        assert!(RefreshScope::SeriesOnly.writes_series());
        assert!(!RefreshScope::SeriesOnly.writes_books());
        assert!(!RefreshScope::BooksOnly.writes_series());
        assert!(RefreshScope::BooksOnly.writes_books());
        assert!(RefreshScope::SeriesAndBooks.writes_series());
        assert!(RefreshScope::SeriesAndBooks.writes_books());
    }

    #[test]
    fn library_job_type_round_trips() {
        let v = LibraryJobType::MetadataRefresh;
        assert_eq!(LibraryJobType::parse(v.as_str()), Some(v));
        assert!(LibraryJobType::parse("nope").is_none());
    }
}
