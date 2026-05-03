//! Tests for the MetadataApplier service.
//!
//! These tests verify that metadata from plugins is correctly applied to series,
//! particularly focusing on title_sort behavior.

#[path = "../common/mod.rs"]
mod common;

use chrono::Utc;
use codex::db::ScanningStrategy;
use codex::db::entities::plugins;
use codex::db::entities::series_metadata;
use codex::db::repositories::{LibraryRepository, SeriesMetadataRepository, SeriesRepository};
use codex::services::metadata::{ApplyOptions, MetadataApplier};
use codex::services::plugin::PluginSeriesMetadata;
use common::db::setup_test_db;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;
use std::collections::HashSet;
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
        metadata_targets: None,
        internal_config: None,
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
        total_volume_count: None,
        total_chapter_count: None,
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
        external_ids: vec![],
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

// =============================================================================
// total_volume_count / total_chapter_count Apply Tests (Phase 3)
// =============================================================================

/// Build a plugin with the given permission strings (e.g. "metadata:write:total_volume_count").
fn create_plugin_with_permissions(permissions: &[&str]) -> plugins::Model {
    plugins::Model {
        id: Uuid::new_v4(),
        name: "test-plugin-counts".to_string(),
        display_name: "Test Plugin Counts".to_string(),
        description: None,
        plugin_type: "system".to_string(),
        command: "node".to_string(),
        args: json!([]),
        env: json!({}),
        working_directory: None,
        permissions: json!(permissions),
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
        metadata_targets: None,
        internal_config: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        created_by: None,
        updated_by: None,
    }
}

fn metadata_with_counts(
    total_volume_count: Option<i32>,
    total_chapter_count: Option<f32>,
) -> PluginSeriesMetadata {
    PluginSeriesMetadata {
        external_id: "counts-1".to_string(),
        external_url: "https://example.com/counts-1".to_string(),
        title: None,
        alternate_titles: vec![],
        summary: None,
        status: None,
        year: None,
        total_volume_count,
        total_chapter_count,
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
        external_ids: vec![],
    }
}

#[tokio::test]
async fn test_apply_total_volume_count_writes_value() {
    let (db, _temp_dir) = setup_test_db().await;
    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Series", None)
        .await
        .unwrap();
    let current = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();

    let plugin = create_plugin_with_permissions(&["metadata:write:total_volume_count"]);
    let result = MetadataApplier::apply(
        &db,
        series.id,
        library.id,
        &plugin,
        &metadata_with_counts(Some(14), None),
        Some(&current),
        &ApplyOptions::default(),
    )
    .await
    .unwrap();

    assert!(
        result
            .applied_fields
            .contains(&"totalVolumeCount".to_string()),
        "totalVolumeCount should be applied"
    );
    let updated = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.total_volume_count, Some(14));
    assert!(updated.total_chapter_count.is_none());
}

#[tokio::test]
async fn test_apply_total_chapter_count_writes_fractional_value() {
    let (db, _temp_dir) = setup_test_db().await;
    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Series", None)
        .await
        .unwrap();
    let current = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();

    let plugin = create_plugin_with_permissions(&["metadata:write:total_chapter_count"]);
    let result = MetadataApplier::apply(
        &db,
        series.id,
        library.id,
        &plugin,
        &metadata_with_counts(None, Some(109.5)),
        Some(&current),
        &ApplyOptions::default(),
    )
    .await
    .unwrap();

    assert!(
        result
            .applied_fields
            .contains(&"totalChapterCount".to_string()),
        "totalChapterCount should be applied"
    );
    let updated = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.total_chapter_count, Some(109.5));
    assert!(updated.total_volume_count.is_none());
}

#[tokio::test]
async fn test_apply_writes_both_count_fields_independently() {
    let (db, _temp_dir) = setup_test_db().await;
    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Series", None)
        .await
        .unwrap();
    let current = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();

    let plugin = create_plugin_with_permissions(&[
        "metadata:write:total_volume_count",
        "metadata:write:total_chapter_count",
    ]);
    let result = MetadataApplier::apply(
        &db,
        series.id,
        library.id,
        &plugin,
        &metadata_with_counts(Some(14), Some(109.0)),
        Some(&current),
        &ApplyOptions::default(),
    )
    .await
    .unwrap();

    assert!(
        result
            .applied_fields
            .contains(&"totalVolumeCount".to_string())
    );
    assert!(
        result
            .applied_fields
            .contains(&"totalChapterCount".to_string())
    );
    let updated = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.total_volume_count, Some(14));
    assert_eq!(updated.total_chapter_count, Some(109.0));
}

#[tokio::test]
async fn test_apply_total_volume_count_skipped_when_locked() {
    let (db, _temp_dir) = setup_test_db().await;
    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Series", None)
        .await
        .unwrap();
    let metadata = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    let mut active: series_metadata::ActiveModel = metadata.into();
    active.total_volume_count = Set(Some(7));
    active.total_volume_count_lock = Set(true);
    active.update(&db).await.unwrap();

    let current = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();

    let plugin = create_plugin_with_permissions(&["metadata:write:total_volume_count"]);
    let result = MetadataApplier::apply(
        &db,
        series.id,
        library.id,
        &plugin,
        &metadata_with_counts(Some(14), None),
        Some(&current),
        &ApplyOptions::default(),
    )
    .await
    .unwrap();

    assert!(
        !result
            .applied_fields
            .contains(&"totalVolumeCount".to_string()),
        "totalVolumeCount should not be applied when locked"
    );
    let skipped = result
        .skipped_fields
        .iter()
        .find(|s| s.field == "totalVolumeCount")
        .expect("totalVolumeCount should be in skipped_fields");
    assert_eq!(skipped.reason, "Field is locked");

    let after = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        after.total_volume_count,
        Some(7),
        "locked value should be preserved"
    );
}

