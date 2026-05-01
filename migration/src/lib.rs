pub use sea_orm_migration::prelude::*;

// Core tables
mod m20260103_000001_create_libraries;
pub mod m20260103_000002_create_users;
pub mod m20260103_000003_create_series;
mod m20260103_000004_create_books;
mod m20260103_000005_create_pages;

// Series metadata enhancement tables
pub mod m20260103_000006_create_series_metadata;
mod m20260103_000007_create_genres;
mod m20260103_000008_create_tags;
mod m20260103_000009_create_series_alternate_titles;
mod m20260103_000010_create_series_external_ratings;
mod m20260103_000011_create_series_external_links;
mod m20260103_000012_create_series_covers;
mod m20260103_000013_create_user_series_ratings;

// Book metadata and metadata sources
pub mod m20260103_000014_create_book_metadata;
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

// User preferences
mod m20260112_000027_create_user_preferences;

// Sharing tags for content access control
mod m20260120_000030_create_sharing_tags;

// Performance indexes for foreign keys
mod m20260121_000031_add_missing_fk_indexes;

// Sorting indexes for efficient ORDER BY operations
mod m20260122_000032_add_sorting_indexes;

// PDF cache cleanup settings
mod m20260122_000033_seed_pdf_cache_settings;

// Add analysis_errors column to books table
mod m20260123_000034_add_analysis_errors_column;

// Plugin system
mod m20260127_000035_create_plugins;
mod m20260129_000036_seed_plugin_settings;

// Thumbnail cron settings
mod m20260130_000037_seed_thumbnail_cron_settings;

// Update validation_rules for settings UI hints
mod m20260130_000038_update_settings_validation_rules;

// Metadata plugin extended features
mod m20260131_000039_create_series_external_ids;
mod m20260131_000040_add_library_preprocessing;
mod m20260131_000041_add_plugin_search_config;

// Cover lock feature
mod m20260201_000042_add_cover_lock;

// Rate-limited task reschedule support
mod m20260202_000043_add_task_reschedule_count;

// Book metadata expansion (Phase 1)
mod m20260202_000044_book_metadata_expansion;
mod m20260202_000046_create_book_external_ids;
mod m20260202_000047_create_book_covers;

// Book external links (mirrors series_external_links)
mod m20260203_000048_create_book_external_links;

// Remove web field from book_metadata (now uses book_external_links)
mod m20260203_000049_remove_book_metadata_web;

// Plugin metadata targets configuration
mod m20260203_000050_add_plugin_metadata_targets;

// OIDC authentication
pub mod m20260205_000051_create_oidc_connections;

// User plugin system (per-user plugin instances and data storage)
mod m20260205_000052_create_user_plugins;

// Plugin task timeout setting
mod m20260211_000053_seed_plugin_task_timeout;

// Plugin internal config (server-side per-plugin settings)
mod m20260211_000054_add_plugin_internal_config;

// Book genres and tags junction tables (shared taxonomy with series)
mod m20260214_000055_create_book_genres_tags;

// Consolidate individual author columns into authors_json
mod m20260215_000056_consolidate_authors;

// Alternate titles lock (independent from title_lock)
mod m20260217_000057_add_alternate_titles_lock;

// Remove prioritize_scans setting (priority now baked into TaskType::default_priority())
mod m20260220_000058_remove_prioritize_scans_setting;

// Add search_title column for accent-insensitive search
mod m20260222_000059_add_search_title;

// Add koreader_hash column for KOReader sync
mod m20260309_000060_add_koreader_hash;

// Add r2_progression column for Readium/OPDS 2.0 EPUB progress sync
mod m20260314_000061_add_r2_progression;

// Add epub_positions column for Readium positions list (cross-app sync)
mod m20260315_000062_add_epub_positions;
mod m20260316_000063_add_epub_spine_items;

// Series data export
mod m20260408_000064_create_series_exports;
mod m20260408_000065_seed_series_export_settings;
mod m20260410_000066_add_export_type;

