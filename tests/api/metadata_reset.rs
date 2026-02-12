//! Integration tests for series metadata reset endpoints
//!
//! Tests for:
//! - DELETE /api/v1/series/{id}/metadata — reset single series metadata
//! - POST /api/v1/series/bulk/metadata/reset — bulk reset metadata

#[path = "../common/mod.rs"]
mod common;

use codex::api::routes::v1::dto::BulkMetadataResetResponse;
use codex::api::routes::v1::dto::series::FullSeriesMetadataResponse;
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    AlternateTitleRepository, ExternalLinkRepository, ExternalRatingRepository, GenreRepository,
    LibraryRepository, SeriesMetadataRepository, SeriesRepository, TagRepository, UserRepository,
};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use sea_orm::prelude::Decimal;
use std::str::FromStr;

// Helper to create admin and token
async fn create_admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AppState,
) -> String {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();
    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

// Helper to create a series with metadata populated
async fn create_series_with_metadata(
    db: &sea_orm::DatabaseConnection,
    library_id: uuid::Uuid,
    name: &str,
) -> codex::db::entities::series::Model {
    let series = SeriesRepository::create(db, library_id, name, None)
        .await
        .unwrap();

    // Set some metadata
    SeriesMetadataRepository::update_title(
        db,
        series.id,
        format!("{} - Plugin Title", name),
        Some(format!("{} sort", name)),
    )
    .await
    .unwrap();

    SeriesMetadataRepository::update_summary(db, series.id, Some("A test summary".to_string()))
        .await
        .unwrap();

    SeriesMetadataRepository::update_publisher(
        db,
        series.id,
        Some("Test Publisher".to_string()),
        Some("Test Imprint".to_string()),
    )
    .await
    .unwrap();

    SeriesMetadataRepository::update_year(db, series.id, Some(2024))
        .await
        .unwrap();

    SeriesMetadataRepository::update_status(db, series.id, Some("ended".to_string()))
        .await
        .unwrap();

    // Lock some fields
    SeriesMetadataRepository::set_lock(db, series.id, "title", true)
        .await
        .unwrap();
    SeriesMetadataRepository::set_lock(db, series.id, "summary", true)
        .await
        .unwrap();

    // Add genres
    GenreRepository::set_genres_for_series(
        db,
        series.id,
        vec!["Action".to_string(), "Comedy".to_string()],
    )
    .await
    .unwrap();

    // Add tags
    TagRepository::set_tags_for_series(
        db,
        series.id,
        vec!["Completed".to_string(), "Favorite".to_string()],
    )
    .await
    .unwrap();

    // Add alternate titles
    AlternateTitleRepository::create(db, series.id, "Japanese", "テスト")
        .await
        .unwrap();

    // Add external rating
    ExternalRatingRepository::upsert(
        db,
        series.id,
        "mangabaka",
        Decimal::from_str("85.5").unwrap(),
        Some(1000),
    )
    .await
    .unwrap();

    // Add external link
    ExternalLinkRepository::upsert(
        db,
        series.id,
        "mangabaka",
        "https://mangabaka.dev/series/test",
        Some("12345"),
    )
    .await
    .unwrap();

    series
}

// ============================================================================
// DELETE /api/v1/series/{id}/metadata tests
// ============================================================================

#[tokio::test]
async fn test_reset_series_metadata_success() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    // Create a library and series with populated metadata
    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = create_series_with_metadata(&db, library.id, "My Manga").await;

    // Verify metadata is populated before reset
    let metadata = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(metadata.title, "My Manga - Plugin Title");
    assert!(metadata.summary.is_some());
    assert!(metadata.publisher.is_some());
    assert!(metadata.title_lock);
    assert!(metadata.summary_lock);

    // Verify genres/tags exist
    let genres = GenreRepository::get_genres_for_series(&db, series.id)
        .await
        .unwrap();
    assert_eq!(genres.len(), 2);

    let tags = TagRepository::get_tags_for_series(&db, series.id)
        .await
        .unwrap();
    assert_eq!(tags.len(), 2);

    // Reset metadata
    let request =
        delete_request_with_auth(&format!("/api/v1/series/{}/metadata", series.id), &token);
    let (status, response): (StatusCode, Option<FullSeriesMetadataResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();

    // Verify metadata is reset to defaults
    assert_eq!(body.series_id, series.id);
    assert_eq!(body.title, "My Manga"); // Reset to series.name (directory-derived)
    assert!(body.title_sort.is_none());
    assert!(body.summary.is_none());
    assert!(body.publisher.is_none());
    assert!(body.imprint.is_none());
    assert!(body.status.is_none());
    assert!(body.age_rating.is_none());
    assert!(body.language.is_none());
    assert!(body.reading_direction.is_none());
    assert!(body.year.is_none());
    assert!(body.total_book_count.is_none());
    assert!(body.custom_metadata.is_none());

    // Verify all locks are false
    assert!(!body.locks.title);
    assert!(!body.locks.title_sort);
    assert!(!body.locks.summary);
    assert!(!body.locks.publisher);
    assert!(!body.locks.imprint);
    assert!(!body.locks.status);
    assert!(!body.locks.age_rating);
    assert!(!body.locks.language);
    assert!(!body.locks.reading_direction);
    assert!(!body.locks.year);
    assert!(!body.locks.genres);
    assert!(!body.locks.tags);
    assert!(!body.locks.cover);

    // Verify related data is cleared
    assert!(body.genres.is_empty());
    assert!(body.tags.is_empty());
    assert!(body.alternate_titles.is_empty());
    assert!(body.external_ratings.is_empty());
    assert!(body.external_links.is_empty());
}

#[tokio::test]
async fn test_reset_series_metadata_clears_database_records() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = create_series_with_metadata(&db, library.id, "Test Series").await;

    // Reset metadata
    let request =
        delete_request_with_auth(&format!("/api/v1/series/{}/metadata", series.id), &token);
    let (status, _): (StatusCode, Option<FullSeriesMetadataResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Verify in database that all related data is cleared
    let genres = GenreRepository::get_genres_for_series(&db, series.id)
        .await
        .unwrap();
    assert!(genres.is_empty());

    let tags = TagRepository::get_tags_for_series(&db, series.id)
        .await
        .unwrap();
    assert!(tags.is_empty());

    let alt_titles = AlternateTitleRepository::get_for_series(&db, series.id)
        .await
        .unwrap();
    assert!(alt_titles.is_empty());

    let ext_ratings = ExternalRatingRepository::get_for_series(&db, series.id)
        .await
        .unwrap();
    assert!(ext_ratings.is_empty());

    let ext_links = ExternalLinkRepository::get_for_series(&db, series.id)
        .await
        .unwrap();
    assert!(ext_links.is_empty());

    // Verify metadata row exists with reset values
    let metadata = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(metadata.title, "Test Series");
    assert!(metadata.summary.is_none());
    assert!(!metadata.title_lock);
}

#[tokio::test]
async fn test_reset_series_metadata_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = delete_request_with_auth(&format!("/api/v1/series/{}/metadata", fake_id), &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_reset_series_metadata_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = delete_request(&format!("/api/v1/series/{}/metadata", fake_id));
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_reset_series_metadata_preserves_series_record() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = create_series_with_metadata(&db, library.id, "Preserved Series").await;

    // Reset metadata
    let request =
        delete_request_with_auth(&format!("/api/v1/series/{}/metadata", series.id), &token);
    let (status, _): (StatusCode, Option<FullSeriesMetadataResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Verify series record is preserved
    let series_after = SeriesRepository::get_by_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(series_after.id, series.id);
    assert_eq!(series_after.name, "Preserved Series");
    assert_eq!(series_after.library_id, library.id);
}

#[tokio::test]
async fn test_reset_series_metadata_idempotent() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = create_series_with_metadata(&db, library.id, "Idempotent Test").await;

    // First reset
    let app = create_test_router(state.clone()).await;
    let request =
        delete_request_with_auth(&format!("/api/v1/series/{}/metadata", series.id), &token);
    let (status, _): (StatusCode, Option<FullSeriesMetadataResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Second reset (should succeed and return same result)
    let app = create_test_router(state.clone()).await;
    let request =
        delete_request_with_auth(&format!("/api/v1/series/{}/metadata", series.id), &token);
    let (status, response): (StatusCode, Option<FullSeriesMetadataResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert_eq!(body.title, "Idempotent Test");
    assert!(body.genres.is_empty());
}

// ============================================================================
// POST /api/v1/series/bulk/metadata/reset tests
// ============================================================================

#[tokio::test]
async fn test_bulk_reset_series_metadata_success() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series1 = create_series_with_metadata(&db, library.id, "Series One").await;
    let series2 = create_series_with_metadata(&db, library.id, "Series Two").await;

    // Bulk reset
    let body = serde_json::json!({
        "seriesIds": [series1.id, series2.id]
    });
    let request = post_request_with_auth_json(
        "/api/v1/series/bulk/metadata/reset",
        &token,
        &body.to_string(),
    );
    let (status, response): (StatusCode, Option<BulkMetadataResetResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert_eq!(body.count, 2);

    // Verify both series are reset
    let meta1 = SeriesMetadataRepository::get_by_series_id(&db, series1.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(meta1.title, "Series One");
    assert!(meta1.summary.is_none());
    assert!(!meta1.title_lock);

    let meta2 = SeriesMetadataRepository::get_by_series_id(&db, series2.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(meta2.title, "Series Two");
    assert!(meta2.summary.is_none());
    assert!(!meta2.title_lock);
}

#[tokio::test]
async fn test_bulk_reset_empty_list() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let body = serde_json::json!({
        "seriesIds": []
    });
    let request = post_request_with_auth_json(
        "/api/v1/series/bulk/metadata/reset",
        &token,
        &body.to_string(),
    );
    let (status, response): (StatusCode, Option<BulkMetadataResetResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert_eq!(body.count, 0);
    assert_eq!(body.message, "No series specified");
}

#[tokio::test]
async fn test_bulk_reset_skips_nonexistent_series() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = create_series_with_metadata(&db, library.id, "Real Series").await;
    let fake_id = uuid::Uuid::new_v4();

    let body = serde_json::json!({
        "seriesIds": [series.id, fake_id]
    });
    let request = post_request_with_auth_json(
        "/api/v1/series/bulk/metadata/reset",
        &token,
        &body.to_string(),
    );
    let (status, response): (StatusCode, Option<BulkMetadataResetResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert_eq!(body.count, 1); // Only the real series was reset
}

#[tokio::test]
async fn test_bulk_reset_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;

    let body = serde_json::json!({
        "seriesIds": [uuid::Uuid::new_v4()]
    });
    let request =
        post_request_with_auth_json("/api/v1/series/bulk/metadata/reset", "", &body.to_string());
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
