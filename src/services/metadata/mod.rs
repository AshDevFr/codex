//! Shared metadata services for applying plugin metadata to series.
//!
//! This module provides:
//! - A unified implementation for applying metadata from plugins
//! - Preprocessing utilities for title cleaning and search query customization
//! - Condition evaluation for controlling auto-match behavior
//!
//! Used by both synchronous API endpoints and background task handlers.

mod apply;
mod cover;
pub mod preprocessing;

pub use apply::{ApplyOptions, MetadataApplier, SkippedField};
pub use cover::CoverService;