// Split series_metadata.total_book_count into total_volume_count and total_chapter_count
mod m20260502_000067_split_book_count;
// Drop legacy series_metadata.total_book_count and lock columns (Phase 9 hard removal)
mod m20260502_000068_drop_book_count;
// Add chapter + chapter_lock columns to book_metadata (Phase 11 per-book classification)
mod m20260503_000069_add_book_chapter;
// Backfill volume/chapter from filename for already-scanned books (Phase 12)
mod m20260503_000070_backfill_book_volume_chapter;
// Library jobs table for scheduled work (Phase 9 of scheduled-metadata-refresh).
// Filename retains the original Phase 1 name for git-history continuity; module
// now creates the generic `library_jobs` table instead of adding a JSON column.
mod m20260503_000071_add_metadata_refresh_config;
// Release tracking (Phase 1): series_tracking sidecar + series_aliases
mod m20260503_000072_create_release_tracking;
// Release tracking (Phase 2): release_sources + release_ledger
mod m20260503_000073_create_release_ledger;

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
            // User preferences
            Box::new(m20260112_000027_create_user_preferences::Migration),
            // Sharing tags for content access control
            Box::new(m20260120_000030_create_sharing_tags::Migration),
            // Performance indexes for foreign keys
            Box::new(m20260121_000031_add_missing_fk_indexes::Migration),
            // Sorting indexes for efficient ORDER BY operations
            Box::new(m20260122_000032_add_sorting_indexes::Migration),
            // PDF cache cleanup settings
            Box::new(m20260122_000033_seed_pdf_cache_settings::Migration),
            // Add analysis_errors column to books table
            Box::new(m20260123_000034_add_analysis_errors_column::Migration),
            // Plugin system
            Box::new(m20260127_000035_create_plugins::Migration),
            Box::new(m20260129_000036_seed_plugin_settings::Migration),
            // Thumbnail cron settings
            Box::new(m20260130_000037_seed_thumbnail_cron_settings::Migration),
            // Update validation_rules for settings UI hints
            Box::new(m20260130_000038_update_settings_validation_rules::Migration),
            // Metadata plugin extended features
            Box::new(m20260131_000039_create_series_external_ids::Migration),
            Box::new(m20260131_000040_add_library_preprocessing::Migration),
            Box::new(m20260131_000041_add_plugin_search_config::Migration),
            // Cover lock feature
            Box::new(m20260201_000042_add_cover_lock::Migration),
            // Rate-limited task reschedule support
            Box::new(m20260202_000043_add_task_reschedule_count::Migration),
            // Book metadata expansion (Phase 1)
            Box::new(m20260202_000044_book_metadata_expansion::Migration),
            Box::new(m20260202_000046_create_book_external_ids::Migration),
            Box::new(m20260202_000047_create_book_covers::Migration),
            // Book external links
            Box::new(m20260203_000048_create_book_external_links::Migration),
            // Remove web field from book_metadata
            Box::new(m20260203_000049_remove_book_metadata_web::Migration),
            // Plugin metadata targets configuration
            Box::new(m20260203_000050_add_plugin_metadata_targets::Migration),
            // OIDC authentication
            Box::new(m20260205_000051_create_oidc_connections::Migration),
            // User plugin system
            Box::new(m20260205_000052_create_user_plugins::Migration),
            // Plugin task timeout setting
            Box::new(m20260211_000053_seed_plugin_task_timeout::Migration),
            // Plugin internal config (server-side per-plugin settings)
            Box::new(m20260211_000054_add_plugin_internal_config::Migration),
            // Book genres and tags junction tables
            Box::new(m20260214_000055_create_book_genres_tags::Migration),
            // Consolidate individual author columns into authors_json
            Box::new(m20260215_000056_consolidate_authors::Migration),
            // Alternate titles lock (independent from title_lock)
            Box::new(m20260217_000057_add_alternate_titles_lock::Migration),
            // Remove prioritize_scans setting
            Box::new(m20260220_000058_remove_prioritize_scans_setting::Migration),
            // Add search_title for accent-insensitive search
            Box::new(m20260222_000059_add_search_title::Migration),
            // Add koreader_hash for KOReader sync
            Box::new(m20260309_000060_add_koreader_hash::Migration),
            // Add r2_progression for Readium EPUB progress sync
            Box::new(m20260314_000061_add_r2_progression::Migration),
            // Add epub_positions for Readium positions list (cross-app sync)
            Box::new(m20260315_000062_add_epub_positions::Migration),
            // Add epub_spine_items for char/byte position normalization (cross-device sync)
            Box::new(m20260316_000063_add_epub_spine_items::Migration),
            // Series data export
            Box::new(m20260408_000064_create_series_exports::Migration),
            Box::new(m20260408_000065_seed_series_export_settings::Migration),
            Box::new(m20260410_000066_add_export_type::Migration),
            // Split total_book_count into total_volume_count and total_chapter_count
            Box::new(m20260502_000067_split_book_count::Migration),
            // Drop legacy total_book_count column and lock (Phase 9 hard removal)
            Box::new(m20260502_000068_drop_book_count::Migration),
            // Add chapter + chapter_lock columns to book_metadata (Phase 11)
            Box::new(m20260503_000069_add_book_chapter::Migration),
            // Backfill book_metadata.volume / .chapter from filename (Phase 12)
            Box::new(m20260503_000070_backfill_book_volume_chapter::Migration),
            // Per-library scheduled metadata refresh config (Phase 1)
            Box::new(m20260503_000071_add_metadata_refresh_config::Migration),
            // Release tracking (Phase 1)
            Box::new(m20260503_000072_create_release_tracking::Migration),
            // Release tracking (Phase 2)
            Box::new(m20260503_000073_create_release_ledger::Migration),
        ]
    }
}
