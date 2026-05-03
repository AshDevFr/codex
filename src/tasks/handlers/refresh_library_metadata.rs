//! Scheduled per-library metadata refresh handler.
//!
//! Reads `libraries.metadata_refresh_config`, builds a [`RefreshPlan`] via
//! [`RefreshPlanner`], then walks the plan one `(series, plugin)` pair at a
//! time. Each pair fetches metadata via the plugin's `metadata/series/get`
//! call (using the stored external ID) and applies it through the existing
//! [`MetadataApplier`]. Per-pair errors are isolated: one failure increments
//! a counter and the handler keeps going.
//!
//! ## Matching strategy
//!
//! Driven by [`MetadataRefreshConfig::existing_source_ids_only`]:
//!
//! - `true` ⇒ [`MatchingStrategy::ExistingExternalIdOnly`]: the
//!   [`RefreshPlanner`] gates pairs at planning time, so the handler never
//!   sees a no-external-id pair. Skips count as `no_external_id`.
//! - `false` ⇒ [`MatchingStrategy::AllowReMatch`]: the planner returns
//!   no-external-id pairs and the handler calls `metadata/series/match`
//!   for each one. A successful match is then fetched via
//!   `metadata/series/get` and applied like any other pair. A miss is
//!   recorded as `no_match_candidate` (distinct from `no_external_id`,
//!   which only applies in strict mode).
//!
//! Field-group resolution lives in
//! [`crate::services::metadata::field_groups::fields_for_groups`] (Phase 3).

use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::db::entities::tasks;
use crate::db::repositories::{
    LibraryRepository, PluginsRepository, SeriesExternalIdRepository, SeriesMetadataRepository,
    SeriesRepository,
};
use crate::events::{EntityChangeEvent, EntityEvent, EventBroadcaster, TaskProgressEvent};
use crate::services::ThumbnailService;
use crate::services::metadata::refresh_planner::{
    PlannedRefresh, RefreshPlan, RefreshPlanner, fields_filter_for_provider,
};
use crate::services::metadata::{ApplyOptions, MatchingStrategy, MetadataApplier};
use crate::services::plugin::PluginManager;
use crate::services::plugin::protocol::{MetadataGetParams, MetadataMatchParams};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Soft cap to keep one library's refresh from monopolizing the worker on
/// a misconfigured `max_concurrency`. The user can lower it; the host does
/// not allow exceeding this.
const MAX_CONCURRENCY_HARD_CAP: usize = 16;

/// Per-`(series, provider)` plugin call timeout. Plugin manager already has
/// its own timeouts but we bound the apply step too so a slow plugin can't
/// stall the whole queue.
const PER_PAIR_TIMEOUT: Duration = Duration::from_secs(60);

/// Aggregated outcome of a single library refresh run.
#[derive(Debug, Default)]
struct RunSummary {
    succeeded: u32,
    failed: u32,
    skipped_no_external_id: u32,
    skipped_recently_synced: u32,
    skipped_provider_unavailable: u32,
    /// Loose-mode `match_series` returned no candidate above the
    /// confidence floor, so the pair was skipped. Distinct from
    /// `no_external_id` (which is the strict-mode skip reason).
    skipped_no_match_candidate: u32,
    fields_applied_total: u32,
}

impl RunSummary {
    fn into_json(
        self,
        total_planned: usize,
        unresolved_providers: Vec<String>,
    ) -> serde_json::Value {
        json!({
            "planned": total_planned,
            "succeeded": self.succeeded,
            "failed": self.failed,
            "skipped": {
                "no_external_id": self.skipped_no_external_id,
                "recently_synced": self.skipped_recently_synced,
                "provider_unavailable": self.skipped_provider_unavailable,
                "no_match_candidate": self.skipped_no_match_candidate,
            },
            "fields_applied_total": self.fields_applied_total,
            "unresolved_providers": unresolved_providers,
        })
    }
}

/// Handler for [`crate::tasks::types::TaskType::RefreshLibraryMetadata`].
pub struct RefreshLibraryMetadataHandler {
    plugin_manager: Arc<PluginManager>,
    thumbnail_service: Option<Arc<ThumbnailService>>,
}

