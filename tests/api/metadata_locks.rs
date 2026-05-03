//! Integration tests for series and book metadata locks endpoints

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::book::{
    BookMetadataLocks, BookMetadataResponse, UpdateBookMetadataLocksRequest,
};
use codex::api::routes::v1::dto::series::{
    FullSeriesMetadataResponse, MetadataLocks, UpdateMetadataLocksRequest,
};
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    AlternateTitleRepository, BookMetadataRepository, BookRepository, ExternalLinkRepository,
    ExternalRatingRepository, GenreRepository, LibraryRepository, SeriesRepository, TagRepository,
    UserRepository,
};
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
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

// ============================================================================
// GET /api/v1/series/{id}/metadata tests
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
    let request = get_request_with_auth(&format!("/api/v1/series/{}/metadata", series.id), &token);
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

    let request = get_request_with_auth(&format!("/api/v1/series/{}/metadata", series.id), &token);
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
    let request = get_request_with_auth(&format!("/api/v1/series/{}/metadata", fake_id), &token);
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

    let request = get_request(&format!("/api/v1/series/{}/metadata", series.id));
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
    assert!(!body.alternate_titles);
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
        total_volume_count: None,
        total_chapter_count: None,
        cover: None,
        authors_json_lock: None,
        alternate_titles: None,
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
        total_volume_count: None,
        total_chapter_count: None,
        cover: None,
        authors_json_lock: None,
        alternate_titles: None,
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
        total_volume_count: None,
        total_chapter_count: None,
        cover: None,
        authors_json_lock: None,
        alternate_titles: None,
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
        total_volume_count: Some(true),
        total_chapter_count: Some(true),
        cover: Some(true),
        authors_json_lock: Some(true),
        alternate_titles: Some(true),
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
    assert!(locks.alternate_titles);
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
        total_volume_count: None,
        total_chapter_count: None,
        cover: None,
        authors_json_lock: None,
        alternate_titles: None,
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
        total_volume_count: None,
        total_chapter_count: None,
        cover: None,
        authors_json_lock: None,
        alternate_titles: None,
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
// Alternate Titles Lock Independence Tests
// ============================================================================

#[tokio::test]
async fn test_alternate_titles_lock_independent_from_title_lock() {
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

    // Lock title only, leave alternate_titles unlocked
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
        total_volume_count: None,
        total_chapter_count: None,
        cover: None,
        authors_json_lock: None,
        alternate_titles: None,
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

    // Title should be locked, alternate_titles should NOT be locked
    assert!(locks.title, "title should be locked");
    assert!(
        !locks.alternate_titles,
        "alternate_titles should NOT be locked when only title is locked"
    );
}

#[tokio::test]
async fn test_alternate_titles_lock_without_affecting_title_lock() {
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

    // Lock alternate_titles only, leave title unlocked
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
        total_volume_count: None,
        total_chapter_count: None,
        cover: None,
        authors_json_lock: None,
        alternate_titles: Some(true),
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

    // Alternate titles should be locked, title should NOT be locked
    assert!(locks.alternate_titles, "alternate_titles should be locked");
    assert!(
        !locks.title,
        "title should NOT be locked when only alternate_titles is locked"
    );

    // Verify via GET
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/metadata/locks", series.id),
        &token,
    );
    let (status, response): (StatusCode, Option<MetadataLocks>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let locks = response.unwrap();
    assert!(locks.alternate_titles);
    assert!(!locks.title);
}

#[tokio::test]
async fn test_alternate_titles_lock_in_full_metadata_response() {
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

    // Lock alternate titles
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
        total_volume_count: None,
        total_chapter_count: None,
        cover: None,
        authors_json_lock: None,
        alternate_titles: Some(true),
    };
    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata/locks", series.id),
        &body,
        &token,
    );
    let (status, _): (StatusCode, Option<MetadataLocks>) =
        make_json_request(app.clone(), request).await;
    assert_eq!(status, StatusCode::OK);

    // Verify it shows up in full metadata response
    let request = get_request_with_auth(&format!("/api/v1/series/{}/metadata", series.id), &token);
    let (status, response): (StatusCode, Option<FullSeriesMetadataResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert!(
        body.locks.alternate_titles,
        "alternate_titles lock should be true in full metadata response"
    );
    assert!(!body.locks.title, "title lock should remain false");
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
        .generate_token(created.id, created.username.clone(), created.get_role())
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
        total_volume_count: None,
        total_chapter_count: None,
        cover: None,
        authors_json_lock: None,
        alternate_titles: None,
    };
    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata/locks", series.id),
        &body,
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ============================================================================
// GET /api/v1/series/{id}?full=true tests (Full Series Data with Metadata)
// ============================================================================

