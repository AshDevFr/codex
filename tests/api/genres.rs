//! Integration tests for genre endpoints

#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::series::{
    AddSeriesGenreRequest, GenreDto, GenreListResponse, SetSeriesGenresRequest,
    TaxonomyCleanupResponse,
};
use codex::api::error::ErrorResponse;
use codex::db::repositories::{
    GenreRepository, LibraryRepository, SeriesRepository, UserRepository,
};
use codex::db::ScanningStrategy;
use codex::utils::password;
use common::*;
use hyper::StatusCode;

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
// List Genres Tests
// ============================================================================

#[tokio::test]
async fn test_list_genres_empty() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/genres", &token);
    let (status, response): (StatusCode, Option<GenreListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let genre_response = response.unwrap();
    assert_eq!(genre_response.genres.len(), 0);
}

#[tokio::test]
async fn test_list_genres_with_data() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create some genres
    GenreRepository::create(&db, "Action").await.unwrap();
    GenreRepository::create(&db, "Comedy").await.unwrap();
    GenreRepository::create(&db, "Drama").await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/genres", &token);
    let (status, response): (StatusCode, Option<GenreListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let genre_response = response.unwrap();
    assert_eq!(genre_response.genres.len(), 3);

    // Verify sorted by name
    let names: Vec<&str> = genre_response
        .genres
        .iter()
        .map(|g| g.name.as_str())
        .collect();
    assert_eq!(names, vec!["Action", "Comedy", "Drama"]);
}

#[tokio::test]
async fn test_list_genres_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // No auth token
    let request = get_request("/api/v1/genres");
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Series Genres Tests
// ============================================================================

#[tokio::test]
async fn test_get_series_genres_empty() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(&format!("/api/v1/series/{}/genres", series.id), &token);
    let (status, response): (StatusCode, Option<GenreListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let genre_response = response.unwrap();
    assert_eq!(genre_response.genres.len(), 0);
}

#[tokio::test]
async fn test_set_series_genres() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Set genres
    let body = SetSeriesGenresRequest {
        genres: vec!["Action".to_string(), "Comedy".to_string()],
    };
    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/genres", series.id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<GenreListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let genre_response = response.unwrap();
    assert_eq!(genre_response.genres.len(), 2);

    let names: Vec<&str> = genre_response
        .genres
        .iter()
        .map(|g| g.name.as_str())
        .collect();
    assert!(names.contains(&"Action"));
    assert!(names.contains(&"Comedy"));
}

#[tokio::test]
async fn test_set_series_genres_replaces_existing() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Set initial genres
    GenreRepository::set_genres_for_series(
        &db,
        series.id,
        vec!["Action".to_string(), "Drama".to_string()],
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Replace with new genres
    let body = SetSeriesGenresRequest {
        genres: vec!["Comedy".to_string()],
    };
    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/genres", series.id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<GenreListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let genre_response = response.unwrap();
    assert_eq!(genre_response.genres.len(), 1);
    assert_eq!(genre_response.genres[0].name, "Comedy");
}

#[tokio::test]
async fn test_set_series_genres_clear() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Set initial genres
    GenreRepository::set_genres_for_series(&db, series.id, vec!["Action".to_string()])
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Clear genres by setting empty list
    let body = SetSeriesGenresRequest { genres: vec![] };
    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/genres", series.id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<GenreListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let genre_response = response.unwrap();
    assert_eq!(genre_response.genres.len(), 0);
}

#[tokio::test]
async fn test_get_series_genres_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(&format!("/api/v1/series/{}/genres", fake_id), &token);
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Add/Remove Single Genre Tests
// ============================================================================

#[tokio::test]
async fn test_add_single_genre_to_series() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Add a single genre
    let body = AddSeriesGenreRequest {
        name: "Action".to_string(),
    };
    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/genres", series.id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<GenreDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let genre = response.unwrap();
    assert_eq!(genre.name, "Action");

    // Verify it was added
    let genres = GenreRepository::get_genres_for_series(&db, series.id)
        .await
        .unwrap();
    assert_eq!(genres.len(), 1);
    assert_eq!(genres[0].name, "Action");
}