impl RefreshLibraryMetadataHandler {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self {
            plugin_manager,
            thumbnail_service: None,
        }
    }

    pub fn with_thumbnail_service(mut self, thumbnail_service: Arc<ThumbnailService>) -> Self {
        self.thumbnail_service = Some(thumbnail_service);
        self
    }

    /// Convert the plan's `skipped` entries into pre-counted
    /// `RunSummary` fields. The planner already does the gating; the
    /// handler just folds the structured reasons into counters.
    fn fold_skipped_into_summary(plan: &RefreshPlan, summary: &mut RunSummary) {
        for s in &plan.skipped {
            match s.reason {
                crate::services::metadata::refresh_planner::SkipReason::NoExternalId => {
                    summary.skipped_no_external_id += 1;
                }
                crate::services::metadata::refresh_planner::SkipReason::RecentlySynced {
                    ..
                } => {
                    summary.skipped_recently_synced += 1;
                }
                crate::services::metadata::refresh_planner::SkipReason::ProviderUnavailable {
                    ..
                } => {
                    summary.skipped_provider_unavailable += 1;
                }
            }
        }
    }
}

impl TaskHandler for RefreshLibraryMetadataHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            let library_id = task
                .library_id
                .ok_or_else(|| anyhow::anyhow!("Missing library_id in task"))?;

            // 1. Load library + config. Treat a missing library as a hard
            //    error (the scheduler shouldn't have fired in that case).
            let library = LibraryRepository::get_by_id(db, library_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Library not found: {}", library_id))?;
            let config = LibraryRepository::get_metadata_refresh_config(db, library_id).await?;

            info!(
                "Task {}: Refreshing library '{}' (id={}) — providers={:?} groups={:?} strict={}",
                task.id,
                library.name,
                library_id,
                config.providers,
                config.field_groups,
                config.existing_source_ids_only
            );

            // 2. Empty providers shortcut.
            if config.providers.is_empty() {
                return Ok(TaskResult::success_with_data(
                    "No providers configured; skipping",
                    RunSummary::default().into_json(0, Vec::new()),
                ));
            }

            // 3. Build the plan.
            let plan = RefreshPlanner::plan(db, library_id, &config)
                .await
                .context("Failed to build refresh plan")?;

            let total_planned = plan.total_work();
            let unresolved_providers = plan.unresolved_providers.clone();
            let mut summary = RunSummary::default();
            Self::fold_skipped_into_summary(&plan, &mut summary);

            if total_planned == 0 {
                let message = if !unresolved_providers.is_empty() {
                    format!(
                        "Nothing to refresh ({} unresolved providers, {} skipped)",
                        unresolved_providers.len(),
                        plan.skipped.len()
                    )
                } else {
                    format!("Nothing to refresh ({} skipped)", plan.skipped.len())
                };
                info!("Task {}: {}", task.id, message);
                return Ok(TaskResult::success_with_data(
                    message,
                    summary.into_json(total_planned, unresolved_providers),
                ));
            }

            // Initial progress event (current=0/total).
            if let Some(broadcaster) = event_broadcaster {
                let _ = broadcaster.emit_task(TaskProgressEvent::progress(
                    task.id,
                    "refresh_library_metadata",
                    0,
                    total_planned,
                    Some(format!(
                        "Refreshing {} ({} pair(s) planned)",
                        library.name, total_planned
                    )),
                    Some(library_id),
                    None,
                    None,
                ));
            }

            // 4. Walk the plan.
            //
            // Phase 2 is sequential: DatabaseConnection is Send + Sync and
            // PluginManager is Arc'd, so spawning per-pair tasks would also
            // work, but sequential keeps progress events ordered and avoids
            // interleaved plugin logs. The bounded JoinSet promised by the
            // plan is deferred until we measure that daily-cadence runs
            // need it. `max_concurrency` is clamped here so when the
            // parallel path lands, the config value is already validated.
            let _max_concurrency =
                (config.max_concurrency as usize).clamp(1, MAX_CONCURRENCY_HARD_CAP);

            let library_name = library.name.clone();
            // Mirror the config toggle into the strategy enum so future
            // call sites that key off `MatchingStrategy` (e.g. dry-run
            // preview, manual API endpoints) share the same vocabulary.
            let matching_strategy = if config.existing_source_ids_only {
                MatchingStrategy::ExistingExternalIdOnly
            } else {
                MatchingStrategy::AllowReMatch
            };

            for (idx, planned) in plan.planned.iter().enumerate() {
                // Resolve the field filter for this specific provider — the
                // planner doesn't know about the per-provider override, so the
                // handler computes it here. Without an override, this returns
                // the same set as the library-wide filter.
                let provider_key = format!("plugin:{}", planned.plugin.name);
                let pair_fields_filter = fields_filter_for_provider(&config, &provider_key);

                let pair_outcome = process_pair(
                    db,
                    library_id,
                    planned,
                    pair_fields_filter.as_ref(),
                    self.thumbnail_service.as_ref(),
                    event_broadcaster,
                    self.plugin_manager.as_ref(),
                    matching_strategy,
                )
                .await;

                match pair_outcome {
                    Ok(applied) => {
                        summary.succeeded += 1;
                        summary.fields_applied_total += applied as u32;
                    }
                    Err(PairError::NoExternalId) => {
                        // Reached only in `ExistingExternalIdOnly` strategy
                        // when somehow a no-id pair slipped past the
                        // planner (defense in depth — shouldn't happen).
                        warn!(
                            "Task {}: Skipping series {} for plugin {} — no external ID under \
                             ExistingExternalIdOnly strategy",
                            task.id, planned.series_id, planned.plugin.name
                        );
                        summary.skipped_no_external_id += 1;
                    }
                    Err(PairError::NoMatchCandidate) => {
                        // Loose-mode re-match found nothing usable.
                        info!(
                            "Task {}: No match candidate for series {} via plugin {}; skipping",
                            task.id, planned.series_id, planned.plugin.name
                        );
                        summary.skipped_no_match_candidate += 1;
                    }
                    Err(PairError::Failed(err)) => {
                        summary.failed += 1;
                        error!(
                            "Task {}: Failed refresh for series {} via plugin {}: {:#}",
                            task.id, planned.series_id, planned.plugin.name, err
                        );
                    }
                }

                // Per-pair progress event (after the work, so `current`
                // reflects what just finished).
                if let Some(broadcaster) = event_broadcaster {
                    let current = idx + 1;
                    let _ = broadcaster.emit_task(TaskProgressEvent::progress(
                        task.id,
                        "refresh_library_metadata",
                        current,
                        total_planned,
                        Some(format!(
                            "Refreshing {} ({}/{}, {} succeeded, {} failed)",
                            library_name, current, total_planned, summary.succeeded, summary.failed
                        )),
                        Some(library_id),
                        Some(planned.series_id),
                        None,
                    ));
                }
            }

            let message = format!(
                "Refreshed {} of {} pair(s) ({} succeeded, {} failed, {} skipped)",
                summary.succeeded,
                total_planned,
                summary.succeeded,
                summary.failed,
                summary.skipped_no_external_id
                    + summary.skipped_recently_synced
                    + summary.skipped_provider_unavailable
                    + summary.skipped_no_match_candidate,
            );

            Ok(TaskResult::success_with_data(
                message,
                summary.into_json(total_planned, unresolved_providers),
            ))
        })
    }
}

