pub mod api;
pub mod db;
pub mod models;
pub mod observability;
pub mod parsers;
pub mod scanner;
pub mod scheduler;
pub mod search;
pub mod services;
pub mod tasks;
pub mod utils;
pub mod web;

// Re-exports of workspace-leaf crates so existing `codex::config::*` and
// `codex::events::*` paths (used pervasively in integration tests) keep
// resolving without churn.
pub use codex_config as config;
pub use codex_events as events;
