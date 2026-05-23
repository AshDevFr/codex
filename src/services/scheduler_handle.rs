//! Trait abstraction for the cron scheduler.
//!
//! `services` needs a way to ask the scheduler to recompute its release-source
//! jobs after a write to `release_sources`, but the concrete scheduler lives
//! above `services` in the layering. This trait inverts that dependency:
//! `services` depends on `SchedulerReconciler`, and the `scheduler` module
//! provides the implementation.

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;

/// Anything the services layer can ask the scheduler to do.
///
/// The only operation services needs today is "reconcile release-source
/// schedules"; if that grows we'll add methods here rather than handing out
/// the full `Scheduler` type. The method returns a `BoxFuture` so the trait
/// stays object-safe without dragging in `async-trait`.
pub trait SchedulerReconciler: Send + Sync {
    /// Reload the release-source poll schedule from the database. Called
    /// after writes to `release_sources` (e.g. when a plugin re-registers
    /// its sources) so the scheduler picks up enable/disable + cron changes
    /// without a restart.
    fn reconcile_release_sources(&self) -> BoxFuture<'_, Result<()>>;
}

/// Type alias used everywhere services-side code holds the handle.
pub type SharedSchedulerReconciler = Arc<dyn SchedulerReconciler>;
