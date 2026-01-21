pub use sea_orm_migration::prelude::*;

// Core tables
mod m20260103_000001_create_libraries;
pub mod m20260103_000002_create_users;
pub mod m20260103_000003_create_series;
mod m20260103_000004_create_books;
mod m20260103_000005_create_pages;

// Series metadata enhancement tables
mod m20260103_000006_create_series_metadata;
mod m20260103_000007_create_genres;
mod m20260103_000008_create_tags;
mod m20260103_000009_create_series_alternate_titles;
mod m20260103_000010_create_series_external_ratings;
mod m20260103_000011_create_series_external_links;
mod m20260103_000012_create_series_covers;
mod m20260103_000013_create_user_series_ratings;

// Book metadata and metadata sources
mod m20260103_000014_create_book_metadata;
mod m20260103_000015_create_metadata_sources;

// User-related tables
mod m20260103_000016_create_read_progress;
mod m20260103_000017_create_api_keys;
mod m20260103_000018_create_email_verification_tokens;

// Background tasks and settings
mod m20260106_000019_create_tasks;
mod m20260107_000020_create_settings;
mod m20260107_000021_create_settings_history;
mod m20260107_000022_seed_settings;

// Additional features
mod m20260108_000023_create_book_duplicates;
mod m20260109_000024_create_task_notification_trigger;

// Task metrics
mod m20260111_000025_create_task_metrics;
mod m20260111_000026_seed_metrics_settings;

// User preferences and integrations
mod m20260112_000027_create_user_preferences;
mod m20260112_000028_create_system_integrations;
mod m20260112_000029_create_user_integrations;

// Sharing tags for content access control
mod m20260120_000030_create_sharing_tags;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            // Core tables
            Box::new(m20260103_000001_create_libraries::Migration),
            Box::new(m20260103_000002_create_users::Migration),
            Box::new(m20260103_000003_create_series::Migration),
            Box::new(m20260103_000004_create_books::Migration),
            Box::new(m20260103_000005_create_pages::Migration),
            // Series metadata enhancement tables
            Box::new(m20260103_000006_create_series_metadata::Migration),
            Box::new(m20260103_000007_create_genres::Migration),
            Box::new(m20260103_000008_create_tags::Migration),
            Box::new(m20260103_000009_create_series_alternate_titles::Migration),
            Box::new(m20260103_000010_create_series_external_ratings::Migration),
            Box::new(m20260103_000011_create_series_external_links::Migration),
            Box::new(m20260103_000012_create_series_covers::Migration),
            Box::new(m20260103_000013_create_user_series_ratings::Migration),
            // Book metadata and metadata sources
            Box::new(m20260103_000014_create_book_metadata::Migration),
            Box::new(m20260103_000015_create_metadata_sources::Migration),
            // User-related tables
            Box::new(m20260103_000016_create_read_progress::Migration),
            Box::new(m20260103_000017_create_api_keys::Migration),
            Box::new(m20260103_000018_create_email_verification_tokens::Migration),
            // Background tasks and settings
            Box::new(m20260106_000019_create_tasks::Migration),
            Box::new(m20260107_000020_create_settings::Migration),
            Box::new(m20260107_000021_create_settings_history::Migration),
            Box::new(m20260107_000022_seed_settings::Migration),
            // Additional features
            Box::new(m20260108_000023_create_book_duplicates::Migration),
            Box::new(m20260109_000024_create_task_notification_trigger::Migration),
            // Task metrics
            Box::new(m20260111_000025_create_task_metrics::Migration),
            Box::new(m20260111_000026_seed_metrics_settings::Migration),
            // User preferences and integrations
            Box::new(m20260112_000027_create_user_preferences::Migration),
            Box::new(m20260112_000028_create_system_integrations::Migration),
            Box::new(m20260112_000029_create_user_integrations::Migration),
            // Sharing tags for content access control
            Box::new(m20260120_000030_create_sharing_tags::Migration),
        ]
    }
}
