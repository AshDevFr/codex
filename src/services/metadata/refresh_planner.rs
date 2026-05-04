//! Planner that decides which `(series, provider)` pairs the scheduled
//! metadata refresh should touch in a given run.
//!
//! Phase 9: each job carries a single provider, so the planner now resolves
//! one `"plugin:<name>"` reference, lists the library's series, and emits one
//! `PlannedRefresh` per series (or skipped reason). The previous
//! many-providers-per-config model has been removed alongside the per-provider
//! override hatch.

#![allow(dead_code)]

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use sea_orm::DatabaseConnection;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::db::entities::plugins::Model as Plugin;
use crate::db::entities::series_external_ids::{self, Model as SeriesExternalId};
use crate::db::repositories::{PluginsRepository, SeriesExternalIdRepository, SeriesRepository};

use crate::services::library_jobs::MetadataRefreshJobConfig;

/// Reason a series was skipped during planning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// `existing_source_ids_only = true` and series has no external ID for the provider.
    NoExternalId,
    /// `last_synced_at` is younger than `skip_recently_synced_within_s`.
    RecentlySynced { last_synced_at: DateTime<Utc> },
}

impl SkipReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            SkipReason::NoExternalId => "no_external_id",
            SkipReason::RecentlySynced { .. } => "recently_synced",
        }
    }
}

/// One planned `(series, plugin)` pair plus the optional pre-fetched
/// external ID. Carrying the external ID through avoids a second DB lookup
/// in the task handler when it dispatches `metadata/series/get`.
#[derive(Debug, Clone)]
pub struct PlannedRefresh {
    pub series_id: Uuid,
    pub plugin: Plugin,
    /// Pre-fetched external ID for this series + plugin, if any.
    pub existing_external_id: Option<SeriesExternalId>,
}

/// One series that was considered but skipped, with the reason. Surfaced so
/// the task handler can record per-reason counts in the task summary.
#[derive(Debug, Clone)]
pub struct SkippedRefresh {
    pub series_id: Uuid,
    pub reason: SkipReason,
}

/// Reason the entire plan resolved to "no work" before the per-series gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanFailure {
    /// Provider string isn't `"plugin:<name>"`.
    InvalidProviderString,
    /// Plugin name doesn't resolve to an installed plugin.
    PluginMissing,
    /// Plugin exists but is disabled.
    PluginDisabled,
}

impl PlanFailure {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlanFailure::InvalidProviderString => "invalid_provider_string",
            PlanFailure::PluginMissing => "plugin_missing",
            PlanFailure::PluginDisabled => "plugin_disabled",
        }
    }
}

/// Output of [`RefreshPlanner::plan`].
#[derive(Debug, Default)]
pub struct RefreshPlan {
    /// The plugin model the planner resolved against. `None` when the
    /// provider couldn't be resolved (see [`Self::failure`]).
    pub plugin: Option<Plugin>,
    /// Refreshes that should actually run.
    pub planned: Vec<PlannedRefresh>,
    /// Per-series skips with reasons.
    pub skipped: Vec<SkippedRefresh>,
    /// Set when provider resolution failed before the per-series step.
    /// Mutually exclusive with `planned`.
    pub failure: Option<PlanFailure>,
}

impl RefreshPlan {
    /// Total work units = number of planned `(series, plugin)` invocations.
    pub fn total_work(&self) -> usize {
        self.planned.len()
    }

    /// Skip count grouped by reason key.
    pub fn skipped_by_reason(&self) -> HashMap<&'static str, usize> {
        let mut out: HashMap<&'static str, usize> = HashMap::new();
        for s in &self.skipped {
            *out.entry(s.reason.as_str()).or_insert(0) += 1;
        }
        out
    }
}

/// Stateless planner. All state is passed in per-call.
pub struct RefreshPlanner;

