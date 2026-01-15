//! Integration tests for series metadata locks endpoints

#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::series::{
    FullSeriesMetadataResponse, MetadataLocks, UpdateMetadataLocksRequest,
};
use codex::api::error::ErrorResponse;
use codex::db::repositories::{
    AlternateTitleRepository, ExternalLinkRepository, ExternalRatingRepository, GenreRepository,
    LibraryRepository, SeriesRepository, TagRepository, UserRepository,
};
use codex::db::ScanningStrategy;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use sea_orm::prelude::Decimal;
use std::str::FromStr;

// Helper to create admin and token
async fn create_admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> String {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();
    state
        .jwt_service
        .generate_token(created.id, created.username, created.is_admin)
        .unwrap()
}

// ============================================================================
// GET /api/v1/series/{id}/metadata/full tests
// ============================================================================

#[tokio::test]
async fn test_get_full_series_metadata() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    // Create a library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Attack on Titan", None)
        .await
        .unwrap();

    // Fetch full metadata
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/metadata/full", series.id),
        &token,
    );
    let (status, response): (StatusCode, Option<FullSeriesMetadataResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();

    assert_eq!(body.series_id, series.id);
    assert_eq!(body.title, "Attack on Titan");
    // Check locks are present and default to false
    assert!(!body.locks.title);
    assert!(!body.locks.summary);
    assert!(!body.locks.publisher);
    assert!(!body.locks.year);
    assert!(!body.locks.genres);
    assert!(!body.locks.tags);
    // Check related data arrays are present
    assert!(body.genres.is_empty());
    assert!(body.tags.is_empty());
    assert!(body.alternate_titles.is_empty());
    assert!(body.external_ratings.is_empty());
    assert!(body.external_links.is_empty());
}

#[tokio::test]
async fn test_get_full_series_metadata_with_related_data() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "One Piece", None)
        .await
        .unwrap();

    // Set up related data
    GenreRepository::add_genre_to_series(&db, series.id, "Action")
        .await
        .unwrap();
    GenreRepository::add_genre_to_series(&db, series.id, "Adventure")
        .await
        .unwrap();
    TagRepository::add_tag_to_series(&db, series.id, "Ongoing")
        .await
        .unwrap();
    AlternateTitleRepository::create(&db, series.id, "Japanese", "ワンピース")
        .await
        .unwrap();
    ExternalRatingRepository::upsert(
        &db,
        series.id,
        "myanimelist",
        Decimal::from_str("90.5").unwrap(),
        Some(50000),
    )
    .await
    .unwrap();
    ExternalLinkRepository::upsert(
        &db,
        series.id,
        "myanimelist",
        "https://myanimelist.net/manga/13",
        Some("13"),
    )
    .await
    .unwrap();

    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/metadata/full", series.id),
        &token,
    );
    let (status, response): (StatusCode, Option<FullSeriesMetadataResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();

    // Check genres
    assert_eq!(body.genres.len(), 2);

    // Check tags
    assert_eq!(body.tags.len(), 1);
    assert_eq!(body.tags[0].name, "Ongoing");

    // Check alternate titles
    assert_eq!(body.alternate_titles.len(), 1);
    assert_eq!(body.alternate_titles[0].label, "Japanese");
    assert_eq!(body.alternate_titles[0].title, "ワンピース");

    // Check external ratings
    assert_eq!(body.external_ratings.len(), 1);
    assert_eq!(body.external_ratings[0].source_name, "myanimelist");

    // Check external links
    assert_eq!(body.external_links.len(), 1);
    assert_eq!(body.external_links[0].source_name, "myanimelist");
}

#[tokio::test]
async fn test_get_full_series_metadata_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request =
        get_request_with_auth(&format!("/api/v1/series/{}/metadata/full", fake_id), &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_full_metadata_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let request = get_request(&format!("/api/v1/series/{}/metadata/full", series.id));
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// GET /api/v1/series/{id}/metadata/locks tests
// ============================================================================

#[tokio::test]
async fn test_get_metadata_locks() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/metadata/locks", series.id),
        &token,
    );
    let (status, response): (StatusCode, Option<MetadataLocks>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();

    // All locks should default to false
    assert!(!body.title);
    assert!(!body.title_sort);
    assert!(!body.summary);
    assert!(!body.publisher);
    assert!(!body.imprint);
    assert!(!body.status);
    assert!(!body.age_rating);
    assert!(!body.language);
    assert!(!body.reading_direction);
    assert!(!body.year);
    assert!(!body.genres);
    assert!(!body.tags);
    assert!(!body.custom_metadata);
}

#[tokio::test]
async fn test_get_metadata_locks_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/metadata/locks", fake_id),
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// PUT /api/v1/series/{id}/metadata/locks tests
// ============================================================================

