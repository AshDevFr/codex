//! API v1 Handlers
//!
//! This module contains all request handlers for API v1.

pub mod api_keys;
pub mod auth;
pub mod books;
pub mod cleanup;
pub mod duplicates;
pub mod events;
pub mod filesystem;
pub mod health;
pub mod libraries;
pub mod metrics;
pub mod pages;
pub mod pdf_cache;
pub mod read_progress;
pub mod scan;
pub mod series;
pub mod settings;
pub mod setup;
pub mod sharing_tags;
pub mod system_integrations;
pub mod task_metrics;
pub mod task_queue;
pub mod user_integrations;
pub mod user_preferences;
pub mod users;

pub use auth::*;
pub use books::*;
pub use duplicates::*;
pub use events::*;
pub use filesystem::*;
pub use health::*;
pub use libraries::*;
pub use metrics::*;
pub use pages::*;
pub use read_progress::*;
pub use scan::*;
pub use series::*;
pub use users::*;
