//! Scheduled per-library metadata refresh handler.
//!
//! Reads `libraries.metadata_refresh_config`, builds a [`RefreshPlan`] via
//! [`RefreshPlanner`], then walks the plan one `(series, plugin)` pair at a
//! time. Each pair fetches metadata via the plugin's `metadata/series/get`
//! call (using the stored external ID) and applies it through the existing
//! [`MetadataApplier`]. Per-pair errors are isolated: one failure increments
//! a counter and the handler keeps going.
//!
//! ## Phase scope (Phase 2)
//!
//! - Only the **`existing_source_ids_only = true`** path is wired. The
//!   [`RefreshPlanner`] does the gating, so the handler never sees a
//!   no-external-id pair when strict mode is on.
//! - When strict mode is off, the planner still returns pairs without an
//!   external ID. Phase 2 logs a warning and skips them — Phase 4 will
//!   introduce a real `MatchingStrategy` that lets the handler call
//!   `metadata/series/match` to re-match.
//! - Field-group resolution is the simple union returned by
//!   [`fields_filter_from_config`]. Phase 3 ships the proper resolver that
//!   expands group names like `"ratings"` to the underlying field names
//!   the applier understands.

use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::db::entities::tasks;
use crate::db::repositories::{
    LibraryRepository, PluginsRepository, SeriesExternalIdRepository, SeriesMetadataRepository,
};
use crate::events::{EntityChangeEvent, EntityEvent, EventBroadcaster, TaskProgressEvent};
use crate::services::ThumbnailService;
use crate::services::metadata::refresh_planner::{
    PlannedRefresh, RefreshPlan, RefreshPlanner, fields_filter_from_config,
};
use crate::services::metadata::{ApplyOptions, MetadataApplier};
use crate::services::plugin::PluginManager;
use crate::services::plugin::protocol::MetadataGetParams;
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

            let fields_filter = fields_filter_from_config(&config);
            let library_name = library.name.clone();

            for (idx, planned) in plan.planned.iter().enumerate() {
                let pair_outcome = process_pair(
                    db,
                    library_id,
                    planned,
                    fields_filter.as_ref(),
                    self.thumbnail_service.as_ref(),
                    event_broadcaster,
                    self.plugin_manager.as_ref(),
                )
                .await;

                match pair_outcome {
                    Ok(applied) => {
                        summary.succeeded += 1;
                        summary.fields_applied_total += applied as u32;
                    }
                    Err(SkipBecauseNoExternalId) => {
                        // Loose-mode planner returns these; Phase 4 will
                        // convert this into a real re-match path.
                        warn!(
                            "Task {}: Skipping series {} for plugin {} — no external ID and \
                             re-matching is not yet implemented (Phase 4)",
                            task.id, planned.series_id, planned.plugin.name
                        );
                        summary.skipped_no_external_id += 1;
                    }
                    Err(SkipBecauseFailed(err)) => {
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
                    + summary.skipped_provider_unavailable,
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
/// us distinguish "planner gave us a no-ID pair" from "the call failed".
enum PairError {
    NoExternalId,
    Failed(anyhow::Error),
}
use PairError::Failed as SkipBecauseFailed;
use PairError::NoExternalId as SkipBecauseNoExternalId;

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
) -> Result<usize, PairError> {
    // Phase 2: only the existing-ID path is wired.
    let Some(external_id_record) = planned.existing_external_id.as_ref() else {
        return Err(PairError::NoExternalId);
    };
    let external_id = external_id_record.external_id.clone();
    let plugin = &planned.plugin;

    // 1. Fetch metadata from the plugin.
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

    // 2. Apply via the shared MetadataApplier.
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

    // 3. Bump `last_synced_at` on the external-id row so the recency
    //    guard works on the next run. `external_url` is preserved.
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

    // 4. Emit a per-series metadata-updated event so the frontend
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

    // 5. Best-effort plugin success bookkeeping. The plugin manager
    //    already records success on its own in the happy path, but we
    //    keep this for parity with plugin_auto_match.
    if let Err(e) = PluginsRepository::record_success(db, plugin.id).await {
        debug!("Plugin success record skipped: {:#}", e);
    }

    Ok(applied_count)
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
            fields_applied_total: 12,
        };
        let json = s.into_json(7, vec!["plugin:gone".to_string()]);
        assert_eq!(json["planned"], 7);
        assert_eq!(json["succeeded"], 3);
        assert_eq!(json["failed"], 1);
        assert_eq!(json["fields_applied_total"], 12);
        assert_eq!(json["skipped"]["no_external_id"], 2);
        assert_eq!(json["skipped"]["recently_synced"], 1);
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