#[tokio::test]
async fn test_apply_total_chapter_count_skipped_when_locked() {
    let (db, _temp_dir) = setup_test_db().await;
    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Series", None)
        .await
        .unwrap();
    let metadata = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    let mut active: series_metadata::ActiveModel = metadata.into();
    active.total_chapter_count = Set(Some(50.0));
    active.total_chapter_count_lock = Set(true);
    active.update(&db).await.unwrap();

    let current = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();

    let plugin = create_plugin_with_permissions(&["metadata:write:total_chapter_count"]);
    let result = MetadataApplier::apply(
        &db,
        series.id,
        library.id,
        &plugin,
        &metadata_with_counts(None, Some(109.5)),
        Some(&current),
        &ApplyOptions::default(),
    )
    .await
    .unwrap();

    assert!(
        !result
            .applied_fields
            .contains(&"totalChapterCount".to_string()),
        "totalChapterCount should not be applied when locked"
    );
    let skipped = result
        .skipped_fields
        .iter()
        .find(|s| s.field == "totalChapterCount")
        .expect("totalChapterCount should be in skipped_fields");
    assert_eq!(skipped.reason, "Field is locked");

    let after = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        after.total_chapter_count,
        Some(50.0),
        "locked value should be preserved"
    );
}

#[tokio::test]
async fn test_apply_count_fields_skipped_when_permission_missing() {
    let (db, _temp_dir) = setup_test_db().await;
    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Series", None)
        .await
        .unwrap();
    let current = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();

    // Plugin holds no count permissions.
    let plugin = create_plugin_with_permissions(&["metadata:write:title"]);
    let result = MetadataApplier::apply(
        &db,
        series.id,
        library.id,
        &plugin,
        &metadata_with_counts(Some(14), Some(109.5)),
        Some(&current),
        &ApplyOptions::default(),
    )
    .await
    .unwrap();

    assert!(
        !result
            .applied_fields
            .contains(&"totalVolumeCount".to_string())
    );
    assert!(
        !result
            .applied_fields
            .contains(&"totalChapterCount".to_string())
    );
    let denied: Vec<&str> = result
        .skipped_fields
        .iter()
        .filter(|s| s.field == "totalVolumeCount" || s.field == "totalChapterCount")
        .map(|s| s.reason.as_str())
        .collect();
    assert_eq!(denied.len(), 2);
    assert!(
        denied
            .iter()
            .all(|r| *r == "Plugin does not have permission")
    );

    let after = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert!(after.total_volume_count.is_none());
    assert!(after.total_chapter_count.is_none());
}

#[tokio::test]
async fn test_apply_count_fields_filtered_out_by_allowlist() {
    let (db, _temp_dir) = setup_test_db().await;
    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Series", None)
        .await
        .unwrap();
    let current = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();

    let plugin = create_plugin_with_permissions(&[
        "metadata:write:total_volume_count",
        "metadata:write:total_chapter_count",
    ]);
    // Allowlist only totalVolumeCount; totalChapterCount must not be touched.
    let mut filter = HashSet::new();
    filter.insert("totalVolumeCount".to_string());
    let options = ApplyOptions {
        fields_filter: Some(filter),
        thumbnail_service: None,
        event_broadcaster: None,
        dry_run: false,
        ..Default::default()
    };

    let result = MetadataApplier::apply(
        &db,
        series.id,
        library.id,
        &plugin,
        &metadata_with_counts(Some(14), Some(109.5)),
        Some(&current),
        &options,
    )
    .await
    .unwrap();

    assert!(
        result
            .applied_fields
            .contains(&"totalVolumeCount".to_string())
    );
    assert!(
        !result
            .applied_fields
            .contains(&"totalChapterCount".to_string()),
        "totalChapterCount should be filtered out by allowlist"
    );
    assert!(
        !result
            .skipped_fields
            .iter()
            .any(|s| s.field == "totalChapterCount"),
        "filtered-out fields should not appear in skipped_fields either"
    );

    let after = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.total_volume_count, Some(14));
    assert!(after.total_chapter_count.is_none());
}

#[tokio::test]
async fn test_apply_count_fields_skip_when_metadata_value_absent() {
    let (db, _temp_dir) = setup_test_db().await;
    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Series", None)
        .await
        .unwrap();
    let metadata = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    let mut active: series_metadata::ActiveModel = metadata.into();
    active.total_volume_count = Set(Some(3));
    active.total_chapter_count = Set(Some(42.0));
    active.update(&db).await.unwrap();
    let current = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();

    let plugin = create_plugin_with_permissions(&[
        "metadata:write:total_volume_count",
        "metadata:write:total_chapter_count",
    ]);
    // Both incoming values are None -> mirroring the existing `Some(...)`-gated
    // pattern, the apply step does not touch the columns.
    let result = MetadataApplier::apply(
        &db,
        series.id,
        library.id,
        &plugin,
        &metadata_with_counts(None, None),
        Some(&current),
        &ApplyOptions::default(),
    )
    .await
    .unwrap();

    assert!(
        !result
            .applied_fields
            .contains(&"totalVolumeCount".to_string())
    );
    assert!(
        !result
            .applied_fields
            .contains(&"totalChapterCount".to_string())
    );

    let after = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.total_volume_count, Some(3));
    assert_eq!(after.total_chapter_count, Some(42.0));
}
