//! Cross-process task **progress** bridge (worker side).
//!
//! Task *completion* is bridged to the web server by a database trigger that
//! fires `pg_notify('task_completion', ...)` when a task row transitions to
//! `completed`/`failed` (see the `notify_task_completion` migration). Progress
//! updates, however, are high-frequency and ephemeral: persisting every tick to
//! the task row just to trigger a NOTIFY would be pure write amplification.
//!
//! Instead, worker processes forward `TaskProgressEvent`s emitted during task
//! execution into an in-memory channel. This publisher drains that channel,
//! throttles per task, and re-publishes each admitted event directly via
//! `pg_notify('task_progress', <json>)` — reusing the same LISTEN/NOTIFY
//! transport as completion, but without touching any table. The web server's
//! [`crate::TaskListener`] listens on `task_progress` and re-broadcasts to SSE.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use codex_events::{TaskProgressEvent, TaskStatus};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use tokio::sync::mpsc;
use tracing::{debug, info};
use uuid::Uuid;

/// PostgreSQL channel name for task progress notifications.
pub const TASK_PROGRESS_CHANNEL: &str = "task_progress";

/// Default minimum interval between published progress updates for a single
/// task. Progress events arriving faster than this are dropped.
pub const DEFAULT_THROTTLE: Duration = Duration::from_millis(500);

/// Per-task rate limiter deciding which progress events are published.
///
/// Rules:
/// - **Terminal** events (`Completed`/`Failed`) are never published here; the
///   database trigger on the tasks table owns terminal delivery via the
///   `task_completion` channel. They are used only to evict per-task state.
/// - **Started** events (status `Running` with no `progress` payload) always
///   pass, so the UI reflects a task going active immediately.
/// - **Progress** events (status `Running` with a `progress` payload) pass at
///   most once per `interval` per task.
struct ProgressThrottle {
    interval: Duration,
    last_published: HashMap<Uuid, Instant>,
}

impl ProgressThrottle {
    fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_published: HashMap::new(),
        }
    }

    /// Decide whether `event` should be published now, updating internal state.
    fn admit(&mut self, event: &TaskProgressEvent, now: Instant) -> bool {
        match event.status {
            TaskStatus::Completed | TaskStatus::Failed => {
                // Terminal states are delivered by the DB trigger; drop here and
                // release the per-task entry so the map does not grow unbounded.
                self.last_published.remove(&event.task_id);
                false
            }
            TaskStatus::Pending | TaskStatus::Running => {
                // A "started" event carries no progress payload -> always admit.
                if event.progress.is_none() {
                    self.last_published.insert(event.task_id, now);
                    return true;
                }
                match self.last_published.get(&event.task_id) {
                    Some(&prev) if now.duration_since(prev) < self.interval => false,
                    _ => {
                        self.last_published.insert(event.task_id, now);
                        true
                    }
                }
            }
        }
    }
}

/// Spawn the task progress publisher.
///
/// Drains `rx`, throttles per task, and publishes admitted events via
/// `pg_notify` on the [`TASK_PROGRESS_CHANNEL`]. The returned handle completes
/// when all senders are dropped (i.e. on worker shutdown).
pub fn spawn(
    db: DatabaseConnection,
    rx: mpsc::Receiver<TaskProgressEvent>,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(run(db, rx, interval))
}

