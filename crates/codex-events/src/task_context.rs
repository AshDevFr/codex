//! Tokio task-local that exposes the "current task's recording broadcaster"
//! to code that runs inside a `TaskHandler::handle` call (and to any
//! reverse-RPC dispatch the handler triggers, since the dispatcher runs on
//! the caller's task — see `services::plugin::rpc`).
//!
//! Why this exists: when a worker runs a task in distributed mode (PostgreSQL
//! deployments), it creates a per-task recording broadcaster so every
//! `EntityChangeEvent` emitted during the task is captured into
//! `tasks.result.emitted_events` and replayed by the web server's
//! `TaskListener`. Code that emits events inside the task call stack receives
//! the broadcaster as a parameter — but plugin reverse-RPC handlers
//! (`releases/record` etc.) sit behind a JSON-RPC dispatcher that only
//! receives the request, not the broadcaster. Threading the broadcaster
//! through every layer of the dispatcher is invasive; the task-local is the
//! seam.
//!
//! The reverse-RPC dispatcher in [`codex::services::plugin::rpc`] runs the
//! dispatch on the *caller's* tokio task (the one that issued the forward
//! call), so the task-local set up by [`codex::tasks::worker`] is in scope.

use std::sync::Arc;
use std::sync::Mutex;

use uuid::Uuid;

use super::EventBroadcaster;

tokio::task_local! {
    /// Recording broadcaster for the currently-executing task. Set by the
    /// worker around `handler.handle(...)`. Read by reverse-RPC handlers via
    /// [`current_recording_broadcaster`].
    static CURRENT_RECORDING_BROADCASTER: Arc<EventBroadcaster>;
    /// Identity + progress-throttle state for the currently-executing task.
    /// Set by the worker around `handler.handle(...)`. Read by reverse-RPC
    /// handlers via [`current_task_identity`] when they need to construct a
    /// `TaskProgressEvent` (which requires the task's id and type) or
    /// rate-limit progress emits.
    static CURRENT_TASK_IDENTITY: Arc<TaskIdentity>;
}

/// Task identity exposed to reverse-RPC handlers via the
/// [`CURRENT_TASK_IDENTITY`] task-local. Carries the fields needed to build
/// a [`super::TaskProgressEvent`] plus a tiny throttle-state cell so
/// `releases/report_progress` can drop emits arriving faster than the
/// configured cadence.
#[derive(Debug)]
pub struct TaskIdentity {
    pub task_id: Uuid,
    pub task_type: String,
    pub library_id: Option<Uuid>,
    pub series_id: Option<Uuid>,
    pub book_id: Option<Uuid>,
    /// Last time a progress emit went through. `None` until the first emit.
    /// Wrapped in a `Mutex` so reverse-RPC handlers (which see the identity
    /// behind an `Arc`) can update it without a `&mut`.
    pub last_progress_emit: Mutex<Option<std::time::Instant>>,
}

impl TaskIdentity {
    pub fn new(
        task_id: Uuid,
        task_type: impl Into<String>,
        library_id: Option<Uuid>,
        series_id: Option<Uuid>,
        book_id: Option<Uuid>,
    ) -> Self {
        Self {
            task_id,
            task_type: task_type.into(),
            library_id,
            series_id,
            book_id,
            last_progress_emit: Mutex::new(None),
        }
    }
}

/// Run `fut` with `broadcaster` as the current task's recording broadcaster.
///
/// Anything inside `fut` that calls [`current_recording_broadcaster`] sees
/// `Some(broadcaster)`. Outside this scope, callers see `None` and should
/// fall back to whatever they would have done previously (typically: skip
/// the emit, since out-of-task emits have nowhere to be replayed to).
pub async fn with_recording_broadcaster<F, T>(broadcaster: Arc<EventBroadcaster>, fut: F) -> T
where
    F: std::future::Future<Output = T>,
{
    CURRENT_RECORDING_BROADCASTER.scope(broadcaster, fut).await
}

/// Snapshot the current task's recording broadcaster, if any.
///
/// Returns `None` when called outside of a `with_recording_broadcaster`
/// scope (e.g. on the web server's request-handling tasks, where emits go
/// through the long-lived broadcaster directly).
pub fn current_recording_broadcaster() -> Option<Arc<EventBroadcaster>> {
    CURRENT_RECORDING_BROADCASTER.try_with(|b| b.clone()).ok()
}

/// Run `fut` with `identity` as the current task's identity.
pub async fn with_task_identity<F, T>(identity: Arc<TaskIdentity>, fut: F) -> T
where
    F: std::future::Future<Output = T>,
{
    CURRENT_TASK_IDENTITY.scope(identity, fut).await
}

/// Snapshot the current task's identity, if any.
///
/// Returns `None` when called outside of a `with_task_identity` scope.
pub fn current_task_identity() -> Option<Arc<TaskIdentity>> {
    CURRENT_TASK_IDENTITY.try_with(|i| i.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_none_outside_scope() {
        assert!(current_recording_broadcaster().is_none());
    }

    #[tokio::test]
    async fn returns_broadcaster_inside_scope() {
        let b = Arc::new(EventBroadcaster::new(8));
        let b_for_check = b.clone();
        with_recording_broadcaster(b, async move {
            let inside = current_recording_broadcaster().expect("should be set");
            assert!(Arc::ptr_eq(&inside, &b_for_check));
        })
        .await;
        assert!(current_recording_broadcaster().is_none());
    }

    #[tokio::test]
    async fn nested_scope_overrides_outer() {
        let outer = Arc::new(EventBroadcaster::new(8));
        let inner = Arc::new(EventBroadcaster::new(8));
        let inner_for_check = inner.clone();
        with_recording_broadcaster(outer.clone(), async move {
            with_recording_broadcaster(inner, async move {
                let seen = current_recording_broadcaster().expect("should be set");
                assert!(Arc::ptr_eq(&seen, &inner_for_check));
            })
            .await;
            // Outer still in scope.
            let seen = current_recording_broadcaster().expect("should be set");
            assert!(Arc::ptr_eq(&seen, &outer));
        })
        .await;
    }

    /// task-locals propagate across `await` (same tokio task), which is what
    /// we rely on when the reverse-RPC dispatcher runs on the caller's task.
    #[tokio::test]
    async fn propagates_across_await_chain() {
        let b = Arc::new(EventBroadcaster::new(8));
        let b_for_check = b.clone();
        with_recording_broadcaster(b, async move {
            // Yield then check — task-local survives across await boundaries
            // on the same task.
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
            let seen = current_recording_broadcaster().expect("should be set");
            assert!(Arc::ptr_eq(&seen, &b_for_check));
        })
        .await;
    }
}
