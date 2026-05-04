//! Per-job metadata refresh handler.
//!
//! Phase 9 entry point: the task carries a `job_id`, the handler loads the
//! [`library_jobs`] row, decodes its [`LibraryJobConfig`] (must be
//! `MetadataRefresh` to land here), resolves the library, builds a
//! [`RefreshPlan`] via [`RefreshPlanner`], and walks the plan one
//! `(series, plugin)` pair at a time.
//!
//! Scope: Phase 9 only honours `RefreshScope::SeriesOnly`. The validator
//! gates this at PATCH time, but the handler also rejects non-series scopes
//! at run time so a job that somehow persisted with a deferred scope
//! short-circuits with a clear failure status.
//!
//! [`library_jobs`]: crate::db::entities::library_jobs

use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::db::entities::tasks;
use crate::db::repositories::{
    LibraryJobRepository, LibraryRepository, PluginsRepository, RecordRunStatus,
    SeriesExternalIdRepository, SeriesMetadataRepository, SeriesRepository,
};
use crate::events::{EntityChangeEvent, EntityEvent, EventBroadcaster, TaskProgressEvent};
use crate::services::ThumbnailService;
use crate::services::library_jobs::{LibraryJobConfig, RefreshScope, parse_job_config};
use crate::services::metadata::refresh_planner::{
    PlanFailure, PlannedRefresh, RefreshPlan, RefreshPlanner, SkipReason,
    fields_filter_from_job_config,
};
use crate::services::metadata::{ApplyOptions, MatchingStrategy, MetadataApplier};
use crate::services::plugin::PluginManager;
use crate::services::plugin::protocol::{MetadataGetParams, MetadataMatchParams};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Soft cap to keep one job's refresh from monopolizing the worker.
const MAX_CONCURRENCY_HARD_CAP: usize = 16;

/// Per-`(series, provider)` plugin call timeout.
const PER_PAIR_TIMEOUT: Duration = Duration::from_secs(60);

/// Aggregated outcome of a single job run.
#[derive(Debug, Default)]
struct RunSummary {
    succeeded: u32,
    failed: u32,
    skipped_no_external_id: u32,
    skipped_recently_synced: u32,
    skipped_no_match_candidate: u32,
    fields_applied_total: u32,
}

