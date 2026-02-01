//! Tests for the MetadataApplier service.
//!
//! These tests verify that metadata from plugins is correctly applied to series,
//! particularly focusing on title_sort behavior.

#[path = "../common/mod.rs"]
mod common;

use chrono::Utc;
use codex::db::entities::plugins;
use codex::db::entities::series_metadata;
use codex::db::repositories::{LibraryRepository, SeriesMetadataRepository, SeriesRepository};
use codex::db::ScanningStrategy;
use codex::services::metadata::{ApplyOptions, MetadataApplier};
use codex::services::plugin::PluginSeriesMetadata;
use common::db::setup_test_db;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;
use uuid::Uuid;

/// Create a test plugin with title write permission
fn create_test_plugin() -> plugins::Model {
    plugins::Model {
        id: Uuid::new_v4(),
        name: "test-plugin".to_string(),
        display_name: "Test Plugin".to_string(),
        description: None,
        plugin_type: "system".to_string(),
        command: "node".to_string(),
        args: json!([]),
        env: json!({}),
        working_directory: None,
        permissions: json!(["metadata:write:title", "metadata:write:summary"]),
        scopes: json!(["series:detail"]),
        library_ids: json!([]),
        credentials: None,
        credential_delivery: "env".to_string(),
        config: json!({}),
        manifest: None,
        enabled: true,
        health_status: "healthy".to_string(),
        failure_count: 0,
        last_failure_at: None,
        last_success_at: None,
        disabled_reason: None,
        rate_limit_requests_per_minute: None,
        search_query_template: None,
        search_preprocessing_rules: None,
        auto_match_conditions: None,
        use_existing_external_id: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        created_by: None,
        updated_by: None,
    }
}

/// Create test plugin metadata with a title
fn create_test_metadata(title: &str) -> PluginSeriesMetadata {
    PluginSeriesMetadata {
        external_id: "test-123".to_string(),
        external_url: "https://example.com/test-123".to_string(),
        title: Some(title.to_string()),
        alternate_titles: vec![],
        summary: None,
        status: None,
        year: None,
        total_book_count: None,
        language: None,
        age_rating: None,
        reading_direction: None,
        genres: vec![],
        tags: vec![],
        authors: vec![],
        artists: vec![],
        publisher: None,
        cover_url: None,
        banner_url: None,
        rating: None,
        external_ratings: vec![],
        external_links: vec![],
    }
}

// =============================================================================
// title_sort Update Tests
// =============================================================================

#[tokio::test]
async fn test_apply_title_updates_title_sort_when_not_locked() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Original Title", None)
        .await
        .unwrap();

    // Set up initial metadata with title_sort matching the title
    let metadata = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    let mut active: series_metadata::ActiveModel = metadata.into();
    active.title_sort = Set(Some("Original Title".to_string()));
    active.update(&db).await.unwrap();

    // Verify initial state
    let current = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.title, "Original Title");
    assert_eq!(current.title_sort, Some("Original Title".to_string()));
    assert!(!current.title_sort_lock);

    // Apply metadata with new title
    let plugin = create_test_plugin();
    let plugin_metadata = create_test_metadata("New Title From Plugin");
    let options = ApplyOptions::default();

    let result = MetadataApplier::apply(
        &db,
        series.id,
        library.id,
        &plugin,
        &plugin_metadata,
        Some(&current),
        &options,
    )
    .await
    .unwrap();

    // Verify title was applied
    assert!(result.applied_fields.contains(&"title".to_string()));
    assert!(
        result.applied_fields.contains(&"titleSort".to_string()),
        "titleSort should be in applied_fields when updated"
    );

    // Verify title_sort was updated to match new title
    let updated = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.title, "New Title From Plugin");
    assert_eq!(
        updated.title_sort,
        Some("New Title From Plugin".to_string()),
        "title_sort should be updated to match the new title"
    );
}

#[tokio::test]
async fn test_apply_title_preserves_title_sort_when_locked() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Original Title", None)
        .await
        .unwrap();

    // Set up initial metadata with a custom locked title_sort
    let metadata = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    let mut active: series_metadata::ActiveModel = metadata.into();
    active.title_sort = Set(Some("Custom Sort Order".to_string()));
    active.title_sort_lock = Set(true);
    active.update(&db).await.unwrap();

    // Verify initial state
    let current = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.title, "Original Title");
    assert_eq!(current.title_sort, Some("Custom Sort Order".to_string()));
    assert!(current.title_sort_lock);

    // Apply metadata with new title
    let plugin = create_test_plugin();
    let plugin_metadata = create_test_metadata("New Title From Plugin");
    let options = ApplyOptions::default();

    let result = MetadataApplier::apply(
        &db,
        series.id,
        library.id,
        &plugin,
        &plugin_metadata,
        Some(&current),
        &options,
    )
    .await
    .unwrap();

    // Verify title was applied
    assert!(result.applied_fields.contains(&"title".to_string()));
    assert!(
        !result.applied_fields.contains(&"titleSort".to_string()),
        "titleSort should NOT be in applied_fields when locked"
    );

    // Verify title_sort was preserved (locked)
    let updated = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.title, "New Title From Plugin");
    assert_eq!(
        updated.title_sort,
        Some("Custom Sort Order".to_string()),
        "title_sort should be preserved when locked"
    );
}

#[tokio::test]
async fn test_apply_title_sets_title_sort_when_none() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Original Title", None)
        .await
        .unwrap();

    // Verify initial state - title_sort should be None by default
    let current = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.title, "Original Title");
    assert!(
        current.title_sort.is_none(),
        "title_sort should be None initially"
    );

    // Apply metadata with new title
    let plugin = create_test_plugin();
    let plugin_metadata = create_test_metadata("New Title From Plugin");
    let options = ApplyOptions::default();

    let result = MetadataApplier::apply(
        &db,
        series.id,
        library.id,
        &plugin,
        &plugin_metadata,
        Some(&current),
        &options,
    )
    .await
    .unwrap();

    // Verify title was applied
    assert!(result.applied_fields.contains(&"title".to_string()));
    assert!(
        result.applied_fields.contains(&"titleSort".to_string()),
        "titleSort should be in applied_fields when set"
    );

    // Verify title_sort was set to match new title
    let updated = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.title, "New Title From Plugin");
    assert_eq!(
        updated.title_sort,
        Some("New Title From Plugin".to_string()),
        "title_sort should be set when it was None"
    );
}
