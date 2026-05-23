//! Real-time entity change event system.
//!
//! Provides a broadcast-based event system for notifying clients about entity
//! changes (books, series, libraries) and task progress in real-time via SSE.
//!
//! In distributed deployments where workers run in separate processes, the
//! event recording feature allows capturing events during task execution and
//! replaying them on the web server when tasks complete.
//!
//! Extracted from the monolithic `codex` crate as a workspace leaf. Carries no
//! dependencies on other Codex crates — event payloads use primitive fields
//! rather than db-entity types so the events crate can sit below `codex-db`
//! in the dep graph.

mod broadcaster;
mod task_context;
mod types;

pub use broadcaster::{EventBroadcaster, RecordedEvent};
pub use task_context::{
    TaskIdentity, current_recording_broadcaster, current_task_identity, with_recording_broadcaster,
    with_task_identity,
};
// TaskProgress is part of the public API for task progress reporting
#[allow(unused_imports)]
pub use types::{
    EntityChangeEvent, EntityEvent, EntityType, TaskProgress, TaskProgressEvent, TaskStatus,
};