/// Fine-grained outcome for a single `(series, provider)` pair.
///
/// Wrapping the `Result` makes the handler loop self-documenting and lets
/// us distinguish strict-mode skips, loose-mode misses, and real failures.
enum PairError {
    /// Strict mode (`ExistingExternalIdOnly`) but the pair has no stored
    /// external ID. Should normally be filtered by the planner, but kept
    /// as a defensive branch.
    NoExternalId,
    /// Loose mode (`AllowReMatch`) called `metadata/series/match` and got
    /// no usable candidate.
    NoMatchCandidate,
    /// Plugin call or apply failed.
    Failed(anyhow::Error),
}

/// Process one planned pair. Returns the number of fields applied on
/// success, or a typed reason for non-success.
///
/// This function is `pub(crate)` rather than nested in the impl so the
/// `cargo fmt` output stays readable for a 100+ line function.
#[allow(clippy::too_many_arguments)]
async fn process_pair(
    db: &DatabaseConnection,
    library_id: uuid::Uuid,
    planned: &PlannedRefresh,
    fields_filter: Option<&std::collections::HashSet<String>>,
    thumbnail_service: Option<&Arc<ThumbnailService>>,
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
    plugin_manager: &PluginManager,
    matching_strategy: MatchingStrategy,
) -> Result<usize, PairError> {
    let plugin = &planned.plugin;

    // 1. Resolve which external ID to fetch.
    //
    // Strict mode: the planner already filtered no-id pairs, so
    // `existing_external_id` is always Some here. The defensive branch
    // returns `NoExternalId` if that ever changes.
    //
    // Loose mode: when there's no stored ID, call `metadata/series/match`
    // with the series' title to find one. A miss is `NoMatchCandidate`.
    let external_id = if let Some(record) = planned.existing_external_id.as_ref() {
        record.external_id.clone()
    } else {
        match matching_strategy {
            MatchingStrategy::ExistingExternalIdOnly => {
                return Err(PairError::NoExternalId);
            }
            MatchingStrategy::AllowReMatch => {
                rematch_external_id(db, planned, plugin_manager).await?
            }
        }
    };

    // 2. Fetch metadata from the plugin.
    let get_params = MetadataGetParams {
        external_id: external_id.clone(),
    };

    let metadata_fut = plugin_manager.get_series_metadata(plugin.id, get_params);
    let plugin_metadata = match tokio::time::timeout(PER_PAIR_TIMEOUT, metadata_fut).await {
        Ok(Ok(m)) => m,
        Ok(Err(e)) => {
            return Err(PairError::Failed(anyhow::Error::new(e).context(format!(
                "Plugin '{}' failed to fetch metadata for external_id {}",
                plugin.name, external_id
            ))));
        }
        Err(_elapsed) => {
            return Err(PairError::Failed(anyhow::anyhow!(
                "Plugin '{}' timed out after {}s fetching external_id {}",
                plugin.name,
                PER_PAIR_TIMEOUT.as_secs(),
                external_id
            )));
        }
    };

    // 3. Apply via the shared MetadataApplier.
    let current_metadata = SeriesMetadataRepository::get_by_series_id(db, planned.series_id)
        .await
        .map_err(|e| PairError::Failed(e.context("Failed to load current metadata")))?;

    let options = ApplyOptions {
        fields_filter: fields_filter.cloned(),
        thumbnail_service: thumbnail_service.cloned(),
        event_broadcaster: event_broadcaster.cloned(),
        dry_run: false,
    };

    let apply_result = MetadataApplier::apply(
        db,
        planned.series_id,
        library_id,
        plugin,
        &plugin_metadata,
        current_metadata.as_ref(),
        &options,
    )
    .await
    .map_err(|e| {
        PairError::Failed(e.context(format!(
            "Failed to apply metadata to series {}",
            planned.series_id
        )))
    })?;

    let applied_count = apply_result.applied_fields.len();

    // 4. Bump `last_synced_at` on the external-id row so the recency
    //    guard works on the next run. `external_url` is preserved.
    //    For the loose-mode re-match path, this is also where the freshly
    //    discovered external_id gets persisted for the next run.
    let external_url = plugin_metadata.external_url.clone();
    if let Err(e) = SeriesExternalIdRepository::upsert_for_plugin(
        db,
        planned.series_id,
        &plugin.name,
        &external_id,
        Some(&external_url),
        None,
    )
    .await
    {
        warn!(
            "Failed to refresh last_synced_at for series {} / plugin {}: {:#}",
            planned.series_id, plugin.name, e
        );
    }

    // 5. Emit a per-series metadata-updated event so the frontend
    //    invalidates its cache. Reuses the same event the manual
    //    apply path emits (see plugin_auto_match.rs).
    if applied_count > 0
        && let Some(broadcaster) = event_broadcaster
    {
        let _ = broadcaster.emit(EntityChangeEvent::new(
            EntityEvent::SeriesMetadataUpdated {
                series_id: planned.series_id,
                library_id,
                plugin_id: plugin.id,
                fields_updated: apply_result.applied_fields.clone(),
            },
            None,
        ));
    }

    // 6. Best-effort plugin success bookkeeping. The plugin manager
    //    already records success on its own in the happy path, but we
    //    keep this for parity with plugin_auto_match.
    if let Err(e) = PluginsRepository::record_success(db, plugin.id).await {
        debug!("Plugin success record skipped: {:#}", e);
    }

    Ok(applied_count)
}

