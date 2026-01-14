//! Real-time entity change event system
//!
//! This module provides a broadcast-based event system for notifying clients
//! about entity changes (books, series, libraries) and task progress in real-time via SSE.
//!
//! In distributed deployments where workers run in separate processes, the event
//! recording feature allows capturing events during task execution and replaying
//! them on the web server when tasks complete.

mod broadcaster;
mod types;

pub use broadcaster::{EventBroadcaster, RecordedEvent};
pub use types::{
    EntityChangeEvent, EntityEvent, EntityType, TaskProgressEvent, TaskStatus,
};