impl RefreshPlanner {
    /// Build a refresh plan for `library_id` against `config`.
    ///
    /// The planner:
    /// 1. Resolves `config.provider`. Failure is recorded on `plan.failure`.
    /// 2. Lists every series in the library.
    /// 3. Fetches all external IDs for those series in one query.
    /// 4. For each `(series, plugin)` pair, emits a `PlannedRefresh` or a
    ///    typed [`SkipReason`].
    pub async fn plan(
        db: &DatabaseConnection,
        library_id: Uuid,
        config: &MetadataRefreshJobConfig,
    ) -> Result<RefreshPlan> {
        let mut plan = RefreshPlan::default();

        // 1. Resolve provider.
        let plugin = match resolve_provider(db, &config.provider).await? {
            ProviderResolution::Resolved(p) => p,
            ProviderResolution::InvalidString => {
                plan.failure = Some(PlanFailure::InvalidProviderString);
                return Ok(plan);
            }
            ProviderResolution::Missing => {
                plan.failure = Some(PlanFailure::PluginMissing);
                return Ok(plan);
            }
            ProviderResolution::Disabled => {
                plan.failure = Some(PlanFailure::PluginDisabled);
                return Ok(plan);
            }
        };

        // 2. List series.
        let series_list = SeriesRepository::list_by_library(db, library_id)
            .await
            .context("Failed to list series for refresh planning")?;
        if series_list.is_empty() {
            plan.plugin = Some(plugin);
            return Ok(plan);
        }
        let series_ids: Vec<Uuid> = series_list.iter().map(|s| s.id).collect();

        // 3. Fetch all external IDs in a single batched query.
        let external_ids_by_series: HashMap<Uuid, Vec<SeriesExternalId>> =
            SeriesExternalIdRepository::get_for_series_ids(db, &series_ids)
                .await
                .context("Failed to load external IDs for refresh planning")?;

        let recently_synced_cutoff: Option<DateTime<Utc>> = if config.skip_recently_synced_within_s
            == 0
        {
            None
        } else {
            Some(Utc::now() - Duration::seconds(i64::from(config.skip_recently_synced_within_s)))
        };

        let plugin_source = series_external_ids::Model::plugin_source(&plugin.name);

        // 4. For each series, decide.
        for series in &series_ids {
            let series_externals = external_ids_by_series.get(series);
            let existing = series_externals
                .and_then(|list| list.iter().find(|e| e.source == plugin_source).cloned());

            if config.existing_source_ids_only && existing.is_none() {
                plan.skipped.push(SkippedRefresh {
                    series_id: *series,
                    reason: SkipReason::NoExternalId,
                });
                continue;
            }

            if let (Some(cutoff), Some(ext)) = (recently_synced_cutoff, existing.as_ref())
                && let Some(last_synced_at) = ext.last_synced_at
                && last_synced_at >= cutoff
            {
                plan.skipped.push(SkippedRefresh {
                    series_id: *series,
                    reason: SkipReason::RecentlySynced { last_synced_at },
                });
                continue;
            }

            plan.planned.push(PlannedRefresh {
                series_id: *series,
                plugin: plugin.clone(),
                existing_external_id: existing,
            });
        }

        plan.plugin = Some(plugin);
        Ok(plan)
    }
}

/// Outcome of resolving the job's `provider` string.
#[allow(clippy::large_enum_variant)]
enum ProviderResolution {
    Resolved(Plugin),
    InvalidString,
    Missing,
    Disabled,
}

async fn resolve_provider(db: &DatabaseConnection, provider: &str) -> Result<ProviderResolution> {
    let Some(name) = provider.strip_prefix("plugin:").filter(|s| !s.is_empty()) else {
        return Ok(ProviderResolution::InvalidString);
    };
    let plugin = PluginsRepository::get_by_name(db, name).await?;
    match plugin {
        None => Ok(ProviderResolution::Missing),
        Some(p) if !p.enabled => Ok(ProviderResolution::Disabled),
        Some(p) => Ok(ProviderResolution::Resolved(p)),
    }
}

