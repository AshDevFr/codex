//! Release-source polling scheduler integration.
//!
//! Each enabled `release_sources` row is registered as a tokio-cron-scheduler
//! job whose schedule is the row's effective cron expression:
//!
//! 1. `release_sources.cron_schedule` (per-source override) when non-NULL.
//! 2. Otherwise the server-wide `release_tracking.default_cron_schedule`
//!    setting.
//! 3. Otherwise the compile-time fallback (`"0 0 * * *"`, daily).
//!
//! When the cron fires, the job enqueues a `PollReleaseSource` task. The
//! task itself maintains per-host backoff via [`super::super::services::
//! release::backoff::HostBackoff`] (recording 429/503 from upstream and
//! resetting on success), so the scheduler does not need to skip cron
//! ticks based on backoff state. A cron firing during a throttled window
//! returns a 429 quickly without doing real work, and the task's recorded
//! error feeds the backoff state for the next tick.

use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::db::repositories::{ReleaseSourceRepository, TaskRepository};
use crate::services::release::schedule::{read_default_cron_schedule, resolve_cron_schedule};
use crate::services::settings::SettingsService;
use crate::tasks::types::TaskType;
use crate::utils::cron::normalize_cron_expression;

/// Tracks scheduler-registered jobs per source row so we can reconcile.
#[derive(Debug, Default)]
pub struct ReleaseSourceSchedule {
    /// Map of `release_sources.id` → tokio-cron-scheduler job UUID.
    jobs: HashMap<Uuid, Uuid>,
    /// Map of `release_sources.id` → effective cron expression currently
    /// registered (post-resolution, pre-normalization). Lets `reconcile`
    /// detect schedule changes without rebuilding every job on every pass.
    last_cron: HashMap<Uuid, String>,
}

impl ReleaseSourceSchedule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn registered_count(&self) -> usize {
        self.jobs.len()
    }

    pub fn contains(&self, source_id: Uuid) -> bool {
        self.jobs.contains_key(&source_id)
    }
}

/// Reconcile the scheduler's release-source jobs against the current set of
/// enabled rows. Adds new sources, removes disabled/deleted ones, and
/// re-registers any whose `cron_schedule` (or the inherited default) changed.
///
/// Idempotent: safe to call repeatedly (e.g. after a `release_sources` write).
pub async fn reconcile(
    scheduler: &mut JobScheduler,
    state: &mut ReleaseSourceSchedule,
    db: &DatabaseConnection,
    server_default: String,
) -> Result<()> {
    let enabled = ReleaseSourceRepository::list_enabled(db)
        .await
        .context("Failed to load enabled release sources")?;

    let mut seen: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
    for source in &enabled {
        seen.insert(source.id);
        let effective_cron =
            resolve_cron_schedule(source.cron_schedule.as_deref(), &server_default);

        if let Some(prev) = state.last_cron.get(&source.id)
            && prev == &effective_cron
            && state.contains(source.id)
        {
            // Same schedule, already registered — nothing to do.
            continue;
        }

        // Schedule changed (or first time we see this source) — drop any
        // existing job and register fresh.
        if let Some(job_id) = state.jobs.remove(&source.id)
            && let Err(e) = scheduler.remove(&job_id).await
        {
            warn!(
                "Failed to remove stale schedule for source {}: {}",
                source.id, e
            );
        }

        if let Err(e) = register_one(scheduler, state, db, source, &effective_cron).await {
            warn!(
                "Failed to register schedule for source {} ({}): {}",
                source.id, source.display_name, e
            );
        }
    }

    // Remove jobs whose source row is no longer enabled.
    let stale: Vec<Uuid> = state
        .jobs
        .keys()
        .copied()
        .filter(|id| !seen.contains(id))
        .collect();
    for source_id in stale {
        if let Some(job_id) = state.jobs.remove(&source_id) {
            state.last_cron.remove(&source_id);
            if let Err(e) = scheduler.remove(&job_id).await {
                warn!(
                    "Failed to remove stale schedule for source {}: {}",
                    source_id, e
                );
            } else {
                debug!("Removed stale schedule for source {}", source_id);
            }
        }
    }

    info!(
        "Reconciled release-source schedules: {} active",
        state.registered_count()
    );
    Ok(())
}