#[tokio::test]
async fn test_get_full_series_basic() {
    use codex::api::routes::v1::dto::series::FullSeriesResponse;

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

    // Fetch full series
    let request = get_request_with_auth(&format!("/api/v1/series/{}?full=true", series.id), &token);
    let (status, response): (StatusCode, Option<FullSeriesResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();

    // Verify series data
    assert_eq!(body.id, series.id);
    assert_eq!(body.library_id, library.id);
    assert_eq!(body.library_name, "Test Library");
    assert_eq!(body.book_count, 0);
    assert!(body.path.is_some());

    // Verify metadata is present
    assert_eq!(body.metadata.title, "Attack on Titan");
    assert!(!body.metadata.locks.title);
    assert!(!body.metadata.locks.summary);

    // Check related data arrays are present and empty
    assert!(body.genres.is_empty());
    assert!(body.tags.is_empty());
    assert!(body.alternate_titles.is_empty());
    assert!(body.external_ratings.is_empty());
    assert!(body.external_links.is_empty());
}

#[tokio::test]
async fn test_get_full_series_with_related_data() {
    use codex::api::routes::v1::dto::series::FullSeriesResponse;

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

    let request = get_request_with_auth(&format!("/api/v1/series/{}?full=true", series.id), &token);
    let (status, response): (StatusCode, Option<FullSeriesResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();

    // Verify series data
    assert_eq!(body.id, series.id);
    assert_eq!(body.metadata.title, "One Piece");

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
async fn test_get_full_series_not_found() {
    use codex::api::routes::v1::dto::series::FullSeriesResponse;

    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(&format!("/api/v1/series/{}?full=true", fake_id), &token);
    let (status, _): (StatusCode, Option<FullSeriesResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_full_series_requires_auth() {
    use codex::api::routes::v1::dto::series::FullSeriesResponse;

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

    // Request without auth token
    let request = get_request(&format!("/api/v1/series/{}?full=true", series.id));
    let (status, _): (StatusCode, Option<FullSeriesResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Book Metadata Locks Tests
// ============================================================================

// Helper to create a test book in the database
fn create_test_book_model(
    series_id: uuid::Uuid,
    library_id: uuid::Uuid,
    path: &str,
    name: &str,
) -> codex::db::entities::books::Model {
    use chrono::Utc;
    codex::db::entities::books::Model {
        id: uuid::Uuid::new_v4(),
        series_id,
        library_id,
        file_path: path.to_string(),
        file_name: name.to_string(),
        file_size: 1024,
        file_hash: format!("hash_{}", uuid::Uuid::new_v4()),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 10,
        deleted: false,
        analyzed: false,
        analysis_error: None,
        analysis_errors: None,
        modified_at: Utc::now(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        thumbnail_path: None,
        thumbnail_generated_at: None,
        koreader_hash: None,
        epub_positions: None,
        epub_spine_items: None,
    }
}

// Helper to create a library, series, and book with metadata
async fn create_test_book_with_metadata(
    db: &sea_orm::DatabaseConnection,
) -> (uuid::Uuid, uuid::Uuid, uuid::Uuid) {
    let library =
        LibraryRepository::create(db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(db, library.id, "Test Series", None)
        .await
        .unwrap();
    let book_model =
        create_test_book_model(series.id, library.id, "/test/path/book1.cbz", "book1.cbz");
    let book = BookRepository::create(db, &book_model, None).await.unwrap();

    // Create metadata record for the book (required for lock endpoints)
    BookMetadataRepository::create_with_title_and_number(
        db,
        book.id,
        Some("Test Book".to_string()),
        None,
    )
    .await
    .unwrap();

    (library.id, series.id, book.id)
}

#[tokio::test]
async fn test_get_book_metadata_locks_default() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, _, book_id) = create_test_book_with_metadata(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request =
        get_request_with_auth(&format!("/api/v1/books/{}/metadata/locks", book_id), &token);
    let (status, response): (StatusCode, Option<BookMetadataLocks>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let locks = response.unwrap();

    // All locks should default to false
    assert!(!locks.title_lock);
    assert!(!locks.title_sort_lock);
    assert!(!locks.number_lock);
    assert!(!locks.summary_lock);
    assert!(!locks.writer_lock);
    assert!(!locks.penciller_lock);
    assert!(!locks.inker_lock);
    assert!(!locks.colorist_lock);
    assert!(!locks.letterer_lock);
    assert!(!locks.cover_artist_lock);
    assert!(!locks.editor_lock);
    assert!(!locks.publisher_lock);
    assert!(!locks.imprint_lock);
    assert!(!locks.genre_lock);
    assert!(!locks.language_iso_lock);
    assert!(!locks.format_detail_lock);
    assert!(!locks.black_and_white_lock);
    assert!(!locks.manga_lock);
    assert!(!locks.year_lock);
    assert!(!locks.month_lock);
    assert!(!locks.day_lock);
    assert!(!locks.volume_lock);
    assert!(!locks.count_lock);
    assert!(!locks.isbns_lock);
}

#[tokio::test]
async fn test_update_book_metadata_locks_single() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, _, book_id) = create_test_book_with_metadata(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Update a single lock field
    let body = UpdateBookMetadataLocksRequest {
        summary_lock: Some(true),
        title_lock: None,
        title_sort_lock: None,
        number_lock: None,
        writer_lock: None,
        penciller_lock: None,
        inker_lock: None,
        colorist_lock: None,
        letterer_lock: None,
        cover_artist_lock: None,
        editor_lock: None,
        publisher_lock: None,
        imprint_lock: None,
        genre_lock: None,
        language_iso_lock: None,
        format_detail_lock: None,
        black_and_white_lock: None,
        manga_lock: None,
        year_lock: None,
        month_lock: None,
        day_lock: None,
        volume_lock: None,
        chapter_lock: None,
        count_lock: None,
        isbns_lock: None,
        book_type_lock: None,
        subtitle_lock: None,
        authors_json_lock: None,
        translator_lock: None,
        edition_lock: None,
        original_title_lock: None,
        original_year_lock: None,
        series_position_lock: None,
        series_total_lock: None,
        subjects_lock: None,
        awards_json_lock: None,
        custom_metadata_lock: None,
        cover_lock: None,
    };
    let request = put_json_request_with_auth(
        &format!("/api/v1/books/{}/metadata/locks", book_id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<BookMetadataLocks>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let locks = response.unwrap();
    assert!(locks.summary_lock);
    // Others remain false
    assert!(!locks.title_lock);
    assert!(!locks.publisher_lock);

    // Verify it persisted by fetching again
    let request =
        get_request_with_auth(&format!("/api/v1/books/{}/metadata/locks", book_id), &token);
    let (status, response): (StatusCode, Option<BookMetadataLocks>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let locks = response.unwrap();
    assert!(locks.summary_lock);
}

#[tokio::test]
async fn test_update_book_metadata_locks_multiple() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, _, book_id) = create_test_book_with_metadata(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Update multiple lock fields
    let body = UpdateBookMetadataLocksRequest {
        title_lock: Some(true),
        summary_lock: Some(true),
        publisher_lock: Some(true),
        year_lock: Some(true),
        title_sort_lock: None,
        number_lock: None,
        writer_lock: None,
        penciller_lock: None,
        inker_lock: None,
        colorist_lock: None,
        letterer_lock: None,
        cover_artist_lock: None,
        editor_lock: None,
        imprint_lock: None,
        genre_lock: None,
        language_iso_lock: None,
        format_detail_lock: None,
        black_and_white_lock: None,
        manga_lock: None,
        month_lock: None,
        day_lock: None,
        volume_lock: None,
        chapter_lock: None,
        count_lock: None,
        isbns_lock: None,
        book_type_lock: None,
        subtitle_lock: None,
        authors_json_lock: None,
        translator_lock: None,
        edition_lock: None,
        original_title_lock: None,
        original_year_lock: None,
        series_position_lock: None,
        series_total_lock: None,
        subjects_lock: None,
        awards_json_lock: None,
        custom_metadata_lock: None,
        cover_lock: None,
    };
    let request = put_json_request_with_auth(
        &format!("/api/v1/books/{}/metadata/locks", book_id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<BookMetadataLocks>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let locks = response.unwrap();
    assert!(locks.title_lock);
    assert!(locks.summary_lock);
    assert!(locks.publisher_lock);
    assert!(locks.year_lock);
    // Others remain false
    assert!(!locks.genre_lock);
    assert!(!locks.writer_lock);
}

#[tokio::test]
async fn test_get_book_metadata_locks_includes_title_sort_lock() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, _, book_id) = create_test_book_with_metadata(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request =
        get_request_with_auth(&format!("/api/v1/books/{}/metadata/locks", book_id), &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();

    // Verify title_sort_lock field exists in the response JSON
    assert!(
        body.get("titleSortLock").is_some(),
        "titleSortLock field should be present in response"
    );
    assert_eq!(body.get("titleSortLock").unwrap(), false);
}

#[tokio::test]
async fn test_update_book_title_sort_lock() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, _, book_id) = create_test_book_with_metadata(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Set title_sort_lock to true
    let body = UpdateBookMetadataLocksRequest {
        title_sort_lock: Some(true),
        title_lock: None,
        number_lock: None,
        summary_lock: None,
        writer_lock: None,
        penciller_lock: None,
        inker_lock: None,
        colorist_lock: None,
        letterer_lock: None,
        cover_artist_lock: None,
        editor_lock: None,
        publisher_lock: None,
        imprint_lock: None,
        genre_lock: None,
        language_iso_lock: None,
        format_detail_lock: None,
        black_and_white_lock: None,
        manga_lock: None,
        year_lock: None,
        month_lock: None,
        day_lock: None,
        volume_lock: None,
        chapter_lock: None,
        count_lock: None,
        isbns_lock: None,
        book_type_lock: None,
        subtitle_lock: None,
        authors_json_lock: None,
        translator_lock: None,
        edition_lock: None,
        original_title_lock: None,
        original_year_lock: None,
        series_position_lock: None,
        series_total_lock: None,
        subjects_lock: None,
        awards_json_lock: None,
        custom_metadata_lock: None,
        cover_lock: None,
    };
    let request = put_json_request_with_auth(
        &format!("/api/v1/books/{}/metadata/locks", book_id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<BookMetadataLocks>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let locks = response.unwrap();
    assert!(locks.title_sort_lock);

    // Verify it persisted
    let request =
        get_request_with_auth(&format!("/api/v1/books/{}/metadata/locks", book_id), &token);
    let (status, response): (StatusCode, Option<BookMetadataLocks>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let locks = response.unwrap();
    assert!(locks.title_sort_lock);
}

#[tokio::test]
async fn test_book_metadata_locks_all_phase6_fields() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, _, book_id) = create_test_book_with_metadata(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Verify all Phase 6 lock fields are present in the response
    let request =
        get_request_with_auth(&format!("/api/v1/books/{}/metadata/locks", book_id), &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();

    // Phase 6 lock fields
    assert!(
        body.get("bookTypeLock").is_some(),
        "bookTypeLock field should be present"
    );
    assert!(
        body.get("subtitleLock").is_some(),
        "subtitleLock field should be present"
    );
    assert!(
        body.get("authorsJsonLock").is_some(),
        "authorsJsonLock field should be present"
    );
    assert!(
        body.get("translatorLock").is_some(),
        "translatorLock field should be present"
    );
    assert!(
        body.get("editionLock").is_some(),
        "editionLock field should be present"
    );
    assert!(
        body.get("originalTitleLock").is_some(),
        "originalTitleLock field should be present"
    );
    assert!(
        body.get("originalYearLock").is_some(),
        "originalYearLock field should be present"
    );
    assert!(
        body.get("seriesPositionLock").is_some(),
        "seriesPositionLock field should be present"
    );
    assert!(
        body.get("seriesTotalLock").is_some(),
        "seriesTotalLock field should be present"
    );
    assert!(
        body.get("subjectsLock").is_some(),
        "subjectsLock field should be present"
    );
    assert!(
        body.get("awardsJsonLock").is_some(),
        "awardsJsonLock field should be present"
    );
    assert!(
        body.get("customMetadataLock").is_some(),
        "customMetadataLock field should be present"
    );
    assert!(
        body.get("coverLock").is_some(),
        "coverLock field should be present"
    );

    // All should default to false
    assert_eq!(body.get("bookTypeLock").unwrap(), false);
    assert_eq!(body.get("subtitleLock").unwrap(), false);
    assert_eq!(body.get("authorsJsonLock").unwrap(), false);
    assert_eq!(body.get("coverLock").unwrap(), false);
}

#[tokio::test]
async fn test_book_metadata_locks_auth_required() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, _, book_id) = create_test_book_with_metadata(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // No auth token
    let request = get_request(&format!("/api/v1/books/{}/metadata/locks", book_id));
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_book_metadata_locks_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request =
        get_request_with_auth(&format!("/api/v1/books/{}/metadata/locks", fake_id), &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_book_metadata_response_includes_locks() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, _, book_id) = create_test_book_with_metadata(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // PATCH metadata and verify response includes locks field
    let request = patch_request_with_auth_json(
        &format!("/api/v1/books/{}/metadata", book_id),
        &token,
        r#"{"summary": "test summary"}"#,
    );
    let (status, response): (StatusCode, Option<BookMetadataResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let metadata = response.unwrap();

    // Verify locks field is present in the response
    assert_eq!(metadata.book_id, book_id);
    assert!(!metadata.locks.title_lock);
    // summary_lock should be auto-locked because we set summary to a non-null value
    assert!(metadata.locks.summary_lock);
    assert!(!metadata.locks.publisher_lock);
}