#[tokio::test]
async fn test_update_metadata_locks() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Update some locks
    let body = UpdateMetadataLocksRequest {
        title: Some(true),
        summary: Some(true),
        year: Some(true),
        title_sort: None,
        publisher: None,
        imprint: None,
        status: None,
        age_rating: None,
        language: None,
        reading_direction: None,
        genres: None,
        tags: None,
        custom_metadata: None,
        total_book_count: None,
    };

    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata/locks", series.id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<MetadataLocks>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let locks = response.unwrap();

    assert!(locks.title);
    assert!(locks.summary);
    assert!(locks.year);
    // Other locks should remain false
    assert!(!locks.publisher);
    assert!(!locks.genres);

    // Verify changes persisted by fetching again
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/metadata/locks", series.id),
        &token,
    );
    let (status, response): (StatusCode, Option<MetadataLocks>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let locks = response.unwrap();
    assert!(locks.title);
    assert!(locks.summary);
    assert!(locks.year);
}

#[tokio::test]
async fn test_update_metadata_locks_partial() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // First set some locks
    let body = UpdateMetadataLocksRequest {
        title: Some(true),
        summary: Some(true),
        title_sort: None,
        publisher: None,
        imprint: None,
        status: None,
        age_rating: None,
        language: None,
        reading_direction: None,
        year: None,
        genres: None,
        tags: None,
        custom_metadata: None,
        total_book_count: None,
    };
    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata/locks", series.id),
        &body,
        &token,
    );
    let (status, _): (StatusCode, Option<MetadataLocks>) =
        make_json_request(app.clone(), request).await;
    assert_eq!(status, StatusCode::OK);

    // Update only one lock - others should remain unchanged
    let body = UpdateMetadataLocksRequest {
        summary: Some(false),
        title: None,
        title_sort: None,
        publisher: None,
        imprint: None,
        status: None,
        age_rating: None,
        language: None,
        reading_direction: None,
        year: None,
        genres: None,
        tags: None,
        custom_metadata: None,
        total_book_count: None,
    };
    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata/locks", series.id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<MetadataLocks>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let locks = response.unwrap();

    // title should still be true
    assert!(locks.title);
    // summary was explicitly set to false
    assert!(!locks.summary);
}

#[tokio::test]
async fn test_update_metadata_locks_all_fields() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Lock all fields
    let body = UpdateMetadataLocksRequest {
        title: Some(true),
        title_sort: Some(true),
        summary: Some(true),
        publisher: Some(true),
        imprint: Some(true),
        status: Some(true),
        age_rating: Some(true),
        language: Some(true),
        reading_direction: Some(true),
        year: Some(true),
        genres: Some(true),
        tags: Some(true),
        custom_metadata: Some(true),
        total_book_count: Some(true),
    };
    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata/locks", series.id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<MetadataLocks>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let locks = response.unwrap();

    assert!(locks.title);
    assert!(locks.title_sort);
    assert!(locks.summary);
    assert!(locks.publisher);
    assert!(locks.imprint);
    assert!(locks.status);
    assert!(locks.age_rating);
    assert!(locks.language);
    assert!(locks.reading_direction);
    assert!(locks.year);
    assert!(locks.genres);
    assert!(locks.tags);
    assert!(locks.custom_metadata);
}

#[tokio::test]
async fn test_update_metadata_locks_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let body = UpdateMetadataLocksRequest {
        title: Some(true),
        title_sort: None,
        summary: None,
        publisher: None,
        imprint: None,
        status: None,
        age_rating: None,
        language: None,
        reading_direction: None,
        year: None,
        genres: None,
        tags: None,
        custom_metadata: None,
        total_book_count: None,
    };
    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata/locks", fake_id),
        &body,
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_metadata_locks_empty_request() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Empty request should still succeed and return current state
    let body = UpdateMetadataLocksRequest {
        title: None,
        title_sort: None,
        summary: None,
        publisher: None,
        imprint: None,
        status: None,
        age_rating: None,
        language: None,
        reading_direction: None,
        year: None,
        genres: None,
        tags: None,
        custom_metadata: None,
        total_book_count: None,
    };
    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata/locks", series.id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<MetadataLocks>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let locks = response.unwrap();

    // All locks should remain false
    assert!(!locks.title);
    assert!(!locks.summary);
}

// ============================================================================
// Authorization tests
// ============================================================================

#[tokio::test]
async fn test_update_locks_requires_write_permission() {
    let (db, _temp_dir) = setup_test_db().await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create non-admin user
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user("user", "user@example.com", &password_hash, false);
    let created = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = state
        .jwt_service
        .generate_token(created.id, created.username, created.is_admin)
        .unwrap();
    let app = create_test_router(state).await;

    let body = UpdateMetadataLocksRequest {
        title: Some(true),
        title_sort: None,
        summary: None,
        publisher: None,
        imprint: None,
        status: None,
        age_rating: None,
        language: None,
        reading_direction: None,
        year: None,
        genres: None,
        tags: None,
        custom_metadata: None,
        total_book_count: None,
    };
    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata/locks", series.id),
        &body,
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}
