// Re-exports of workspace crates so existing `codex::<module>::*` paths used
// pervasively in integration tests keep resolving without churn.
pub use codex_api as api;
pub use codex_api::observability;
pub use codex_api::web;
pub use codex_config as config;
pub use codex_db as db;
pub use codex_events as events;
pub use codex_migrate as migrate;
pub use codex_models as models;
pub use codex_parsers as parsers;
pub use codex_scanner as scanner;
pub use codex_scheduler as scheduler;
pub use codex_search as search;
pub use codex_services as services;
pub use codex_tasks as tasks;
pub use codex_utils as utils;
