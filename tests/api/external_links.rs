//! Integration tests for series external link endpoints

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::series::{
    CreateExternalLinkRequest, ExternalLinkDto, ExternalLinkListResponse,
};
use codex::db::repositories::{LibraryRepository, SeriesRepository, UserRepository};
use codex::db::ScanningStrategy;
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
// List External Links Tests
// ============================================================================

#[tokio::test]
async fn test_list_external_links_empty() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/external-links", series_id),
        &token,
    );
    let (status, response): (StatusCode, Option<ExternalLinkListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let link_response = response.unwrap();
    assert_eq!(link_response.links.len(), 0);
}

#[tokio::test]
async fn test_list_external_links_with_data() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    use codex::db::repositories::ExternalLinkRepository;
    ExternalLinkRepository::create(
        &db,
        series_id,
        "myanimelist",
        "https://mal.net/1",
        Some("1"),
    )
    .await
    .unwrap();
    ExternalLinkRepository::create(&db, series_id, "mangadex", "https://mangadex.org/2", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/external-links", series_id),
        &token,
    );
    let (status, response): (StatusCode, Option<ExternalLinkListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let link_response = response.unwrap();
    assert_eq!(link_response.links.len(), 2);
}

#[tokio::test]
async fn test_list_external_links_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = Uuid::new_v4();
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/external-links", fake_id),
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_list_external_links_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let request = get_request(&format!("/api/v1/series/{}/external-links", series_id));
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Create External Link Tests
// ============================================================================

#[tokio::test]
async fn test_create_external_link() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = CreateExternalLinkRequest {
        source_name: "myanimelist".to_string(),
        url: "https://myanimelist.net/manga/12345".to_string(),
        external_id: Some("12345".to_string()),
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/external-links", series_id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<ExternalLinkDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let link = response.unwrap();
    assert_eq!(link.source_name, "myanimelist");
    assert_eq!(link.url, "https://myanimelist.net/manga/12345");
    assert_eq!(link.external_id, Some("12345".to_string()));
    assert_eq!(link.series_id, series_id);
}

#[tokio::test]
async fn test_create_external_link_normalizes_source_name() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = CreateExternalLinkRequest {
        source_name: "  MyAnimeList  ".to_string(),
        url: "https://mal.net/1".to_string(),
        external_id: None,
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/external-links", series_id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<ExternalLinkDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let link = response.unwrap();
    assert_eq!(link.source_name, "myanimelist");
}

#[tokio::test]
async fn test_create_external_link_trims_url() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = CreateExternalLinkRequest {
        source_name: "mal".to_string(),
        url: "  https://mal.net/1  ".to_string(),
        external_id: Some("  123  ".to_string()),
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/external-links", series_id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<ExternalLinkDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let link = response.unwrap();
    assert_eq!(link.url, "https://mal.net/1");
    assert_eq!(link.external_id, Some("123".to_string()));
}

#[tokio::test]
async fn test_create_external_link_upsert() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    // Create initial link
    let body = CreateExternalLinkRequest {
        source_name: "myanimelist".to_string(),
        url: "https://mal.net/old".to_string(),
        external_id: Some("old-id".to_string()),
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/external-links", series_id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<ExternalLinkDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let first_link = response.unwrap();

    // Update (upsert) link
    let app = create_test_router(state).await;
    let body = CreateExternalLinkRequest {
        source_name: "myanimelist".to_string(),
        url: "https://mal.net/new".to_string(),
        external_id: Some("new-id".to_string()),
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/external-links", series_id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<ExternalLinkDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let second_link = response.unwrap();

    // Should be same ID (upsert)
    assert_eq!(first_link.id, second_link.id);
    assert_eq!(second_link.url, "https://mal.net/new");
    assert_eq!(second_link.external_id, Some("new-id".to_string()));
}

#[tokio::test]
async fn test_create_external_link_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = Uuid::new_v4();
    let body = CreateExternalLinkRequest {
        source_name: "myanimelist".to_string(),
        url: "https://mal.net/1".to_string(),
        external_id: None,
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/external-links", fake_id),
        &body,
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Delete External Link Tests
// ============================================================================

#[tokio::test]
async fn test_delete_external_link() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    use codex::db::repositories::ExternalLinkRepository;
    ExternalLinkRepository::create(&db, series_id, "myanimelist", "https://mal.net/1", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/external-links/myanimelist", series_id),
        &token,
    );
    let (status, _response): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify it was deleted
    let remaining = ExternalLinkRepository::get_for_series(&db, series_id)
        .await
        .unwrap();
    assert!(remaining.is_empty());
}

#[tokio::test]
async fn test_delete_external_link_case_insensitive() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    use codex::db::repositories::ExternalLinkRepository;
    ExternalLinkRepository::create(&db, series_id, "myanimelist", "https://mal.net/1", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Delete using different case
    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/external-links/MyAnimeList", series_id),
        &token,
    );
    let (status, _response): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_delete_external_link_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/external-links/nonexistent", series_id),
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_external_link_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = Uuid::new_v4();
    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/external-links/myanimelist", fake_id),
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}