#[tokio::test]
async fn test_add_genre_to_series_idempotent() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Pre-add the genre
    GenreRepository::add_genre_to_series(&db, series.id, "Action")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Add the same genre again
    let body = AddSeriesGenreRequest {
        name: "Action".to_string(),
    };
    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/genres", series.id),
        &body,
        &token,
    );
    let (status, _response): (StatusCode, Option<GenreDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);

    // Should still only have one genre
    let genres = GenreRepository::get_genres_for_series(&db, series.id)
        .await
        .unwrap();
    assert_eq!(genres.len(), 1);
}

#[tokio::test]
async fn test_remove_genre_from_series() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Add genres
    let genre = GenreRepository::add_genre_to_series(&db, series.id, "Action")
        .await
        .unwrap();
    GenreRepository::add_genre_to_series(&db, series.id, "Comedy")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Remove one genre
    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/genres/{}", series.id, genre.id),
        &token,
    );
    let (status, _): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify only Comedy remains
    let genres = GenreRepository::get_genres_for_series(&db, series.id)
        .await
        .unwrap();
    assert_eq!(genres.len(), 1);
    assert_eq!(genres[0].name, "Comedy");
}

#[tokio::test]
async fn test_remove_genre_from_series_not_linked() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create a genre but don't link it
    let genre = GenreRepository::create(&db, "NotLinked").await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Try to remove a genre that's not linked
    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/genres/{}", series.id, genre.id),
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Delete Genre Tests (Admin)
// ============================================================================

#[tokio::test]
async fn test_delete_genre_admin() {
    let (db, _temp_dir) = setup_test_db().await;

    let genre = GenreRepository::create(&db, "ToDelete").await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = delete_request_with_auth(&format!("/api/v1/genres/{}", genre.id), &token);
    let (status, _): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify deleted
    let fetched = GenreRepository::get_by_id(&db, genre.id).await.unwrap();
    assert!(fetched.is_none());
}

#[tokio::test]
async fn test_delete_genre_non_admin_forbidden() {
    let (db, _temp_dir) = setup_test_db().await;

    let genre = GenreRepository::create(&db, "ToDelete").await.unwrap();

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

    let request = delete_request_with_auth(&format!("/api/v1/genres/{}", genre.id), &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);

    // Verify NOT deleted
    let fetched = GenreRepository::get_by_id(&db, genre.id).await.unwrap();
    assert!(fetched.is_some());
}

#[tokio::test]
async fn test_delete_genre_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = delete_request_with_auth(&format!("/api/v1/genres/{}", fake_id), &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Cleanup Genres Tests (Admin)
// ============================================================================

#[tokio::test]
async fn test_cleanup_genres_admin() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create genres - one used, two unused
    GenreRepository::add_genre_to_series(&db, series.id, "UsedGenre")
        .await
        .unwrap();
    GenreRepository::create(&db, "UnusedGenre1").await.unwrap();
    GenreRepository::create(&db, "UnusedGenre2").await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = post_request_with_auth("/api/v1/genres/cleanup", &token);
    let (status, response): (StatusCode, Option<TaxonomyCleanupResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let cleanup = response.unwrap();
    assert_eq!(cleanup.deleted_count, 2);
    assert!(cleanup.deleted_names.contains(&"UnusedGenre1".to_string()));
    assert!(cleanup.deleted_names.contains(&"UnusedGenre2".to_string()));

    // Verify only UsedGenre remains
    let remaining = GenreRepository::list_all(&db).await.unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].name, "UsedGenre");
}

#[tokio::test]
async fn test_cleanup_genres_non_admin_forbidden() {
    let (db, _temp_dir) = setup_test_db().await;

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

    let request = post_request_with_auth("/api/v1/genres/cleanup", &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_cleanup_genres_empty() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = post_request_with_auth("/api/v1/genres/cleanup", &token);
    let (status, response): (StatusCode, Option<TaxonomyCleanupResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let cleanup = response.unwrap();
    assert_eq!(cleanup.deleted_count, 0);
    assert!(cleanup.deleted_names.is_empty());
}
