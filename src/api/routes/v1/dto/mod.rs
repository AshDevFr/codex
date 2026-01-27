//! API v1 Data Transfer Objects
//!
//! This module contains all DTOs for API v1 request/response serialization.

pub mod api_key;
pub mod auth;
pub mod book;
pub mod cleanup;
pub mod common;
pub mod duplicates;
pub mod filter;
pub mod library;
pub mod metrics;
pub mod page;
pub mod patch;
pub mod pdf_cache;
pub mod read_progress;
pub mod scan;
pub mod series;
pub mod settings;
pub mod setup;
pub mod sharing_tag;
pub mod task_metrics;
pub mod user;
pub mod user_preferences;

pub use api_key::*;
pub use auth::*;
pub use book::*;
pub use cleanup::*;
pub use common::*;
pub use duplicates::*;
pub use filter::*;
pub use library::*;
pub use metrics::*;
pub use page::*;
pub use pdf_cache::*;
pub use read_progress::*;
pub use scan::*;
pub use series::*;
pub use settings::*;
pub use setup::*;
pub use sharing_tag::*;
pub use task_metrics::*;
pub use user::*;
pub use user_preferences::*;
