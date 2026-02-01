//! `SeaORM` Entity definitions for the Codex database

pub mod prelude;

// Re-export common enums
pub use series_metadata::SeriesStatus;

// Core entities
pub mod api_keys;
pub mod book_duplicates;
pub mod book_error;
pub mod book_metadata;
pub mod books;
pub mod email_verification_tokens;
pub mod libraries;
pub mod metadata_sources;
pub mod pages;
pub mod plugin_failures;
pub mod plugins;
pub mod read_progress;
pub mod series;
pub mod settings;
pub mod settings_history;
pub mod task_metrics;
pub mod tasks;
pub mod users;

// Series metadata enhancement entities
pub mod genres;
pub mod series_alternate_titles;
pub mod series_covers;
pub mod series_external_ids;
pub mod series_external_links;
pub mod series_external_ratings;
pub mod series_genres;
pub mod series_metadata;
pub mod series_tags;
pub mod tags;
pub mod user_preferences;
pub mod user_series_ratings;

// Sharing tags for content access control
pub mod series_sharing_tags;
pub mod sharing_tags;
pub mod user_sharing_tags;
