//! API v1 Data Transfer Objects
//!
//! This module contains all DTOs for API v1 request/response serialization.

pub mod api_key;
pub mod auth;
pub mod book;
pub mod bulk_metadata;
pub mod cleanup;
pub mod common;
pub mod duplicates;
pub mod filter;
pub mod info;
pub mod library;
pub mod library_jobs;
pub mod metrics;
pub mod oidc;
pub mod page;
pub mod patch;
pub mod pdf_cache;
pub mod plugin_storage;
pub mod plugins;
pub mod read_progress;
pub mod recommendations;
pub mod scan;
pub mod series;
pub mod series_export;
pub mod settings;
pub mod setup;
pub mod sharing_tag;
pub mod task_metrics;
pub mod user;
pub mod user_plugins;
pub mod user_preferences;

pub use api_key::*;
pub use auth::*;
pub use book::*;
#[allow(unused_imports)]
pub use bulk_metadata::*;
pub use cleanup::*;
pub use common::*;
pub use duplicates::*;
pub use filter::*;
pub use info::*;
pub use library::*;
pub use library_jobs::*;
pub use metrics::*;
pub use oidc::*;
pub use page::*;
pub use pdf_cache::*;
pub use plugin_storage::*;
pub use plugins::*;
pub use read_progress::*;
#[allow(unused_imports)]
pub use recommendations::*;
pub use scan::*;
pub use series::*;
#[allow(unused_imports)]
pub use series_export::*;
pub use settings::*;
pub use setup::*;
pub use sharing_tag::*;
pub use task_metrics::*;
pub use user::*;
#[allow(unused_imports)]
pub use user_plugins::*;
pub use user_preferences::*;
