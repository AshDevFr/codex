//! Integration tests for series external ID endpoints

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::series::{
    CreateSeriesExternalIdRequest, SeriesExternalIdDto, SeriesExternalIdListResponse,
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
// List External IDs Tests
// ============================================================================

#[tokio::test]
async fn test_list_series_external_ids_empty() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/external-ids", series_id),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesExternalIdListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let id_response = response.unwrap();
    assert_eq!(id_response.external_ids.len(), 0);
}

#[tokio::test]
async fn test_list_series_external_ids_with_data() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    use codex::db::repositories::SeriesExternalIdRepository;
    SeriesExternalIdRepository::create(
        &db,
        series_id,
        "plugin:anilist",
        "12345",
        Some("https://anilist.co/manga/12345"),
        None,
    )
    .await
    .unwrap();
    SeriesExternalIdRepository::create(&db, series_id, "manual", "67890", None, None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/external-ids", series_id),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesExternalIdListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let id_response = response.unwrap();
    assert_eq!(id_response.external_ids.len(), 2);
}

#[tokio::test]
async fn test_list_series_external_ids_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = Uuid::new_v4();
    let request =
        get_request_with_auth(&format!("/api/v1/series/{}/external-ids", fake_id), &token);
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_list_series_external_ids_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let request = get_request(&format!("/api/v1/series/{}/external-ids", series_id));
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Create External ID Tests
// ============================================================================

#[tokio::test]
async fn test_create_series_external_id() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = CreateSeriesExternalIdRequest {
        source: "plugin:anilist".to_string(),
        external_id: "12345".to_string(),
        external_url: Some("https://anilist.co/manga/12345".to_string()),
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/external-ids", series_id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesExternalIdDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let ext_id = response.unwrap();
    assert_eq!(ext_id.source, "plugin:anilist");
    assert_eq!(ext_id.external_id, "12345");
    assert_eq!(
        ext_id.external_url,
        Some("https://anilist.co/manga/12345".to_string())
    );
    assert_eq!(ext_id.series_id, series_id);
}

#[tokio::test]
async fn test_create_series_external_id_upsert() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    // Create initial external ID
    let body = CreateSeriesExternalIdRequest {
        source: "plugin:anilist".to_string(),
        external_id: "12345".to_string(),
        external_url: Some("https://anilist.co/manga/12345".to_string()),
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/external-ids", series_id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesExternalIdDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let first = response.unwrap();

    // Update (upsert) with same source
    let app = create_test_router(state).await;
    let body = CreateSeriesExternalIdRequest {
        source: "plugin:anilist".to_string(),
        external_id: "67890".to_string(),
        external_url: Some("https://anilist.co/manga/67890".to_string()),
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/external-ids", series_id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesExternalIdDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let second = response.unwrap();

    // Should be same ID (upsert)
    assert_eq!(first.id, second.id);
    assert_eq!(second.external_id, "67890");
    assert_eq!(
        second.external_url,
        Some("https://anilist.co/manga/67890".to_string())
    );
}

#[tokio::test]
async fn test_create_series_external_id_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = Uuid::new_v4();
    let body = CreateSeriesExternalIdRequest {
        source: "manual".to_string(),
        external_id: "12345".to_string(),
        external_url: None,
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/external-ids", fake_id),
        &body,
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Delete External ID Tests
// ============================================================================

#[tokio::test]
async fn test_delete_series_external_id() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    use codex::db::repositories::SeriesExternalIdRepository;
    let ext_id =
        SeriesExternalIdRepository::create(&db, series_id, "plugin:anilist", "12345", None, None)
            .await
            .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/external-ids/{}", series_id, ext_id.id),
        &token,
    );
    let (status, _response): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify it was deleted
    let remaining = SeriesExternalIdRepository::get_for_series(&db, series_id)
        .await
        .unwrap();
    assert!(remaining.is_empty());
}

#[tokio::test]
async fn test_delete_series_external_id_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_ext_id = Uuid::new_v4();
    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/external-ids/{}", series_id, fake_ext_id),
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_series_external_id_wrong_series() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    use codex::db::repositories::SeriesExternalIdRepository;
    let ext_id =
        SeriesExternalIdRepository::create(&db, series_id, "plugin:anilist", "12345", None, None)
            .await
            .unwrap();

    // Create another series
    let library2 = LibraryRepository::create(
        &db,
        "Test Library 2",
        "/test/path2",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let series2 = SeriesRepository::create(&db, library2.id, "Test Series 2", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Try to delete using wrong series ID
    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/external-ids/{}", series2.id, ext_id.id),
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}