/// Expand `field_groups + extra_fields` into the concrete camelCase field
/// set the applier understands. Returns `None` when both lists are empty
/// (apply everything; existing `MetadataApplier` semantics).
pub fn fields_filter_from_job_config(config: &MetadataRefreshJobConfig) -> Option<HashSet<String>> {
    super::field_groups::fields_for_groups(&config.field_groups, &config.extra_fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::entities::plugins::PluginPermission;
    use crate::db::repositories::{LibraryRepository, PluginsRepository, SeriesRepository};
    use crate::db::test_helpers::setup_test_db;
    use crate::services::library_jobs::{MetadataRefreshJobConfig, RefreshScope};
    use crate::services::plugin::protocol::PluginScope;
    use std::env;
    use std::sync::Once;

    static INIT_ENCRYPTION: Once = Once::new();

    fn setup_test_encryption_key() {
        INIT_ENCRYPTION.call_once(|| {
            if env::var("CODEX_ENCRYPTION_KEY").is_err() {
                // SAFETY: tests share env; first-writer-wins is safe with a constant.
                unsafe {
                    env::set_var(
                        "CODEX_ENCRYPTION_KEY",
                        "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=",
                    );
                }
            }
        });
    }

    async fn create_library(db: &DatabaseConnection, name: &str) -> Uuid {
        LibraryRepository::create(db, name, &format!("/tmp/{name}"), ScanningStrategy::Default)
            .await
            .unwrap()
            .id
    }

    async fn create_series(db: &DatabaseConnection, library_id: Uuid, name: &str) -> Uuid {
        SeriesRepository::create(db, library_id, name, None)
            .await
            .unwrap()
            .id
    }

    async fn create_plugin(db: &DatabaseConnection, name: &str, enabled: bool) -> Plugin {
        setup_test_encryption_key();
        PluginsRepository::create(
            db,
            name,
            name,
            None,
            "system",
            "node",
            vec!["dist/index.js".to_string()],
            vec![],
            None,
            vec![PluginPermission::MetadataWriteSummary],
            vec![PluginScope::SeriesDetail],
            vec![],
            None,
            "env",
            None,
            enabled,
            None,
            None,
        )
        .await
        .unwrap()
    }

    fn cfg(provider: &str) -> MetadataRefreshJobConfig {
        MetadataRefreshJobConfig {
            provider: provider.to_string(),
            scope: RefreshScope::SeriesOnly,
            field_groups: vec![],
            extra_fields: vec![],
            book_field_groups: vec![],
            book_extra_fields: vec![],
            existing_source_ids_only: true,
            skip_recently_synced_within_s: 0,
            max_concurrency: 4,
        }
    }

    #[tokio::test]
    async fn plan_invalid_provider_string() {
        let db = setup_test_db().await;
        let lib = create_library(&db, "lib").await;
        let plan = RefreshPlanner::plan(&db, lib, &cfg("not-a-plugin"))
            .await
            .unwrap();
        assert!(matches!(
            plan.failure,
            Some(PlanFailure::InvalidProviderString)
        ));
        assert!(plan.planned.is_empty());
    }

    #[tokio::test]
    async fn plan_missing_plugin() {
        let db = setup_test_db().await;
        let lib = create_library(&db, "lib").await;
        let plan = RefreshPlanner::plan(&db, lib, &cfg("plugin:missing"))
            .await
            .unwrap();
        assert!(matches!(plan.failure, Some(PlanFailure::PluginMissing)));
    }

    #[tokio::test]
    async fn plan_disabled_plugin() {
        let db = setup_test_db().await;
        let lib = create_library(&db, "lib").await;
        let _ = create_plugin(&db, "off", false).await;
        let plan = RefreshPlanner::plan(&db, lib, &cfg("plugin:off"))
            .await
            .unwrap();
        assert!(matches!(plan.failure, Some(PlanFailure::PluginDisabled)));
    }

    #[tokio::test]
    async fn plan_strict_mode_skips_no_id() {
        let db = setup_test_db().await;
        let lib = create_library(&db, "lib").await;
        let _ = create_series(&db, lib, "S1").await;
        let _ = create_series(&db, lib, "S2").await;
        let _ = create_plugin(&db, "x", true).await;
        let mut config = cfg("plugin:x");
        config.existing_source_ids_only = true;
        let plan = RefreshPlanner::plan(&db, lib, &config).await.unwrap();
        assert!(plan.failure.is_none());
        assert!(plan.planned.is_empty());
        assert_eq!(plan.skipped.len(), 2);
        assert!(
            plan.skipped
                .iter()
                .all(|s| s.reason == SkipReason::NoExternalId)
        );
    }

    #[tokio::test]
    async fn plan_loose_mode_keeps_no_id_pairs() {
        let db = setup_test_db().await;
        let lib = create_library(&db, "lib").await;
        let _ = create_series(&db, lib, "S1").await;
        let _ = create_plugin(&db, "x", true).await;
        let mut config = cfg("plugin:x");
        config.existing_source_ids_only = false;
        let plan = RefreshPlanner::plan(&db, lib, &config).await.unwrap();
        assert_eq!(plan.planned.len(), 1);
        assert!(plan.skipped.is_empty());
    }

    #[tokio::test]
    async fn fields_filter_returns_none_when_empty() {
        let cfg = MetadataRefreshJobConfig::default();
        // Default has non-empty groups.
        assert!(fields_filter_from_job_config(&cfg).is_some());
        let empty = MetadataRefreshJobConfig {
            field_groups: vec![],
            ..cfg
        };
        assert!(fields_filter_from_job_config(&empty).is_none());
    }

    #[tokio::test]
    async fn fields_filter_expands_groups() {
        let cfg = MetadataRefreshJobConfig {
            field_groups: vec!["ratings".to_string(), "status".to_string()],
            extra_fields: vec!["language".to_string()],
            ..MetadataRefreshJobConfig::default()
        };
        let out = fields_filter_from_job_config(&cfg).unwrap();
        assert!(out.contains("rating"));
        assert!(out.contains("externalRatings"));
        assert!(out.contains("status"));
        assert!(out.contains("year"));
        assert!(out.contains("language"));
    }
}
