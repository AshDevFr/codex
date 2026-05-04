//! Shared metadata services for applying plugin metadata to series and books.
//!
//! This module provides:
//! - A unified implementation for applying metadata from plugins
//! - Preprocessing utilities for title cleaning and search query customization
//! - Condition evaluation for controlling auto-match behavior
//!
//! Used by both synchronous API endpoints and background task handlers.

mod apply;
mod book_apply;
mod cover;
pub mod field_groups;
pub mod preprocessing;
pub mod refresh_planner;

pub use apply::{ApplyOptions, MatchingStrategy, MetadataApplier, SkippedField};
pub use book_apply::{BookApplyOptions, BookMetadataApplier};
pub use cover::CoverService;
pub use field_groups::{FieldGroup, fields_for_group};
#[allow(unused_imports)]
pub use refresh_planner::{
    PlanFailure, PlannedRefresh, RefreshPlan, RefreshPlanner, SkipReason, SkippedRefresh,
    fields_filter_from_job_config,
};
