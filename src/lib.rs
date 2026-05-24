pub mod api;
pub mod observability;
pub mod scheduler;
pub mod web;

// Re-exports of workspace-leaf crates so existing `codex::config::*`,
// `codex::db::*`, `codex::events::*`, `codex::models::*`, `codex::parsers::*`,
// `codex::services::*`, and `codex::utils::*` paths (used pervasively in
// integration tests) keep resolving without churn.
pub use codex_config as config;
pub use codex_db as db;
pub use codex_events as events;
pub use codex_models as models;
pub use codex_parsers as parsers;
pub use codex_scanner as scanner;
pub use codex_search as search;
pub use codex_services as services;
pub use codex_tasks as tasks;
pub use codex_utils as utils;
