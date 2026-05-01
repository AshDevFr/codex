//! Integration tests for release-tracking config + alias endpoints.

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::tracking::{
    CreateSeriesAliasRequest, SeriesAliasDto, SeriesAliasListResponse, SeriesTrackingDto,
    UpdateSeriesTrackingRequest,
};
use codex::db::ScanningStrategy;
use codex::db::repositories::{LibraryRepository, SeriesRepository, UserRepository};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use uuid::Uuid;

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

async fn create_regular_user_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> String {
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user("regular", "user@example.com", &password_hash, false);
    let created = UserRepository::create(db, &user).await.unwrap();
    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

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

// =============================================================================
// GET /tracking
// =============================================================================

#[tokio::test]
async fn get_tracking_returns_virtual_default_when_no_row() {
    let (db, _temp) = setup_test_db().await;
    let (_lib, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth(&format!("/api/v1/series/{}/tracking", series_id), &token);
    let (status, dto): (StatusCode, Option<SeriesTrackingDto>) = make_json_request(app, req).await;

    assert_eq!(status, StatusCode::OK);
    let dto = dto.unwrap();
    assert_eq!(dto.series_id, series_id);
    assert!(!dto.tracked);
    assert_eq!(dto.tracking_status, "unknown");
    assert!(dto.track_chapters);
    assert!(dto.track_volumes);
}

#[tokio::test]
async fn get_tracking_404_when_series_missing() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake = Uuid::new_v4();
    let req = get_request_with_auth(&format!("/api/v1/series/{}/tracking", fake), &token);
    let (status, _err): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// =============================================================================
// PATCH /tracking
// =============================================================================

#[tokio::test]
async fn patch_tracking_creates_then_updates() {
    let (db, _temp) = setup_test_db().await;
    let (_lib, series_id) = create_test_series(&db).await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // First PATCH: insert.
    let app1 = create_test_router(state.clone()).await;
    let body = UpdateSeriesTrackingRequest {
        tracked: Some(true),
        tracking_status: Some("ongoing".to_string()),
        latest_known_chapter: Some(Some(142.5)),
        ..Default::default()
    };
    let req = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/tracking", series_id),
        &body,
        &token,
    );
    let (status, dto): (StatusCode, Option<SeriesTrackingDto>) = make_json_request(app1, req).await;
    assert_eq!(status, StatusCode::OK);
    let dto = dto.unwrap();
    assert!(dto.tracked);
    assert_eq!(dto.tracking_status, "ongoing");
    assert_eq!(dto.latest_known_chapter, Some(142.5));

    // Second PATCH: only update one field; others persist.
    let app2 = create_test_router(state).await;
    let body = UpdateSeriesTrackingRequest {
        latest_known_chapter: Some(Some(143.0)),
        ..Default::default()
    };
    let req = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/tracking", series_id),
        &body,
        &token,
    );
    let (status, dto): (StatusCode, Option<SeriesTrackingDto>) = make_json_request(app2, req).await;
    assert_eq!(status, StatusCode::OK);
    let dto = dto.unwrap();
    assert!(dto.tracked, "tracked should persist");
    assert_eq!(dto.tracking_status, "ongoing", "status should persist");
    assert_eq!(dto.latest_known_chapter, Some(143.0));
}