/// Loose-mode helper: call `metadata/series/match` to find a candidate
/// external ID for a series with no stored mapping.
///
/// The match call uses the series' metadata title (when set) and falls back
/// to `series.name`. Year/author hints are intentionally omitted — they're
/// available to the manual flow via `series_context`, but the scheduled
/// refresh path keeps the call shape minimal so misconfigured matchers
/// don't accidentally narrow results.
///
/// Returns the external ID on success, [`PairError::NoMatchCandidate`] when
/// the plugin returned `None`, or [`PairError::Failed`] on plugin error.
async fn rematch_external_id(
    db: &DatabaseConnection,
    planned: &PlannedRefresh,
    plugin_manager: &PluginManager,
) -> Result<String, PairError> {
    let plugin = &planned.plugin;

    let series = SeriesRepository::get_by_id(db, planned.series_id)
        .await
        .map_err(|e| {
            PairError::Failed(e.context(format!(
                "Failed to load series {} for re-match",
                planned.series_id
            )))
        })?
        .ok_or_else(|| {
            PairError::Failed(anyhow::anyhow!(
                "Series {} disappeared during refresh",
                planned.series_id
            ))
        })?;

    let metadata = SeriesMetadataRepository::get_by_series_id(db, planned.series_id)
        .await
        .map_err(|e| {
            PairError::Failed(e.context("Failed to load current metadata for re-match"))
        })?;

    let title = metadata
        .as_ref()
        .map(|m| m.title.clone())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| series.name.clone());
    let year = metadata.as_ref().and_then(|m| m.year);

    let match_params = MetadataMatchParams {
        title,
        year,
        author: None,
    };

    let match_fut = plugin_manager.match_series(plugin.id, match_params);
    let match_result = match tokio::time::timeout(PER_PAIR_TIMEOUT, match_fut).await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            return Err(PairError::Failed(anyhow::Error::new(e).context(format!(
                "Plugin '{}' failed to re-match series {}",
                plugin.name, planned.series_id
            ))));
        }
        Err(_elapsed) => {
            return Err(PairError::Failed(anyhow::anyhow!(
                "Plugin '{}' timed out after {}s re-matching series {}",
                plugin.name,
                PER_PAIR_TIMEOUT.as_secs(),
                planned.series_id
            )));
        }
    };

    match match_result {
        Some(r) => Ok(r.external_id),
        None => Err(PairError::NoMatchCandidate),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::entities::plugins::PluginPermission;
    use crate::db::repositories::{
        LibraryRepository, PluginsRepository, SeriesRepository, TaskRepository,
    };
    use crate::db::test_helpers::setup_test_db;
    use crate::services::metadata::refresh_config::MetadataRefreshConfig;
    use crate::services::plugin::PluginManager;
    use crate::services::plugin::protocol::PluginScope;
    use crate::tasks::types::TaskType;
    use std::env;
    use std::sync::Once;

    static INIT_ENCRYPTION: Once = Once::new();

    fn setup_test_encryption_key() {
        INIT_ENCRYPTION.call_once(|| {
            if env::var("CODEX_ENCRYPTION_KEY").is_err() {
                // SAFETY: Tests share env. First-writer-wins is safe because
                // the value is constant.
                unsafe {
                    env::set_var(
                        "CODEX_ENCRYPTION_KEY",
                        "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=",
                    );
                }
            }
        });
    }

    #[test]
    fn run_summary_serializes_zero_state() {
        let json = RunSummary::default().into_json(0, vec![]);
        assert_eq!(json["planned"], 0);
        assert_eq!(json["succeeded"], 0);
        assert_eq!(json["failed"], 0);
        assert_eq!(json["skipped"]["no_external_id"], 0);
        assert_eq!(json["skipped"]["recently_synced"], 0);
        assert_eq!(json["skipped"]["provider_unavailable"], 0);
        assert_eq!(json["skipped"]["no_match_candidate"], 0);
        assert_eq!(json["fields_applied_total"], 0);
        assert!(json["unresolved_providers"].as_array().unwrap().is_empty());
    }

    #[test]
    fn run_summary_carries_unresolved_providers() {
        let s = RunSummary {
            succeeded: 3,
            failed: 1,
            skipped_no_external_id: 2,
            skipped_recently_synced: 1,
            skipped_provider_unavailable: 0,
            skipped_no_match_candidate: 1,
            fields_applied_total: 12,
        };
        let json = s.into_json(7, vec!["plugin:gone".to_string()]);
        assert_eq!(json["planned"], 7);
        assert_eq!(json["succeeded"], 3);
        assert_eq!(json["failed"], 1);
        assert_eq!(json["fields_applied_total"], 12);
        assert_eq!(json["skipped"]["no_external_id"], 2);
        assert_eq!(json["skipped"]["recently_synced"], 1);
        assert_eq!(json["skipped"]["no_match_candidate"], 1);
        assert_eq!(json["unresolved_providers"][0], "plugin:gone");
    }

    /// Build a worker-style task row from a `TaskType`.
    async fn enqueue_and_load(db: &DatabaseConnection, task_type: TaskType) -> tasks::Model {
        let id = TaskRepository::enqueue(db, task_type, None).await.unwrap();
        TaskRepository::get_by_id(db, id).await.unwrap().unwrap()
    }

    fn make_handler(db: &DatabaseConnection) -> RefreshLibraryMetadataHandler {
        let pm = Arc::new(PluginManager::with_defaults(Arc::new(db.clone())));
        RefreshLibraryMetadataHandler::new(pm)
    }

    #[tokio::test]
    async fn handler_short_circuits_when_no_providers_configured() {
        let db = setup_test_db().await;

        let library = LibraryRepository::create(
            &db,
            "lib-empty",
            "/tmp/lib-empty",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Save an enabled config but with empty providers list.
        let cfg = MetadataRefreshConfig {
            enabled: true,
            providers: vec![],
            ..Default::default()
        };
        LibraryRepository::set_metadata_refresh_config(&db, library.id, &cfg)
            .await
            .unwrap();

        let task = enqueue_and_load(
            &db,
            TaskType::RefreshLibraryMetadata {
                library_id: library.id,
            },
        )
        .await;

        let handler = make_handler(&db);
        let result = handler.handle(&task, &db, None).await.unwrap();

        assert!(result.success);
        let data = result.data.expect("data should be populated");
        assert_eq!(data["planned"], 0);
        assert_eq!(data["succeeded"], 0);
        assert_eq!(data["failed"], 0);
    }

    #[tokio::test]
    async fn handler_reports_unresolved_provider() {
        let db = setup_test_db().await;
        let library = LibraryRepository::create(
            &db,
            "lib-unresolved",
            "/tmp/lib-unresolved",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Provider references a plugin that doesn't exist.
        let cfg = MetadataRefreshConfig {
            enabled: true,
            providers: vec!["plugin:does-not-exist".to_string()],
            ..Default::default()
        };
        LibraryRepository::set_metadata_refresh_config(&db, library.id, &cfg)
            .await
            .unwrap();

        let task = enqueue_and_load(
            &db,
            TaskType::RefreshLibraryMetadata {
                library_id: library.id,
            },
        )
        .await;
        let handler = make_handler(&db);
        let result = handler.handle(&task, &db, None).await.unwrap();

        assert!(result.success);
        let data = result.data.unwrap();
        assert_eq!(data["planned"], 0);
        assert_eq!(data["unresolved_providers"][0], "plugin:does-not-exist");
    }

    #[tokio::test]
    async fn handler_counts_no_external_id_skips_in_strict_mode() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let library = LibraryRepository::create(
            &db,
            "lib-strict",
            "/tmp/lib-strict",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();
        // Two unmatched series.
        let _s1 = SeriesRepository::create(&db, library.id, "Series A", None)
            .await
            .unwrap();
        let _s2 = SeriesRepository::create(&db, library.id, "Series B", None)
            .await
            .unwrap();
        // Enabled plugin, but no external IDs seeded.
        let _plugin = PluginsRepository::create(
            &db,
            "mangabaka",
            "MangaBaka",
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
            true,
            None,
            None,
        )
        .await
        .unwrap();

        let cfg = MetadataRefreshConfig {
            enabled: true,
            providers: vec!["plugin:mangabaka".to_string()],
            existing_source_ids_only: true,
            skip_recently_synced_within_s: 0,
            ..Default::default()
        };
        LibraryRepository::set_metadata_refresh_config(&db, library.id, &cfg)
            .await
            .unwrap();

        let task = enqueue_and_load(
            &db,
            TaskType::RefreshLibraryMetadata {
                library_id: library.id,
            },
        )
        .await;
        let handler = make_handler(&db);
        let result = handler.handle(&task, &db, None).await.unwrap();

        assert!(result.success);
        let data = result.data.unwrap();
        // Both series skipped at planning time.
        assert_eq!(data["planned"], 0);
        assert_eq!(data["skipped"]["no_external_id"], 2);
        assert_eq!(data["succeeded"], 0);
        assert_eq!(data["failed"], 0);
    }

    /// Phase 4 contract: when `existing_source_ids_only = false`, the
    /// planner stops gating no-id pairs and the handler is responsible for
    /// invoking the re-match path. The handler thus *attempts* to call
    /// `metadata/series/match` for every unmatched series. Without a real
    /// plugin process the call fails, and the `failed` counter increments
    /// rather than `skipped_no_external_id` — proving loose mode is wired.
    /// This is the negative-space evidence that strict mode and loose mode
    /// take distinct branches.
    #[tokio::test]
    async fn handler_attempts_rematch_in_loose_mode() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let library = LibraryRepository::create(
            &db,
            "lib-loose",
            "/tmp/lib-loose",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();
        // One unmatched series.
        let _series = SeriesRepository::create(&db, library.id, "Series Loose", None)
            .await
            .unwrap();
        // Enabled plugin without a real process — match_series will fail.
        let _plugin = PluginsRepository::create(
            &db,
            "mangabaka",
            "MangaBaka",
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
            true,
            None,
            None,
        )
        .await
        .unwrap();

        let cfg = MetadataRefreshConfig {
            enabled: true,
            providers: vec!["plugin:mangabaka".to_string()],
            existing_source_ids_only: false,
            skip_recently_synced_within_s: 0,
            ..Default::default()
        };
        LibraryRepository::set_metadata_refresh_config(&db, library.id, &cfg)
            .await
            .unwrap();

        let task = enqueue_and_load(
            &db,
            TaskType::RefreshLibraryMetadata {
                library_id: library.id,
            },
        )
        .await;
        let handler = make_handler(&db);
        let result = handler.handle(&task, &db, None).await.unwrap();

        assert!(result.success);
        let data = result.data.unwrap();
        // Planner included the series (loose mode keeps no-id pairs).
        assert_eq!(data["planned"], 1);
        // The plugin call failed (no real process), so the pair was
        // counted as `failed` — NOT as `skipped_no_external_id`.
        assert_eq!(data["failed"], 1);
        assert_eq!(data["skipped"]["no_external_id"], 0);
        assert_eq!(data["succeeded"], 0);
    }

    #[tokio::test]
    async fn handler_errors_when_library_missing() {
        let db = setup_test_db().await;
        let task_type = TaskType::RefreshLibraryMetadata {
            library_id: uuid::Uuid::new_v4(),
        };

        // Build a synthetic task model directly (no DB row required since
        // the handler reads `task.library_id` and goes to the repository).
        let now = chrono::Utc::now();
        let task = tasks::Model {
            id: uuid::Uuid::new_v4(),
            task_type: task_type.type_string().to_string(),
            library_id: task_type.library_id(),
            series_id: None,
            book_id: None,
            params: None,
            status: "processing".to_string(),
            priority: 0,
            locked_by: None,
            locked_until: None,
            attempts: 1,
            max_attempts: 3,
            last_error: None,
            reschedule_count: 0,
            max_reschedules: 0,
            result: None,
            scheduled_for: now,
            created_at: now,
            started_at: Some(now),
            completed_at: None,
        };

        let handler = make_handler(&db);
        let err = handler.handle(&task, &db, None).await.unwrap_err();
        assert!(err.to_string().contains("Library not found"));
    }
}
