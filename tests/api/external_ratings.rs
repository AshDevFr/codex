//! Integration tests for series external rating endpoints

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::series::{
    CreateExternalRatingRequest, ExternalRatingDto, ExternalRatingListResponse,
};
use codex::db::ScanningStrategy;
use codex::db::repositories::{LibraryRepository, SeriesRepository, UserRepository};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use uuid::Uuid;

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

// Helper to create a library and series
async fn create_test_series(db: &sea_orm::DatabaseConnection) -> (Uuid, Uuid) {
    let library =
        LibraryRepository::create(db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(db, library.id, "Test Series", None)
        .await
        .unwrap();
    (library.id, series.id)
}

// ============================================================================
// List External Ratings Tests
// ============================================================================

#[tokio::test]
async fn test_list_external_ratings_empty() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/external-ratings", series_id),
        &token,
    );
    let (status, response): (StatusCode, Option<ExternalRatingListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let rating_response = response.unwrap();
    assert_eq!(rating_response.ratings.len(), 0);
}

#[tokio::test]
async fn test_list_external_ratings_with_data() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    use codex::db::repositories::ExternalRatingRepository;
    use sea_orm::prelude::Decimal;

    let dec = |v: f64| Decimal::from_f64_retain(v).unwrap();
    ExternalRatingRepository::create(&db, series_id, "myanimelist", dec(85.0), Some(1000))
        .await
        .unwrap();
    ExternalRatingRepository::create(&db, series_id, "anilist", dec(90.5), Some(500))
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/external-ratings", series_id),
        &token,
    );
    let (status, response): (StatusCode, Option<ExternalRatingListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let rating_response = response.unwrap();
    assert_eq!(rating_response.ratings.len(), 2);
}

#[tokio::test]
async fn test_list_external_ratings_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = Uuid::new_v4();
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/external-ratings", fake_id),
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_list_external_ratings_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let request = get_request(&format!("/api/v1/series/{}/external-ratings", series_id));
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Create External Rating Tests
// ============================================================================

#[tokio::test]
async fn test_create_external_rating() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = CreateExternalRatingRequest {
        source_name: "myanimelist".to_string(),
        rating: 85.5,
        vote_count: Some(12500),
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/external-ratings", series_id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<ExternalRatingDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let rating = response.unwrap();
    assert_eq!(rating.source_name, "myanimelist");
    assert!((rating.rating - 85.5).abs() < 0.01);
    assert_eq!(rating.vote_count, Some(12500));
    assert_eq!(rating.series_id, series_id);
}

#[tokio::test]
async fn test_create_external_rating_normalizes_source_name() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = CreateExternalRatingRequest {
        source_name: "  MyAnimeList  ".to_string(),
        rating: 80.0,
        vote_count: None,
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/external-ratings", series_id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<ExternalRatingDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let rating = response.unwrap();
    assert_eq!(rating.source_name, "myanimelist");
}

#[tokio::test]
async fn test_create_external_rating_upsert() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    // Create initial rating
    let body = CreateExternalRatingRequest {
        source_name: "myanimelist".to_string(),
        rating: 80.0,
        vote_count: Some(1000),
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/external-ratings", series_id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<ExternalRatingDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let first_rating = response.unwrap();

    // Update (upsert) rating
    let app = create_test_router(state).await;
    let body = CreateExternalRatingRequest {
        source_name: "myanimelist".to_string(),
        rating: 90.0,
        vote_count: Some(2000),
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/external-ratings", series_id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<ExternalRatingDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let second_rating = response.unwrap();

    // Should be same ID (upsert)
    assert_eq!(first_rating.id, second_rating.id);
    assert!((second_rating.rating - 90.0).abs() < 0.01);
    assert_eq!(second_rating.vote_count, Some(2000));
}

#[tokio::test]
async fn test_create_external_rating_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = Uuid::new_v4();
    let body = CreateExternalRatingRequest {
        source_name: "myanimelist".to_string(),
        rating: 85.0,
        vote_count: None,
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/external-ratings", fake_id),
        &body,
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Delete External Rating Tests
// ============================================================================

#[tokio::test]
async fn test_delete_external_rating() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    use codex::db::repositories::ExternalRatingRepository;
    use sea_orm::prelude::Decimal;

    let dec = |v: f64| Decimal::from_f64_retain(v).unwrap();
    ExternalRatingRepository::create(&db, series_id, "myanimelist", dec(85.0), None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/external-ratings/myanimelist", series_id),
        &token,
    );
    let (status, _response): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify it was deleted
    let remaining = ExternalRatingRepository::get_for_series(&db, series_id)
        .await
        .unwrap();
    assert!(remaining.is_empty());
}

#[tokio::test]
async fn test_delete_external_rating_case_insensitive() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    use codex::db::repositories::ExternalRatingRepository;
    use sea_orm::prelude::Decimal;

    let dec = |v: f64| Decimal::from_f64_retain(v).unwrap();
    ExternalRatingRepository::create(&db, series_id, "myanimelist", dec(85.0), None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Delete using different case
    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/external-ratings/MyAnimeList", series_id),
        &token,
    );
    let (status, _response): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_delete_external_rating_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/external-ratings/nonexistent", series_id),
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_external_rating_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = Uuid::new_v4();
    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/external-ratings/myanimelist", fake_id),
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}