impl RunSummary {
    fn into_json(
        self,
        total_planned: usize,
        plan_failure: Option<&PlanFailure>,
    ) -> serde_json::Value {
        json!({
            "planned": total_planned,
            "succeeded": self.succeeded,
            "failed": self.failed,
            "skipped": {
                "no_external_id": self.skipped_no_external_id,
                "recently_synced": self.skipped_recently_synced,
                "no_match_candidate": self.skipped_no_match_candidate,
            },
            "fields_applied_total": self.fields_applied_total,
            "plan_failure": plan_failure.map(|f| f.as_str()),
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

    fn fold_skipped_into_summary(plan: &RefreshPlan, summary: &mut RunSummary) {
        for s in &plan.skipped {
            match s.reason {
                SkipReason::NoExternalId => summary.skipped_no_external_id += 1,
                SkipReason::RecentlySynced { .. } => summary.skipped_recently_synced += 1,
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
            // 1. Resolve job_id from the task params payload.
            let job_id = task
                .params
                .as_ref()
                .and_then(|p| p.get("job_id"))
                .and_then(|v| v.as_str())
                .and_then(|s| uuid::Uuid::parse_str(s).ok())
                .ok_or_else(|| anyhow::anyhow!("Missing or invalid job_id in task params"))?;

            let job = LibraryJobRepository::get_by_id(db, job_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Library job not found: {}", job_id))?;

            let cfg = parse_job_config(&job.r#type, &job.config)
                .context("Failed to decode library job config")?;
            let LibraryJobConfig::MetadataRefresh(cfg) = cfg;

            // 2. Phase 9 scope guard. The validator should have rejected
            //    non-series scopes already; this is defense-in-depth so a
            //    persisted bad row fails loudly rather than silently no-op.
            if cfg.scope != RefreshScope::SeriesOnly {
                let msg = format!(
                    "Book-scope refresh ('{}') not yet implemented",
                    cfg.scope.as_str()
                );
                let _ = LibraryJobRepository::record_run(
                    db,
                    job.id,
                    RecordRunStatus::Failure,
                    Some(msg.clone()),
                )
                .await;
                return Ok(TaskResult::failure(msg));
            }

            let library = LibraryRepository::get_by_id(db, job.library_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Library not found for job: {}", job.library_id))?;

            info!(
                "Task {}: Refreshing job '{}' on library '{}' — provider={}, groups={:?}, strict={}",
                task.id,
                job.name,
                library.name,
                cfg.provider,
                cfg.field_groups,
                cfg.existing_source_ids_only
            );

            // 3. Verify capabilities haven't drifted between job-create and run-time.
            //    The validator already cross-checked at write; the runtime check
            //    catches plugin updates that drop the required capability after the fact.
            //    SeriesOnly requires `metadata_provider`.
            if let Some(plugin_name) = cfg.provider.strip_prefix("plugin:")
                && let Ok(Some(plugin)) =
                    crate::db::repositories::PluginsRepository::get_by_name(db, plugin_name).await
                && let Some(manifest) = plugin.cached_manifest()
                && !manifest.capabilities.can_provide_series_metadata()
            {
                let msg = format!(
                    "Provider '{}' no longer supports series metadata",
                    cfg.provider
                );
                let _ = LibraryJobRepository::record_run(
                    db,
                    job.id,
                    RecordRunStatus::Failure,
                    Some(msg.clone()),
                )
                .await;
                return Ok(TaskResult::failure(msg));
            }

            // 4. Build the plan.
            let plan = RefreshPlanner::plan(db, job.library_id, &cfg)
                .await
                .context("Failed to build refresh plan")?;

            let total_planned = plan.total_work();
            let plan_failure = plan.failure.clone();
            let mut summary = RunSummary::default();
            Self::fold_skipped_into_summary(&plan, &mut summary);

            if let Some(failure) = plan_failure.as_ref() {
                let msg = format!(
                    "Refresh aborted: {} (provider={})",
                    failure.as_str(),
                    cfg.provider
                );
                info!("Task {}: {}", task.id, msg);
                let summary_json = summary.into_json(total_planned, Some(failure));
                let _ = LibraryJobRepository::record_run(
                    db,
                    job.id,
                    RecordRunStatus::Failure,
                    Some(msg.clone()),
                )
                .await;
                return Ok(TaskResult::success_with_data(msg, summary_json));
            }

            if total_planned == 0 {
                let message = format!("Nothing to refresh ({} skipped)", plan.skipped.len());
                info!("Task {}: {}", task.id, message);
                let summary_json = summary.into_json(total_planned, None);
                let _ = LibraryJobRepository::record_run(
                    db,
                    job.id,
                    RecordRunStatus::Success,
                    Some(message.clone()),
                )
                .await;
                return Ok(TaskResult::success_with_data(message, summary_json));
            }

            // 5. Walk the plan.
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
                    Some(job.library_id),
                    None,
                    None,
                ));
            }

            let _max_concurrency =
                (cfg.max_concurrency as usize).clamp(1, MAX_CONCURRENCY_HARD_CAP);

            let library_name = library.name.clone();
            let matching_strategy = if cfg.existing_source_ids_only {
                MatchingStrategy::ExistingExternalIdOnly
            } else {
                MatchingStrategy::AllowReMatch
            };

            let pair_fields_filter = fields_filter_from_job_config(&cfg);

            for (idx, planned) in plan.planned.iter().enumerate() {
                let pair_outcome = process_pair(
                    db,
                    job.library_id,
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
                        warn!(
                            "Task {}: Skipping series {} (no external ID under strict mode)",
                            task.id, planned.series_id
                        );
                        summary.skipped_no_external_id += 1;
                    }
                    Err(PairError::NoMatchCandidate) => {
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
                        Some(job.library_id),
                        Some(planned.series_id),
                        None,
                    ));
                }
            }

            let total_skipped = summary.skipped_no_external_id
                + summary.skipped_recently_synced
                + summary.skipped_no_match_candidate;

            let message = format!(
                "Refreshed {} of {} pair(s) ({} succeeded, {} failed, {} skipped)",
                summary.succeeded, total_planned, summary.succeeded, summary.failed, total_skipped,
            );

            let final_status = if summary.failed > 0 && summary.succeeded == 0 {
                RecordRunStatus::Failure
            } else {
                RecordRunStatus::Success
            };
            let _ =
                LibraryJobRepository::record_run(db, job.id, final_status, Some(message.clone()))
                    .await;

            Ok(TaskResult::success_with_data(
                message,
                summary.into_json(total_planned, None),
            ))
        })
    }
}

/// Fine-grained outcome for a single `(series, provider)` pair.
enum PairError {
    NoExternalId,
    NoMatchCandidate,
    Failed(anyhow::Error),
}

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

