//! Shared metadata services for applying plugin metadata to series.
//!
//! This module provides a unified implementation for applying metadata from plugins,
//! used by both synchronous API endpoints and background task handlers.

mod apply;
mod cover;

pub use apply::{ApplyOptions, MetadataApplier, SkippedField};
pub use cover::CoverService;