async fn run(
    db: DatabaseConnection,
    mut rx: mpsc::Receiver<TaskProgressEvent>,
    interval: Duration,
) {
    info!(
        "Task progress publisher started (channel '{}', throttle {:?})",
        TASK_PROGRESS_CHANNEL, interval
    );
    let mut throttle = ProgressThrottle::new(interval);

    while let Some(event) = rx.recv().await {
        if !throttle.admit(&event, Instant::now()) {
            continue;
        }

        let payload = match serde_json::to_string(&event) {
            Ok(p) => p,
            Err(e) => {
                debug!("Failed to serialize task progress event: {e}");
                continue;
            }
        };

        // `pg_notify` runs in autocommit here, so the notification is delivered
        // immediately. Payloads are capped at 8000 bytes by PostgreSQL; progress
        // messages are far smaller, so no truncation guard is needed.
        let stmt = Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT pg_notify($1, $2)",
            [TASK_PROGRESS_CHANNEL.into(), payload.into()],
        );
        if let Err(e) = db.execute(stmt).await {
            debug!("Failed to publish task progress notify: {e}");
        }
    }

    info!("Task progress publisher stopped");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started(task_id: Uuid) -> TaskProgressEvent {
        TaskProgressEvent::started(task_id, "scan_library", None, None, None)
    }

    fn progress(task_id: Uuid, current: usize) -> TaskProgressEvent {
        TaskProgressEvent::progress(
            task_id,
            "scan_library",
            current,
            100,
            None,
            None,
            None,
            None,
        )
    }

    fn completed(task_id: Uuid) -> TaskProgressEvent {
        TaskProgressEvent::completed(
            task_id,
            "scan_library",
            chrono::Utc::now(),
            None,
            None,
            None,
        )
    }

    #[test]
    fn started_events_always_admitted() {
        let mut t = ProgressThrottle::new(Duration::from_millis(500));
        let id = Uuid::new_v4();
        let now = Instant::now();
        assert!(t.admit(&started(id), now));
        // A second started-like event (no progress payload) still passes.
        assert!(t.admit(&started(id), now));
    }

    #[test]
    fn progress_is_throttled_per_task() {
        let mut t = ProgressThrottle::new(Duration::from_millis(500));
        let id = Uuid::new_v4();
        let t0 = Instant::now();

        // First progress admitted.
        assert!(t.admit(&progress(id, 1), t0));
        // Within the window -> dropped.
        assert!(!t.admit(&progress(id, 2), t0 + Duration::from_millis(100)));
        assert!(!t.admit(&progress(id, 3), t0 + Duration::from_millis(499)));
        // After the window -> admitted again.
        assert!(t.admit(&progress(id, 4), t0 + Duration::from_millis(500)));
    }

    #[test]
    fn throttle_is_independent_across_tasks() {
        let mut t = ProgressThrottle::new(Duration::from_millis(500));
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let now = Instant::now();
        assert!(t.admit(&progress(a, 1), now));
        // Different task within the same instant is unaffected.
        assert!(t.admit(&progress(b, 1), now));
    }

    #[test]
    fn progress_event_survives_notify_json_round_trip() {
        // The publisher serializes with serde_json and the TaskListener
        // deserializes the same type off the `task_progress` channel. Lock that
        // wire contract so a future field change can't silently break it.
        let id = Uuid::new_v4();
        let lib = Uuid::new_v4();
        let event = TaskProgressEvent::progress(
            id,
            "scan_library",
            57,
            15106,
            Some("Scanning Manga (57/15106 files)".to_string()),
            Some(lib),
            None,
            None,
        );

        let payload = serde_json::to_string(&event).expect("serialize");
        let decoded: TaskProgressEvent = serde_json::from_str(&payload).expect("deserialize");

        assert_eq!(decoded.task_id, id);
        assert_eq!(decoded.task_type, "scan_library");
        assert_eq!(decoded.status, TaskStatus::Running);
        assert_eq!(decoded.library_id, Some(lib));
        let progress = decoded.progress.expect("progress payload preserved");
        assert_eq!(progress.current, 57);
        assert_eq!(progress.total, 15106);
        assert_eq!(
            progress.message.as_deref(),
            Some("Scanning Manga (57/15106 files)")
        );
    }

    #[test]
    fn terminal_events_are_dropped_and_evict_state() {
        let mut t = ProgressThrottle::new(Duration::from_millis(500));
        let id = Uuid::new_v4();
        let t0 = Instant::now();

        assert!(t.admit(&progress(id, 1), t0));
        // Terminal event is never published on this channel...
        assert!(!t.admit(&completed(id), t0 + Duration::from_millis(10)));
        // ...and it evicted the per-task timer, so a subsequent progress for a
        // reused id is admitted immediately rather than being throttled.
        assert!(t.admit(&progress(id, 2), t0 + Duration::from_millis(20)));
        assert!(t.last_published.contains_key(&id));
    }
}
