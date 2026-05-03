//! Planner that decides which `(series, provider)` pairs the scheduled
//! metadata refresh should touch in a given run.
//!
//! The planner is intentionally side-effect free: it queries series and
//! external-id state and returns a deterministic plan. The task handler is
//! responsible for actually fetching from plugins and applying metadata.
//!
//! ## Filters
//!
//! - **Provider resolution**: config stores `"plugin:<name>"` strings. Only
//!   providers that resolve to an enabled plugin contribute to the plan;
//!   missing/disabled providers are recorded as plan-level skips.
//! - **`existing_source_ids_only`**: skip series with no
//!   `series_external_ids` row for the resolved provider.
//! - **`skip_recently_synced_within_s`**: skip series whose
//!   `last_synced_at` for the provider is younger than the cutoff.
//!
//! Phase 4 will add an explicit `MatchingStrategy` enum that callers (manual
//! API vs scheduled task) can override; for Phase 2 the planner uses the
//! library's `existing_source_ids_only` toggle directly.

#![allow(dead_code)]

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use sea_orm::DatabaseConnection;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::db::entities::plugins::Model as Plugin;
use crate::db::entities::series_external_ids::{self, Model as SeriesExternalId};
use crate::db::repositories::{PluginsRepository, SeriesExternalIdRepository, SeriesRepository};

use super::refresh_config::MetadataRefreshConfig;

/// Reason a series was skipped during planning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// Provider config references a plugin that isn't installed/enabled.
    ProviderUnavailable { provider: String },
    /// `existing_source_ids_only = true` and series has no external ID for the provider.
    NoExternalId,
    /// `last_synced_at` is younger than `skip_recently_synced_within_s`.
    RecentlySynced { last_synced_at: DateTime<Utc> },
}

impl SkipReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            SkipReason::ProviderUnavailable { .. } => "provider_unavailable",
            SkipReason::NoExternalId => "no_external_id",
            SkipReason::RecentlySynced { .. } => "recently_synced",
        }
    }
}

/// One planned `(series_id, plugin)` pair plus the optional pre-fetched
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
    pub provider: String,
    pub reason: SkipReason,
}

/// Output of [`RefreshPlanner::plan`].
#[derive(Debug, Default)]
pub struct RefreshPlan {
    /// Refreshes that should actually run.
    pub planned: Vec<PlannedRefresh>,
    /// Per-`(series, provider)` skips with reasons.
    pub skipped: Vec<SkippedRefresh>,
    /// Provider strings from the config that don't resolve to an enabled
    /// plugin. Recorded once (not per series). Useful for warning the user
    /// in the task summary.
    pub unresolved_providers: Vec<String>,
}

impl RefreshPlan {
    /// Total work units = number of planned `(series, plugin)` invocations.
    pub fn total_work(&self) -> usize {
        self.planned.len()
    }