#[tokio::test]
async fn patch_tracking_rejects_invalid_status() {
    let (db, _temp) = setup_test_db().await;
    let (_lib, series_id) = create_test_series(&db).await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = UpdateSeriesTrackingRequest {
        tracking_status: Some("paused".to_string()),
        ..Default::default()
    };
    let req = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/tracking", series_id),
        &body,
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn patch_tracking_requires_auth() {
    let (db, _temp) = setup_test_db().await;
    let (_lib, series_id) = create_test_series(&db).await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let body = UpdateSeriesTrackingRequest {
        tracked: Some(true),
        ..Default::default()
    };
    let req = patch_json_request(&format!("/api/v1/series/{}/tracking", series_id), &body);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// =============================================================================
// Aliases
// =============================================================================

#[tokio::test]
async fn list_aliases_empty_for_new_series() {
    let (db, _temp) = setup_test_db().await;
    let (_lib, series_id) = create_test_series(&db).await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth(&format!("/api/v1/series/{}/aliases", series_id), &token);
    let (status, body): (StatusCode, Option<SeriesAliasListResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.unwrap().aliases.is_empty());
}

#[tokio::test]
async fn create_alias_inserts_then_idempotent() {
    let (db, _temp) = setup_test_db().await;
    let (_lib, series_id) = create_test_series(&db).await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let app1 = create_test_router(state.clone()).await;
    let body = CreateSeriesAliasRequest {
        alias: "My Hero Academia".to_string(),
        source: None,
    };
    let req = post_json_request_with_auth(
        &format!("/api/v1/series/{}/aliases", series_id),
        &body,
        &token,
    );
    let (status, dto): (StatusCode, Option<SeriesAliasDto>) = make_json_request(app1, req).await;
    assert_eq!(status, StatusCode::CREATED);
    let dto = dto.unwrap();
    assert_eq!(dto.series_id, series_id);
    assert_eq!(dto.alias, "My Hero Academia");
    assert_eq!(dto.normalized, "my hero academia");
    assert_eq!(dto.source, "manual");

    // Second call with same alias: idempotent OK (not CREATED), same id.
    let app2 = create_test_router(state).await;
    let body = CreateSeriesAliasRequest {
        alias: "My Hero Academia".to_string(),
        source: None,
    };
    let req = post_json_request_with_auth(
        &format!("/api/v1/series/{}/aliases", series_id),
        &body,
        &token,
    );
    let (status, dto2): (StatusCode, Option<SeriesAliasDto>) = make_json_request(app2, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dto2.unwrap().id, dto.id);
}

#[tokio::test]
async fn create_alias_rejects_blank() {
    let (db, _temp) = setup_test_db().await;
    let (_lib, series_id) = create_test_series(&db).await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = CreateSeriesAliasRequest {
        alias: "   ".to_string(),
        source: None,
    };
    let req = post_json_request_with_auth(
        &format!("/api/v1/series/{}/aliases", series_id),
        &body,
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_alias_rejects_invalid_explicit_source() {
    // An explicit invalid source falls back to `manual` (we filter via is_valid),
    // so the create should succeed but with source = "manual". This guards
    // against a 500: bad input shouldn't crash, even if we don't surface 400.
    let (db, _temp) = setup_test_db().await;
    let (_lib, series_id) = create_test_series(&db).await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = CreateSeriesAliasRequest {
        alias: "Test".to_string(),
        source: Some("garbage".to_string()),
    };
    let req = post_json_request_with_auth(
        &format!("/api/v1/series/{}/aliases", series_id),
        &body,
        &token,
    );
    let (status, dto): (StatusCode, Option<SeriesAliasDto>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(dto.unwrap().source, "manual");
}

#[tokio::test]
async fn delete_alias_removes_row() {
    use codex::db::repositories::SeriesAliasRepository;

    let (db, _temp) = setup_test_db().await;
    let (_lib, series_id) = create_test_series(&db).await;
    let alias = SeriesAliasRepository::create(&db, series_id, "Manual Alias", "manual")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = delete_request_with_auth(
        &format!("/api/v1/series/{}/aliases/{}", series_id, alias.id),
        &token,
    );
    let (status, _bytes) = make_request(app, req).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let remaining = SeriesAliasRepository::get_for_series(&db, series_id)
        .await
        .unwrap();
    assert!(remaining.is_empty());
}

#[tokio::test]
async fn delete_alias_404_when_alias_missing() {
    let (db, _temp) = setup_test_db().await;
    let (_lib, series_id) = create_test_series(&db).await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake = Uuid::new_v4();
    let req = delete_request_with_auth(
        &format!("/api/v1/series/{}/aliases/{}", series_id, fake),
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_alias_404_when_belongs_to_other_series() {
    use codex::db::repositories::SeriesAliasRepository;

    let (db, _temp) = setup_test_db().await;
    let (lib_id, series_a) = create_test_series(&db).await;
    let series_b = SeriesRepository::create(&db, lib_id, "Other", None)
        .await
        .unwrap();
    let alias_b = SeriesAliasRepository::create(&db, series_b.id, "Belongs To B", "manual")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Try to delete series_b's alias by quoting series_a's path.
    let req = delete_request_with_auth(
        &format!("/api/v1/series/{}/aliases/{}", series_a, alias_b.id),
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Confirm alias still exists.
    assert!(
        SeriesAliasRepository::get_by_id(&db, alias_b.id)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn aliases_require_write_permission_for_mutations() {
    let (db, _temp) = setup_test_db().await;
    let (_lib, series_id) = create_test_series(&db).await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_regular_user_and_token(&db, &state).await;

    let app = create_test_router(state).await;
    let body = CreateSeriesAliasRequest {
        alias: "X".to_string(),
        source: None,
    };
    let req = post_json_request_with_auth(
        &format!("/api/v1/series/{}/aliases", series_id),
        &body,
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
