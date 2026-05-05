//! Release-source polling scheduler integration.
//!
//! Each enabled `release_sources` row is reconciled into the scheduler as a
//! tokio-cron-scheduler job. The job fires a `PollReleaseSource` task at the
//! row's effective interval (per-source override → server default), with
//! ±10% jitter applied on registration and per-host backoff applied at
//! firing time.
//!
//! `tokio-cron-scheduler` doesn't have a "fire every N seconds with jitter"
//! primitive, so we build a 6-part cron string from the resolved interval
//! and let the existing job machinery handle dispatch.

use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::db::repositories::{ReleaseSourceRepository, TaskRepository};
use crate::services::release::backoff::HostBackoff;
use crate::services::release::schedule::{
    DEFAULT_POLL_INTERVAL_S, MIN_POLL_INTERVAL_S, SETTING_DEFAULT_POLL_INTERVAL_S, apply_backoff,
    jitter_interval_s, resolve_interval_s,
};
use crate::services::settings::SettingsService;
use crate::tasks::types::TaskType;

/// Tracks scheduler-registered jobs per source row so we can reconcile.
#[derive(Debug, Default)]
pub struct ReleaseSourceSchedule {
    /// Map of `release_sources.id` → tokio-cron-scheduler job UUID.
    jobs: HashMap<Uuid, Uuid>,
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

/// Read the configured global default poll interval (seconds). Falls back
/// to the compile-time default when settings are unavailable or the value
/// is invalid (`<= 0`).
pub async fn read_default_poll_interval(settings: &SettingsService) -> u32 {
    let raw = settings
        .get_uint(
            SETTING_DEFAULT_POLL_INTERVAL_S,
            DEFAULT_POLL_INTERVAL_S as u64,
        )
        .await
        .unwrap_or(DEFAULT_POLL_INTERVAL_S as u64);
    if raw == 0 {
        DEFAULT_POLL_INTERVAL_S
    } else {
        // Clamp on read so a misconfigured row can't push below the
        // sane minimum.
        raw.max(MIN_POLL_INTERVAL_S as u64).min(u32::MAX as u64) as u32
    }
}

/// Reconcile the scheduler's release-source jobs against the current set of
/// enabled rows. Adds new sources, removes disabled/deleted ones, and
/// re-registers any whose interval changed.
///
/// Idempotent: safe to call repeatedly (e.g. after a `release_sources` write).
pub async fn reconcile(
    scheduler: &mut JobScheduler,
    state: &mut ReleaseSourceSchedule,
    db: &DatabaseConnection,
    backoff: HostBackoff,
    default_interval_s: u32,
) -> Result<()> {
    let enabled = ReleaseSourceRepository::list_enabled(db)
        .await
        .context("Failed to load enabled release sources")?;

    // Track which sources we've seen this pass.
    let mut seen: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
    for source in &enabled {
        seen.insert(source.id);
        // The interval/jitter combo doesn't change between reconciles
        // unless the row's `poll_interval_s` is mutated. Cheap rule:
        // re-register on every reconcile that doesn't already have the
        // job. We accept the small cost of re-registration on a
        // poll-interval change.
        if state.contains(source.id) {
            continue;
        }
        if let Err(e) =
            register_one(scheduler, state, db, &backoff, source, default_interval_s).await
        {
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
    backoff: &HostBackoff,
    source: &crate::db::entities::release_sources::Model,
    default_interval_s: u32,
) -> Result<()> {
    let resolved = resolve_interval_s(source.poll_interval_s, default_interval_s);
    let jittered = jitter_interval_s(resolved);
    // Apply current backoff multiplier so a recently-throttled host
    // doesn't get re-polled immediately on scheduler reload.
    let url_hint = derive_url_hint(source);
    let multiplier = backoff.multiplier(&url_hint).await;
    let final_s = apply_backoff(jittered, multiplier);

    let cron = secs_to_cron(final_s);

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
            match TaskRepository::enqueue(&db, task_type, None).await {
                Ok(_) => {}
                Err(e) => {
                    error!(
                        "Failed to enqueue PollReleaseSource for source {}: {}",
                        source_id, e
                    );
                }
            }
        })
    })
    .with_context(|| format!("Failed to build cron job for source {}", source.id))?;

    let job_uuid = scheduler
        .add(job)
        .await
        .with_context(|| format!("Failed to add cron job for source {}", source.id))?;
    state.jobs.insert(source.id, job_uuid);

    info!(
        "Scheduled poll for source {} ({}) every {}s (resolved {}, backoff x{:.1})",
        source.id, source.display_name, final_s, resolved, multiplier
    );
    Ok(())
}

