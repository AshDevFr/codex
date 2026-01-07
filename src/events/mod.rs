//! Real-time entity change event system
//!
//! This module provides a broadcast-based event system for notifying clients
//! about entity changes (books, series, libraries) in real-time via SSE.

mod broadcaster;
mod types;

pub use broadcaster::EventBroadcaster;
pub use types::{EntityChangeEvent, EntityEvent, EntityType};