async fn register_one(
    scheduler: &mut JobScheduler,
    state: &mut ReleaseSourceSchedule,
    db: &DatabaseConnection,
    source: &crate::db::entities::release_sources::Model,
    effective_cron: &str,
) -> Result<()> {
    // Normalize 5-field POSIX cron to the 6-field form tokio-cron-scheduler
    // expects (or accept 6-field expressions as-is).
    let cron = normalize_cron_expression(effective_cron).with_context(|| {
        format!(
            "Invalid cron expression for source {} ({}): {}",
            source.id, source.display_name, effective_cron
        )
    })?;

    let db_clone = db.clone();
    let source_id = source.id;
    let display_name = source.display_name.clone();
    let job = Job::new_async(cron.as_str(), move |_uuid, _lock| {
        let db = db_clone.clone();
        let display_name = display_name.clone();
        Box::pin(async move {
            debug!(
                "Triggering scheduled poll for source {} ({})",
                source_id, display_name
            );
            let task_type = TaskType::PollReleaseSource { source_id };
            if let Err(e) = TaskRepository::enqueue(&db, task_type, None).await {
                error!(
                    "Failed to enqueue PollReleaseSource for source {}: {}",
                    source_id, e
                );
            }
        })
    })
    .with_context(|| format!("Failed to build cron job for source {}", source.id))?;

    let job_uuid = scheduler
        .add(job)
        .await
        .with_context(|| format!("Failed to add cron job for source {}", source.id))?;
    state.jobs.insert(source.id, job_uuid);
    state
        .last_cron
        .insert(source.id, effective_cron.to_string());

    info!(
        "Scheduled poll for source {} ({}) with cron `{}`",
        source.id, source.display_name, effective_cron
    );
    Ok(())
}

/// Outcome of an `enqueue_poll_now` call.
#[derive(Debug, Clone, Copy)]
pub struct EnqueuePollOutcome {
    /// The ID of the task — either the freshly enqueued one or the
    /// in-flight task we coalesced onto.
    pub task_id: Uuid,
    /// `true` when a pending/processing task already existed for this
    /// source and we returned its ID instead of enqueuing a new one.
    pub coalesced: bool,
}

/// Wrapper for callers (e.g., HTTP handlers) that want to enqueue a poll
/// directly instead of waiting for the scheduler tick.
///
/// **Dedup**: if a `poll_release_source` task for the same `source_id` is
/// already pending or processing, returns that task's ID instead of
/// enqueuing another one. This guards against the "click Poll now twice
/// and only one finishes" footgun: with a worker pool size > 1, two
/// independent tasks for the same source would race on `last_summary` /
/// `last_polled_at` writes and overlap upstream fetches. Coalescing onto
/// the in-flight task gives the user the same UX (their click acks) and
/// keeps the source's state coherent.
pub async fn enqueue_poll_now(
    db: &DatabaseConnection,
    source_id: Uuid,
) -> Result<EnqueuePollOutcome> {
    if let Some(existing) = TaskRepository::find_pending_or_processing_by_param(
        db,
        "poll_release_source",
        "source_id",
        &source_id.to_string(),
    )
    .await
    .context("Failed to check for in-flight poll task")?
    {
        return Ok(EnqueuePollOutcome {
            task_id: existing,
            coalesced: true,
        });
    }

    let task_type = TaskType::PollReleaseSource { source_id };
    let task_id = TaskRepository::enqueue(db, task_type, None)
        .await
        .context("Failed to enqueue PollReleaseSource task")?;
    Ok(EnqueuePollOutcome {
        task_id,
        coalesced: false,
    })
}

/// Read the resolved server-wide default cron schedule. Convenience for
/// callers (HTTP handlers, scheduler reconcile) that need it without
/// pulling in the schedule module directly.
pub async fn read_server_default_cron(settings: &SettingsService) -> String {
    read_default_cron_schedule(settings).await
}