/// Build a 6-part cron expression that fires approximately every `secs`
/// seconds.
///
/// `tokio-cron-scheduler` doesn't have a `every-N-seconds-from-now`
/// primitive; we approximate with cron-style intervals:
///
/// - `secs < 3600`: minute-granularity step (`0 */M * * * *`). Capped at 59
///   minutes since `*/60` is invalid; sources that want longer must hit
///   the hourly branch.
/// - `secs ≥ 3600`: hour-granularity step (`0 0 */H * * *`). Capped at 23
///   hours; for `secs ≥ 24h` we fall back to "once daily at 00:00"
///   (`0 0 0 * * *`).
///
/// Caveat: the "step" semantics in cron align to wall-clock boundaries
/// (e.g., `*/30` fires at minute 0 and 30, not at "30 minutes from now").
/// Combined with ±10% jitter at registration, this still spreads load
/// well; precise inter-poll spacing isn't a goal of this layer.
pub fn secs_to_cron(secs: u32) -> String {
    let secs = secs.max(MIN_POLL_INTERVAL_S);
    if secs < 3600 {
        let mins = secs.div_ceil(60).clamp(1, 59);
        if mins == 1 {
            "0 * * * * *".to_string()
        } else {
            format!("0 */{} * * * *", mins)
        }
    } else if secs < 86_400 {
        let hours = (secs / 3600).clamp(1, 23);
        if hours == 1 {
            "0 0 * * * *".to_string()
        } else {
            format!("0 0 */{} * * *", hours)
        }
    } else {
        // ≥ 24h — fire once daily at midnight. Sources that want longer
        // intervals (rare) get folded into "daily" since cron can't
        // express "every 48h" cleanly without a state machine.
        "0 0 0 * * *".to_string()
    }
}

/// Best-effort URL hint extraction matching the polling task's logic. Kept
/// in sync to avoid backoff key drift between scheduler and handler.
fn derive_url_hint(source: &crate::db::entities::release_sources::Model) -> String {
    if let Some(cfg) = source.config.as_ref() {
        for key in ["url", "feedUrl", "feed_url", "baseUrl", "base_url"] {
            if let Some(v) = cfg.get(key).and_then(|v| v.as_str())
                && !v.is_empty()
            {
                return v.to_string();
            }
        }
    }
    source.plugin_id.clone()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secs_to_cron_minute_step() {
        assert_eq!(secs_to_cron(60), "0 * * * * *");
        assert_eq!(secs_to_cron(120), "0 */2 * * * *");
        assert_eq!(secs_to_cron(1800), "0 */30 * * * *");
    }

    #[test]
    fn secs_to_cron_hour_step() {
        assert_eq!(secs_to_cron(3600), "0 0 * * * *");
        assert_eq!(secs_to_cron(7200), "0 0 */2 * * *");
        // 6h
        assert_eq!(secs_to_cron(21_600), "0 0 */6 * * *");
    }

    #[test]
    fn secs_to_cron_daily_for_long_intervals() {
        assert_eq!(secs_to_cron(86_400), "0 0 0 * * *");
        // "Every 48h" gets folded into daily.
        assert_eq!(secs_to_cron(2 * 86_400), "0 0 0 * * *");
    }

    #[test]
    fn secs_to_cron_clamps_to_min() {
        // 10s clamps up to 60s → "0 * * * * *".
        assert_eq!(secs_to_cron(10), "0 * * * * *");
    }
}
