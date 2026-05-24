//! Library jobs service: type-discriminated configs for the
//! [`library_jobs`] table.
//!
//! This module owns the typed shape of the per-job `config` JSON payload.
//! The repository layer ([`codex_db::repositories::LibraryJobRepository`])
//! persists strings; the parsing, default-filling, and validation lives here.
//!
//! Currently the `metadata_refresh` type is supported. Future job types extend
//! [`LibraryJobConfig`] without schema changes.
//!
//! [`library_jobs`]: codex_db::entities::library_jobs

pub mod types;
pub mod validation;

#[allow(unused_imports)]
pub use types::{
    LibraryJobConfig, LibraryJobType, MetadataRefreshJobConfig, RefreshScope, parse_job_config,
};
#[allow(unused_imports)]
pub use validation::{ValidationError, validate_metadata_refresh_config};