    /// Skip count grouped by reason key. Used by the task handler to surface
    /// a structured summary (e.g. "skipped: 12 no_external_id, 3 recently_synced").
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
    /// 1. Resolves each `plugin:<name>` provider string to an enabled plugin
    ///    via [`PluginsRepository::get_by_name`]. Missing or disabled
    ///    providers go into `unresolved_providers`.
    /// 2. Lists every series in the library.
    /// 3. Fetches all external IDs for those series in one query
    ///    (`get_for_series_ids`).
    /// 4. For each `(series, resolved_provider)` pair, decides:
    ///    - `existing_source_ids_only` ⇒ skip when no external ID for that
    ///      provider exists.
    ///    - `skip_recently_synced_within_s` ⇒ skip when the provider's
    ///      `last_synced_at` is too recent.
    ///    - otherwise plan the refresh.
    pub async fn plan(
        db: &DatabaseConnection,
        library_id: Uuid,
        config: &MetadataRefreshConfig,
    ) -> Result<RefreshPlan> {
        let mut plan = RefreshPlan::default();

        if config.providers.is_empty() {
            return Ok(plan);
        }

        // 1. Resolve providers — convert `"plugin:<name>"` strings to
        //    `Plugin` models, recording unresolved entries.
        let mut resolved_providers: Vec<(String, Plugin)> = Vec::new();
        for provider in &config.providers {
            match resolve_provider(db, provider).await {
                Ok(Some(plugin)) => resolved_providers.push((provider.clone(), plugin)),
                Ok(None) => plan.unresolved_providers.push(provider.clone()),
                Err(e) => {
                    return Err(e.context(format!("Failed to resolve provider '{}'", provider)));
                }
            }
        }

        if resolved_providers.is_empty() {
            return Ok(plan);
        }

        // 2. List series in the library.
        let series_list = SeriesRepository::list_by_library(db, library_id)
            .await
            .context("Failed to list series for refresh planning")?;
        if series_list.is_empty() {
            return Ok(plan);
        }
        let series_ids: Vec<Uuid> = series_list.iter().map(|s| s.id).collect();

        // 3. Fetch all external IDs in a single batched query.
        let external_ids_by_series: HashMap<Uuid, Vec<SeriesExternalId>> =
            SeriesExternalIdRepository::get_for_series_ids(db, &series_ids)
                .await
                .context("Failed to load external IDs for refresh planning")?;

        // Pre-compute the recency cutoff once.
        let recently_synced_cutoff: Option<DateTime<Utc>> = if config.skip_recently_synced_within_s
            == 0
        {
            None
        } else {
            Some(Utc::now() - Duration::seconds(i64::from(config.skip_recently_synced_within_s)))
        };

        // 4. For each (series, provider) pair, decide.
        for series in &series_ids {
            let series_externals = external_ids_by_series.get(series);
            for (provider_str, plugin) in &resolved_providers {
                let plugin_source = series_external_ids::Model::plugin_source(&plugin.name);
                let existing = series_externals
                    .and_then(|list| list.iter().find(|e| e.source == plugin_source).cloned());

                if config.existing_source_ids_only && existing.is_none() {
                    plan.skipped.push(SkippedRefresh {
                        series_id: *series,
                        provider: provider_str.clone(),
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
                        provider: provider_str.clone(),
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
        }

        Ok(plan)
    }
}

/// Resolve a `"plugin:<name>"` string to an enabled plugin.
///
/// Returns:
/// - `Ok(Some(plugin))` if the string parses and an enabled plugin exists.
/// - `Ok(None)` if the prefix is missing, the plugin doesn't exist, or the
///   plugin exists but is disabled. Caller records this as a plan-level
///   `unresolved_providers` entry.
/// - `Err(_)` only on DB errors.
async fn resolve_provider(db: &DatabaseConnection, provider: &str) -> Result<Option<Plugin>> {
    let Some(name) = provider.strip_prefix("plugin:") else {
        return Ok(None);
    };
    let plugin = PluginsRepository::get_by_name(db, name).await?;
    Ok(plugin.filter(|p| p.enabled))
}

/// Convenience: dedup + flatten the field allowlist that the task handler
/// will pass to `MetadataApplier::apply` via `ApplyOptions::fields_filter`.
///
/// Group names in `config.field_groups` are expanded via
/// [`crate::services::metadata::field_groups::fields_for_groups`] into the
/// concrete camelCase field names the applier checks. `config.extra_fields`
/// is unioned in verbatim (power-user hatch).
///
/// Returns `None` when both lists are empty (apply everything; existing
/// `MetadataApplier` semantics). Unknown group names are silently skipped —
/// the API layer is responsible for validating user input up front.
pub fn fields_filter_from_config(config: &MetadataRefreshConfig) -> Option<HashSet<String>> {
    super::field_groups::fields_for_groups(&config.field_groups, &config.extra_fields)
}

/// Per-provider variant of [`fields_filter_from_config`].
///
/// When `config.per_provider_overrides` contains an entry for `provider`, that
/// override's `field_groups` + `extra_fields` are used in place of the
/// library-wide selection. Without an override, the result is identical to
/// [`fields_filter_from_config`].
///
/// `provider` is the wire-format string (e.g. `"plugin:mangabaka"`), matching
/// the keys used in the per-library config and the planner's
/// `unresolved_providers` list.
pub fn fields_filter_for_provider(
    config: &MetadataRefreshConfig,
    provider: &str,
) -> Option<HashSet<String>> {
    if let Some(overrides) = config.per_provider_overrides.as_ref()
        && let Some(ovr) = overrides.get(provider)
    {
        return super::field_groups::fields_for_groups(&ovr.field_groups, &ovr.extra_fields);
    }
    fields_filter_from_config(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::entities::plugins::PluginPermission;
    use crate::db::repositories::{LibraryRepository, PluginsRepository, SeriesRepository};
    use crate::db::test_helpers::setup_test_db;
    use crate::services::metadata::refresh_config::MetadataRefreshConfig;
    use crate::services::plugin::protocol::PluginScope;
    use std::env;
    use std::sync::Once;

    static INIT_ENCRYPTION: Once = Once::new();

    fn setup_test_encryption_key() {
        INIT_ENCRYPTION.call_once(|| {
            if env::var("CODEX_ENCRYPTION_KEY").is_err() {
                // SAFETY: tests run with shared env access; first writer wins.
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
        let lib = LibraryRepository::create(
            db,
            name,
            &format!("/tmp/{}", name),
            ScanningStrategy::Default,
        )
        .await
        .unwrap();
        lib.id
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

    fn config_with_provider(provider: &str) -> MetadataRefreshConfig {
        MetadataRefreshConfig {
            providers: vec![provider.to_string()],
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn plan_is_empty_when_no_providers_configured() {
        let db = setup_test_db().await;
        let library_id = create_library(&db, "lib").await;
        let _series_id = create_series(&db, library_id, "Series A").await;

        let cfg = MetadataRefreshConfig::default();
        let plan = RefreshPlanner::plan(&db, library_id, &cfg).await.unwrap();

        assert!(plan.planned.is_empty());
        assert!(plan.skipped.is_empty());
        assert!(plan.unresolved_providers.is_empty());
    }

    #[tokio::test]
    async fn plan_records_unresolved_provider_for_missing_plugin() {
        let db = setup_test_db().await;
        let library_id = create_library(&db, "lib").await;
        let _series = create_series(&db, library_id, "Series A").await;

        let cfg = config_with_provider("plugin:does-not-exist");
        let plan = RefreshPlanner::plan(&db, library_id, &cfg).await.unwrap();

        assert!(plan.planned.is_empty());
        assert!(plan.skipped.is_empty());
        assert_eq!(plan.unresolved_providers, vec!["plugin:does-not-exist"]);
    }

    #[tokio::test]
    async fn plan_skips_disabled_plugin() {
        let db = setup_test_db().await;
        let library_id = create_library(&db, "lib").await;
        let _series = create_series(&db, library_id, "Series A").await;
        let _plugin = create_plugin(&db, "mangabaka", false).await;

        let cfg = config_with_provider("plugin:mangabaka");
        let plan = RefreshPlanner::plan(&db, library_id, &cfg).await.unwrap();

        assert!(plan.planned.is_empty());
        assert_eq!(plan.unresolved_providers, vec!["plugin:mangabaka"]);
    }

    #[tokio::test]
    async fn plan_strict_mode_skips_series_without_external_id() {
        let db = setup_test_db().await;
        let library_id = create_library(&db, "lib").await;
        let series_id = create_series(&db, library_id, "Series A").await;
        let _plugin = create_plugin(&db, "mangabaka", true).await;

        let cfg = MetadataRefreshConfig {
            providers: vec!["plugin:mangabaka".to_string()],
            existing_source_ids_only: true,
            skip_recently_synced_within_s: 0,
            ..Default::default()
        };
        let plan = RefreshPlanner::plan(&db, library_id, &cfg).await.unwrap();

        assert!(plan.planned.is_empty());
        assert_eq!(plan.skipped.len(), 1);
        assert_eq!(plan.skipped[0].series_id, series_id);
        assert_eq!(plan.skipped[0].reason, SkipReason::NoExternalId);
    }

    #[tokio::test]
    async fn plan_strict_mode_includes_series_with_external_id() {
        let db = setup_test_db().await;
        let library_id = create_library(&db, "lib").await;
        let series_id = create_series(&db, library_id, "Series A").await;
        let _plugin = create_plugin(&db, "mangabaka", true).await;

        // Seed an external ID for plugin:mangabaka
        SeriesExternalIdRepository::upsert_for_plugin(
            &db,
            series_id,
            "mangabaka",
            "ext-1",
            None,
            None,
        )
        .await
        .unwrap();

        let cfg = MetadataRefreshConfig {
            providers: vec!["plugin:mangabaka".to_string()],
            existing_source_ids_only: true,
            skip_recently_synced_within_s: 0,
            ..Default::default()
        };
        let plan = RefreshPlanner::plan(&db, library_id, &cfg).await.unwrap();

        assert_eq!(plan.planned.len(), 1);
        assert_eq!(plan.planned[0].series_id, series_id);
        assert_eq!(plan.planned[0].plugin.name, "mangabaka");
        assert!(plan.planned[0].existing_external_id.is_some());
        assert!(plan.skipped.is_empty());
    }

    #[tokio::test]
    async fn plan_loose_mode_includes_unmatched_series() {
        let db = setup_test_db().await;
        let library_id = create_library(&db, "lib").await;
        let series_id = create_series(&db, library_id, "Series A").await;
        let _plugin = create_plugin(&db, "mangabaka", true).await;

        let cfg = MetadataRefreshConfig {
            providers: vec!["plugin:mangabaka".to_string()],
            existing_source_ids_only: false,
            skip_recently_synced_within_s: 0,
            ..Default::default()
        };
        let plan = RefreshPlanner::plan(&db, library_id, &cfg).await.unwrap();

        assert_eq!(plan.planned.len(), 1);
        assert_eq!(plan.planned[0].series_id, series_id);
        assert!(plan.planned[0].existing_external_id.is_none());
    }

    #[tokio::test]
    async fn plan_skips_recently_synced_series() {
        let db = setup_test_db().await;
        let library_id = create_library(&db, "lib").await;
        let series_id = create_series(&db, library_id, "Series A").await;
        let _plugin = create_plugin(&db, "mangabaka", true).await;

        // Seeded `last_synced_at` defaults to "now", which is < cutoff
        SeriesExternalIdRepository::upsert_for_plugin(
            &db,
            series_id,
            "mangabaka",
            "ext-1",
            None,
            None,
        )
        .await
        .unwrap();

        let cfg = MetadataRefreshConfig {
            providers: vec!["plugin:mangabaka".to_string()],
            existing_source_ids_only: true,
            skip_recently_synced_within_s: 3600, // 1h
            ..Default::default()
        };
        let plan = RefreshPlanner::plan(&db, library_id, &cfg).await.unwrap();

        assert!(plan.planned.is_empty());
        assert_eq!(plan.skipped.len(), 1);
        assert!(matches!(
            plan.skipped[0].reason,
            SkipReason::RecentlySynced { .. }
        ));
    }

    #[tokio::test]
    async fn plan_includes_when_recency_guard_disabled() {
        let db = setup_test_db().await;
        let library_id = create_library(&db, "lib").await;
        let series_id = create_series(&db, library_id, "Series A").await;
        let _plugin = create_plugin(&db, "mangabaka", true).await;

        SeriesExternalIdRepository::upsert_for_plugin(
            &db,
            series_id,
            "mangabaka",
            "ext-1",
            None,
            None,
        )
        .await
        .unwrap();

        let cfg = MetadataRefreshConfig {
            providers: vec!["plugin:mangabaka".to_string()],
            existing_source_ids_only: true,
            skip_recently_synced_within_s: 0,
            ..Default::default()
        };
        let plan = RefreshPlanner::plan(&db, library_id, &cfg).await.unwrap();

        assert_eq!(plan.planned.len(), 1);
        assert!(plan.skipped.is_empty());
    }

    #[tokio::test]
    async fn plan_handles_multiple_providers_independently() {
        let db = setup_test_db().await;
        let library_id = create_library(&db, "lib").await;
        let series_id = create_series(&db, library_id, "Series A").await;
        let _p1 = create_plugin(&db, "mangabaka", true).await;
        let _p2 = create_plugin(&db, "anilist", true).await;

        // Series matched on mangabaka but not anilist
        SeriesExternalIdRepository::upsert_for_plugin(
            &db,
            series_id,
            "mangabaka",
            "ext-1",
            None,
            None,
        )
        .await
        .unwrap();

        let cfg = MetadataRefreshConfig {
            providers: vec!["plugin:mangabaka".to_string(), "plugin:anilist".to_string()],
            existing_source_ids_only: true,
            skip_recently_synced_within_s: 0,
            ..Default::default()
        };
        let plan = RefreshPlanner::plan(&db, library_id, &cfg).await.unwrap();

        assert_eq!(plan.planned.len(), 1);
        assert_eq!(plan.planned[0].plugin.name, "mangabaka");
        assert_eq!(plan.skipped.len(), 1);
        assert_eq!(plan.skipped[0].provider, "plugin:anilist");
        assert_eq!(plan.skipped[0].reason, SkipReason::NoExternalId);

        let counts = plan.skipped_by_reason();
        assert_eq!(counts.get("no_external_id").copied(), Some(1));
    }

    #[tokio::test]
    async fn plan_empty_library_returns_empty_plan() {
        let db = setup_test_db().await;
        let library_id = create_library(&db, "lib").await;
        let _plugin = create_plugin(&db, "mangabaka", true).await;

        let cfg = config_with_provider("plugin:mangabaka");
        let plan = RefreshPlanner::plan(&db, library_id, &cfg).await.unwrap();

        assert!(plan.planned.is_empty());
        assert!(plan.skipped.is_empty());
        assert!(plan.unresolved_providers.is_empty());
    }

    #[test]
    fn fields_filter_returns_none_when_no_groups_or_extras() {
        let cfg = MetadataRefreshConfig {
            field_groups: vec![],
            extra_fields: vec![],
            ..Default::default()
        };
        assert!(fields_filter_from_config(&cfg).is_none());
    }

    #[test]
    fn fields_filter_expands_groups_to_concrete_fields() {
        // "ratings" → ["rating", "externalRatings"]
        // "status"  → ["status", "year"]
        // extras    → ["language"]
        let cfg = MetadataRefreshConfig {
            field_groups: vec!["ratings".to_string(), "status".to_string()],
            extra_fields: vec!["language".to_string()],
            ..Default::default()
        };
        let filter = fields_filter_from_config(&cfg).unwrap();
        assert!(filter.contains("rating"));
        assert!(filter.contains("externalRatings"));
        assert!(filter.contains("status"));
        assert!(filter.contains("year"));
        assert!(filter.contains("language"));
        assert_eq!(filter.len(), 5);
    }

    #[test]
    fn fields_filter_extras_only_passes_through() {
        let cfg = MetadataRefreshConfig {
            field_groups: vec![],
            extra_fields: vec!["title".to_string(), "summary".to_string()],
            ..Default::default()
        };
        let filter = fields_filter_from_config(&cfg).unwrap();
        assert!(filter.contains("title"));
        assert!(filter.contains("summary"));
        assert_eq!(filter.len(), 2);
    }

    #[test]
    fn fields_filter_silently_drops_unknown_groups() {
        let cfg = MetadataRefreshConfig {
            field_groups: vec!["ratings".to_string(), "made_up_group".to_string()],
            extra_fields: vec![],
            ..Default::default()
        };
        let filter = fields_filter_from_config(&cfg).unwrap();
        assert!(filter.contains("rating"));
        assert!(filter.contains("externalRatings"));
        assert_eq!(filter.len(), 2);
    }

    #[test]
    fn fields_filter_for_provider_uses_library_default_when_no_override() {
        let cfg = MetadataRefreshConfig {
            field_groups: vec!["ratings".to_string()],
            extra_fields: vec![],
            ..Default::default()
        };
        let filter = fields_filter_for_provider(&cfg, "plugin:mangabaka").unwrap();
        assert!(filter.contains("rating"));
        assert!(filter.contains("externalRatings"));
        assert_eq!(filter.len(), 2);
    }

    #[test]
    fn fields_filter_for_provider_uses_override_when_set() {
        use crate::services::metadata::ProviderOverride;
        use std::collections::BTreeMap;

        let mut overrides = BTreeMap::new();
        overrides.insert(
            "plugin:anilist".to_string(),
            ProviderOverride {
                field_groups: vec!["ratings".to_string()],
                extra_fields: vec![],
            },
        );
        let cfg = MetadataRefreshConfig {
            // Library default is "status" (so default ⇒ status, year)
            field_groups: vec!["status".to_string()],
            extra_fields: vec![],
            per_provider_overrides: Some(overrides),
            ..Default::default()
        };

        // anilist override → ratings only
        let anilist = fields_filter_for_provider(&cfg, "plugin:anilist").unwrap();
        assert!(anilist.contains("rating"));
        assert!(anilist.contains("externalRatings"));
        assert!(!anilist.contains("status"));
        assert_eq!(anilist.len(), 2);

        // mangabaka has no override → falls back to library default (status)
        let mangabaka = fields_filter_for_provider(&cfg, "plugin:mangabaka").unwrap();
        assert!(mangabaka.contains("status"));
        assert!(mangabaka.contains("year"));
        assert!(!mangabaka.contains("rating"));
        assert_eq!(mangabaka.len(), 2);
    }

    #[test]
    fn fields_filter_for_provider_override_with_only_extras() {
        use crate::services::metadata::ProviderOverride;
        use std::collections::BTreeMap;

        let mut overrides = BTreeMap::new();
        overrides.insert(
            "plugin:custom".to_string(),
            ProviderOverride {
                field_groups: vec![],
                extra_fields: vec!["coverUrl".to_string()],
            },
        );
        let cfg = MetadataRefreshConfig {
            field_groups: vec!["ratings".to_string()],
            extra_fields: vec![],
            per_provider_overrides: Some(overrides),
            ..Default::default()
        };

        let filter = fields_filter_for_provider(&cfg, "plugin:custom").unwrap();
        assert!(filter.contains("coverUrl"));
        assert!(!filter.contains("rating"));
        assert_eq!(filter.len(), 1);
    }

    #[test]
    fn fields_filter_for_provider_empty_override_returns_none() {
        use crate::services::metadata::ProviderOverride;
        use std::collections::BTreeMap;

        let mut overrides = BTreeMap::new();
        overrides.insert(
            "plugin:custom".to_string(),
            ProviderOverride {
                field_groups: vec![],
                extra_fields: vec![],
            },
        );
        let cfg = MetadataRefreshConfig {
            field_groups: vec!["ratings".to_string()],
            extra_fields: vec![],
            per_provider_overrides: Some(overrides),
            ..Default::default()
        };

        // An empty override is interpreted as "apply everything" — explicit
        // user intent to bypass the library's restriction for this provider.
        let filter = fields_filter_for_provider(&cfg, "plugin:custom");
        assert!(filter.is_none());
    }
}
