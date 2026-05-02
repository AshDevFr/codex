//! Tests for BookMetadataApplier — Phase 12 of metadata-count-split focus on
//! the new per-book volume / chapter write blocks. Uses the same plugin-permission
//! shape as the series-side `metadata_apply.rs` tests for consistency.

#[path = "../common/mod.rs"]
mod common;

use chrono::Utc;
use codex::db::ScanningStrategy;
use codex::db::entities::plugins;
use codex::db::repositories::{
    BookMetadataRepository, BookRepository, LibraryRepository, SeriesRepository,
};
use codex::services::metadata::{BookApplyOptions, BookMetadataApplier};
use codex::services::plugin::protocol::PluginBookMetadata;
use common::db::setup_test_db;
use common::fixtures::create_test_book;
use serde_json::json;
use std::collections::HashSet;
use uuid::Uuid;

fn create_plugin_with_permissions(permissions: &[&str]) -> plugins::Model {
    plugins::Model {
        id: Uuid::new_v4(),
        name: "test-plugin-book".to_string(),
        display_name: "Test Plugin Book".to_string(),
        description: None,
        plugin_type: "system".to_string(),
        command: "node".to_string(),
        args: json!([]),
        env: json!({}),
        working_directory: None,
        permissions: json!(permissions),
        scopes: json!(["book:detail"]),
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

fn book_metadata_with_volume_chapter(
    volume: Option<f64>,
    chapter: Option<f64>,
) -> PluginBookMetadata {
    PluginBookMetadata {
        external_id: "book-1".to_string(),
        external_url: "https://example.com/book-1".to_string(),
        title: None,
        subtitle: None,
        alternate_titles: vec![],
        summary: None,
        book_type: None,
        volume,
        chapter,
        page_count: None,
        release_date: None,
        year: None,
        isbn: None,
        isbns: vec![],
        edition: None,
        original_title: None,
        original_year: None,
        translator: None,
        language: None,
        series_position: None,
        series_total: None,
        genres: vec![],
        tags: vec![],
        subjects: vec![],
        authors: vec![],
        artists: vec![],
        publisher: None,
        cover_url: None,
        covers: vec![],
        rating: None,
        external_ratings: vec![],
        awards: vec![],
        external_links: vec![],
        external_ids: vec![],
    }
}

async fn setup_book(db: &sea_orm::DatabaseConnection) -> codex::db::entities::book_metadata::Model {
    let library = LibraryRepository::create(db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(db, library.id, "Series", None)
        .await
        .unwrap();
    let book = create_test_book(
        series.id,
        library.id,
        "/lib/Series v01.cbz",
        "Series v01.cbz",
        "hash",
        "cbz",
        10,
    );
    BookRepository::create(db, &book, None).await.unwrap();
    BookMetadataRepository::create_with_title_and_number(db, book.id, None, None)
        .await
        .unwrap()
}

#[tokio::test]
async fn test_apply_book_volume_writes_value() {
    let (db, _temp_dir) = setup_test_db().await;
    let current = setup_book(&db).await;

    let plugin = create_plugin_with_permissions(&["metadata:write:volume"]);
    let result = BookMetadataApplier::apply(
        &db,
        current.book_id,
        &plugin,
        &book_metadata_with_volume_chapter(Some(7.0), None),
        Some(&current),
        &BookApplyOptions::default(),
    )
    .await
    .unwrap();

    assert!(
        result.applied_fields.contains(&"volume".to_string()),
        "volume should be applied (got applied={:?}, skipped={:?})",
        result.applied_fields,
        result.skipped_fields
    );
    let updated = BookMetadataRepository::get_by_book_id(&db, current.book_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.volume, Some(7));
    assert!(updated.chapter.is_none());
}

#[tokio::test]
async fn test_apply_book_chapter_writes_fractional_value() {
    let (db, _temp_dir) = setup_test_db().await;
    let current = setup_book(&db).await;

    let plugin = create_plugin_with_permissions(&["metadata:write:chapter"]);
    let result = BookMetadataApplier::apply(
        &db,
        current.book_id,
        &plugin,
        &book_metadata_with_volume_chapter(None, Some(47.5)),
        Some(&current),
        &BookApplyOptions::default(),
    )
    .await
    .unwrap();

    assert!(result.applied_fields.contains(&"chapter".to_string()));
    let updated = BookMetadataRepository::get_by_book_id(&db, current.book_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.chapter, Some(47.5));
    assert!(updated.volume.is_none());
}

#[tokio::test]
async fn test_apply_book_volume_skipped_when_locked() {
    let (db, _temp_dir) = setup_test_db().await;
    let current = setup_book(&db).await;

    BookMetadataRepository::set_lock(&db, current.book_id, "volume", true)
        .await
        .unwrap();
    let locked = BookMetadataRepository::get_by_book_id(&db, current.book_id)
        .await
        .unwrap()
        .unwrap();

    let plugin = create_plugin_with_permissions(&["metadata:write:volume"]);
    let result = BookMetadataApplier::apply(
        &db,
        current.book_id,
        &plugin,
        &book_metadata_with_volume_chapter(Some(7.0), None),
        Some(&locked),
        &BookApplyOptions::default(),
    )
    .await
    .unwrap();

    assert!(!result.applied_fields.contains(&"volume".to_string()));
    let skipped = result
        .skipped_fields
        .iter()
        .find(|s| s.field == "volume")
        .expect("volume must be in skipped");
    assert!(skipped.reason.contains("locked"));

    let updated = BookMetadataRepository::get_by_book_id(&db, current.book_id)
        .await
        .unwrap()
        .unwrap();
    assert!(updated.volume.is_none(), "locked volume must stay null");
}

#[tokio::test]
async fn test_apply_book_chapter_skipped_when_permission_missing() {
    let (db, _temp_dir) = setup_test_db().await;
    let current = setup_book(&db).await;

    // Plugin has volume permission but NOT chapter — chapter must be skipped.
    let plugin = create_plugin_with_permissions(&["metadata:write:volume"]);
    let result = BookMetadataApplier::apply(
        &db,
        current.book_id,
        &plugin,
        &book_metadata_with_volume_chapter(Some(7.0), Some(42.0)),
        Some(&current),
        &BookApplyOptions::default(),
    )
    .await
    .unwrap();

    assert!(result.applied_fields.contains(&"volume".to_string()));
    assert!(!result.applied_fields.contains(&"chapter".to_string()));
    let skipped = result
        .skipped_fields
        .iter()
        .find(|s| s.field == "chapter")
        .expect("chapter must be in skipped");
    assert!(skipped.reason.contains("permission"));
}

#[tokio::test]
async fn test_apply_book_fractional_volume_rejected() {
    let (db, _temp_dir) = setup_test_db().await;
    let current = setup_book(&db).await;

    let plugin = create_plugin_with_permissions(&["metadata:write:volume"]);
    let result = BookMetadataApplier::apply(
        &db,
        current.book_id,
        &plugin,
        &book_metadata_with_volume_chapter(Some(1.5), None),
        Some(&current),
        &BookApplyOptions::default(),
    )
    .await
    .unwrap();

    assert!(!result.applied_fields.contains(&"volume".to_string()));
    let skipped = result
        .skipped_fields
        .iter()
        .find(|s| s.field == "volume")
        .expect("volume must be skipped for fractional");
    assert!(
        skipped.reason.to_lowercase().contains("fractional"),
        "reason should mention fractional rejection: {}",
        skipped.reason
    );
}

#[tokio::test]
async fn test_apply_book_volume_chapter_filtered_by_allowlist() {
    let (db, _temp_dir) = setup_test_db().await;
    let current = setup_book(&db).await;

    let plugin =
        create_plugin_with_permissions(&["metadata:write:volume", "metadata:write:chapter"]);
    // Allowlist: only chapter — volume should not be touched even though
    // permission + value are present.
    let mut filter = HashSet::new();
    filter.insert("chapter".to_string());
    let options = BookApplyOptions {
        fields_filter: Some(filter),
        ..BookApplyOptions::default()
    };
    let result = BookMetadataApplier::apply(
        &db,
        current.book_id,
        &plugin,
        &book_metadata_with_volume_chapter(Some(7.0), Some(42.0)),
        Some(&current),
        &options,
    )
    .await
    .unwrap();

    assert!(result.applied_fields.contains(&"chapter".to_string()));
    assert!(!result.applied_fields.contains(&"volume".to_string()));
    let updated = BookMetadataRepository::get_by_book_id(&db, current.book_id)
        .await
        .unwrap()
        .unwrap();
    assert!(updated.volume.is_none());
    assert_eq!(updated.chapter, Some(42.0));
}
