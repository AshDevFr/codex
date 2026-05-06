//! Real-time entity change event system
//!
//! This module provides a broadcast-based event system for notifying clients
//! about entity changes (books, series, libraries) and task progress in real-time via SSE.
//!
//! In distributed deployments where workers run in separate processes, the event
//! recording feature allows capturing events during task execution and replaying
//! them on the web server when tasks complete.

mod broadcaster;
mod task_context;
mod types;

pub use broadcaster::{EventBroadcaster, RecordedEvent};
pub use task_context::{current_recording_broadcaster, with_recording_broadcaster};
// TaskProgress is part of the public API for task progress reporting
#[allow(unused_imports)]
pub use types::{
    EntityChangeEvent, EntityEvent, EntityType, TaskProgress, TaskProgressEvent, TaskStatus,
};