    let external_id = if let Some(record) = planned.existing_external_id.as_ref() {
        record.external_id.clone()
    } else {
        match matching_strategy {
            MatchingStrategy::ExistingExternalIdOnly => return Err(PairError::NoExternalId),
            MatchingStrategy::AllowReMatch => {
                rematch_external_id(db, planned, plugin_manager).await?
            }
        }
    };

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

    let current_metadata = SeriesMetadataRepository::get_by_series_id(db, planned.series_id)
        .await
        .map_err(|e| PairError::Failed(e.context("Failed to load current metadata")))?;

    let _ = matching_strategy;
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

    if let Err(e) = PluginsRepository::record_success(db, plugin.id).await {
        debug!("Plugin success record skipped: {:#}", e);
    }

    Ok(applied_count)
}

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
        CreateLibraryJobParams, LibraryJobRepository, LibraryRepository, PluginsRepository,
        SeriesRepository, TaskRepository,
    };
    use crate::db::test_helpers::setup_test_db;
    use crate::services::library_jobs::{
        LibraryJobConfig, MetadataRefreshJobConfig, RefreshScope, parse_job_config,
    };
    use crate::services::plugin::PluginManager;
    use crate::services::plugin::protocol::PluginScope;
    use crate::tasks::types::TaskType;
    use std::env;
    use std::sync::Once;

    static INIT_ENCRYPTION: Once = Once::new();

    fn setup_test_encryption_key() {
        INIT_ENCRYPTION.call_once(|| {
            if env::var("CODEX_ENCRYPTION_KEY").is_err() {
                // SAFETY: tests share env. First-writer-wins is safe.
                unsafe {
                    env::set_var(
                        "CODEX_ENCRYPTION_KEY",
                        "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=",
                    );
                }
            }
        });
    }

    fn refresh_cfg(provider: &str) -> MetadataRefreshJobConfig {
        MetadataRefreshJobConfig {
            provider: provider.to_string(),
            scope: RefreshScope::SeriesOnly,
            field_groups: vec!["ratings".to_string()],
            extra_fields: vec![],
            book_field_groups: vec![],
            book_extra_fields: vec![],
            existing_source_ids_only: true,
            skip_recently_synced_within_s: 0,
            max_concurrency: 4,
        }
    }

    async fn create_job(
        db: &DatabaseConnection,
        library_id: uuid::Uuid,
        cfg: MetadataRefreshJobConfig,
    ) -> uuid::Uuid {
        let wrapped = LibraryJobConfig::MetadataRefresh(cfg);
        let json = serde_json::to_string(&wrapped).unwrap();
        let row = LibraryJobRepository::create(
            db,
            CreateLibraryJobParams {
                library_id,
                job_type: "metadata_refresh".to_string(),
                name: "Test Job".to_string(),
                enabled: true,
                cron_schedule: "0 0 4 * * *".to_string(),
                timezone: None,
                config: json,
            },
        )
        .await
        .unwrap();
        row.id
    }

    async fn enqueue_and_load(db: &DatabaseConnection, task_type: TaskType) -> tasks::Model {
        let id = TaskRepository::enqueue(db, task_type, None).await.unwrap();
        TaskRepository::get_by_id(db, id).await.unwrap().unwrap()
    }

    fn make_handler(db: &DatabaseConnection) -> RefreshLibraryMetadataHandler {
        let pm = Arc::new(PluginManager::with_defaults(Arc::new(db.clone())));
        RefreshLibraryMetadataHandler::new(pm)
    }

    #[test]
    fn run_summary_zero_state() {
        let json = RunSummary::default().into_json(0, None);
        assert_eq!(json["planned"], 0);
        assert_eq!(json["succeeded"], 0);
        assert_eq!(json["skipped"]["no_external_id"], 0);
        assert_eq!(json["skipped"]["recently_synced"], 0);
        assert_eq!(json["skipped"]["no_match_candidate"], 0);
        assert!(json["plan_failure"].is_null());
    }

    #[test]
    fn run_summary_with_plan_failure() {
        let s = RunSummary::default();
        let json = s.into_json(0, Some(&PlanFailure::PluginMissing));
        assert_eq!(json["plan_failure"], "plugin_missing");
    }

    #[test]
    fn parse_job_config_round_trips_for_handler() {
        let cfg = refresh_cfg("plugin:x");
        let wrapped = LibraryJobConfig::MetadataRefresh(cfg.clone());
        let json = serde_json::to_string(&wrapped).unwrap();
        let parsed = parse_job_config("metadata_refresh", &json).unwrap();
        let LibraryJobConfig::MetadataRefresh(out) = parsed;
        assert_eq!(out.provider, cfg.provider);
    }

    #[tokio::test]
    async fn handler_short_circuits_when_provider_missing() {
        let db = setup_test_db().await;
        let lib = LibraryRepository::create(
            &db,
            "lib-empty",
            "/tmp/lib-empty",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let cfg = refresh_cfg("plugin:does-not-exist");
        let job_id = create_job(&db, lib.id, cfg).await;

        let task = enqueue_and_load(&db, TaskType::RefreshLibraryMetadata { job_id }).await;
        let handler = make_handler(&db);
        let result = handler.handle(&task, &db, None).await.unwrap();
        assert!(result.success);
        let data = result.data.unwrap();
        assert_eq!(data["planned"], 0);
        assert_eq!(data["plan_failure"], "plugin_missing");
    }

    #[tokio::test]
    async fn handler_counts_no_external_id_in_strict_mode() {
        setup_test_encryption_key();
        let db = setup_test_db().await;
        let lib = LibraryRepository::create(
            &db,
            "lib-strict",
            "/tmp/lib-strict",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();
        let _s1 = SeriesRepository::create(&db, lib.id, "S1", None)
            .await
            .unwrap();
        let _s2 = SeriesRepository::create(&db, lib.id, "S2", None)
            .await
            .unwrap();
        let _plugin = PluginsRepository::create(
            &db,
            "mb",
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

        let cfg = refresh_cfg("plugin:mb");
        let job_id = create_job(&db, lib.id, cfg).await;
        let task = enqueue_and_load(&db, TaskType::RefreshLibraryMetadata { job_id }).await;
        let handler = make_handler(&db);
        let result = handler.handle(&task, &db, None).await.unwrap();
        assert!(result.success);
        let data = result.data.unwrap();
        assert_eq!(data["planned"], 0);
        assert_eq!(data["skipped"]["no_external_id"], 2);
    }

    #[tokio::test]
    async fn handler_records_failure_when_job_missing() {
        let db = setup_test_db().await;
        let task = enqueue_and_load(
            &db,
            TaskType::RefreshLibraryMetadata {
                job_id: uuid::Uuid::new_v4(),
            },
        )
        .await;
        let handler = make_handler(&db);
        let err = handler.handle(&task, &db, None).await.unwrap_err();
        assert!(err.to_string().contains("Library job not found"));
    }
}
