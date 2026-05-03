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
pub mod refresh_config;
pub mod refresh_planner;
pub mod refresh_skip_reason;

pub use apply::{ApplyOptions, MatchingStrategy, MetadataApplier, SkippedField};
pub use book_apply::{BookApplyOptions, BookMetadataApplier};
pub use cover::CoverService;
#[allow(unused_imports)]
pub use field_groups::{FieldGroup, fields_for_group, fields_for_groups, group_for_field};
pub use refresh_config::{MetadataRefreshConfig, parse_metadata_refresh_config};
// Re-exported for downstream phases (HTTP PATCH endpoint, per-provider
// overrides). Allowed to be unused until those phases land.
#[allow(unused_imports)]
pub use refresh_config::{MetadataRefreshConfigPatch, ProviderOverride};
#[allow(unused_imports)]
pub use refresh_planner::{
    PlannedRefresh, RefreshPlan, RefreshPlanner, SkipReason as PlannerSkipReason, SkippedRefresh,
    fields_filter_for_provider, fields_filter_from_config,
};
#[allow(unused_imports)]
pub use refresh_skip_reason::RefreshSkipReason;
