//! Integration tests for the release ledger and release-source admin endpoints.

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::release::{
    PollNowResponse, ReleaseLedgerEntryDto, ReleaseSourceDto, ReleaseSourceListResponse,
    UpdateReleaseLedgerEntryRequest, UpdateReleaseSourceRequest,
};
use codex::db::ScanningStrategy;
use codex::db::entities::release_sources::kind;
use codex::db::repositories::{
    LibraryRepository, NewReleaseEntry, NewReleaseSource, ReleaseLedgerRepository,
    ReleaseSourceRepository, SeriesRepository, UserRepository,
};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

// =============================================================================
// Helpers
// =============================================================================

async fn create_admin_and_token(
    db: &DatabaseConnection,
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

async fn create_reader_and_token(
    db: &DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> String {
    let password_hash = password::hash_password("reader123").unwrap();
    let user = create_test_user("reader", "reader@example.com", &password_hash, false);
    let created = UserRepository::create(db, &user).await.unwrap();
    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

async fn make_series(db: &DatabaseConnection) -> Uuid {
    let library = LibraryRepository::create(db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(db, library.id, "Series", None)
        .await
        .unwrap();
    series.id
}

async fn make_source(db: &DatabaseConnection, source_key: &str) -> Uuid {
    let s = ReleaseSourceRepository::create(
        db,
        NewReleaseSource {
            plugin_id: "release-nyaa".to_string(),
            source_key: source_key.to_string(),
            display_name: format!("Nyaa - {}", source_key),
            kind: kind::RSS_UPLOADER.to_string(),
            poll_interval_s: 3600,
            enabled: None,
            config: None,
        },
    )
    .await
    .unwrap();
    s.id
}

async fn record_announced(
    db: &DatabaseConnection,
    series_id: Uuid,
    source_id: Uuid,
    external_id: &str,
) -> Uuid {
    let outcome = ReleaseLedgerRepository::record(
        db,
        NewReleaseEntry {
            series_id,
            source_id,
            external_release_id: external_id.to_string(),
            info_hash: None,
            chapter: Some(143.0),
            volume: None,
            language: Some("en".to_string()),
            format_hints: None,
            group_or_uploader: Some("uploader".to_string()),
            payload_url: format!("https://nyaa.si/view/{}", external_id),
            confidence: 0.95,
            metadata: None,
            observed_at: chrono::Utc::now(),
        },
    )
    .await
    .unwrap();
    outcome.row.id
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PaginatedDtoResponse<T> {
    data: Vec<T>,
    page: u64,
    page_size: u64,
    total: u64,
}

// =============================================================================
// GET /series/{id}/releases
// =============================================================================

#[tokio::test]
async fn list_series_releases_returns_entries_for_series() {
    let (db, _temp) = setup_test_db().await;
    let series = make_series(&db).await;
    let other = make_series(&db).await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    record_announced(&db, series, source, "rel-1").await;
    record_announced(&db, series, source, "rel-2").await;
    record_announced(&db, other, source, "rel-3").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth(&format!("/api/v1/series/{}/releases", series), &token);
    let (status, body): (
        StatusCode,
        Option<PaginatedDtoResponse<ReleaseLedgerEntryDto>>,
    ) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();
    assert_eq!(body.total, 2);
    assert_eq!(body.data.len(), 2);
    for entry in &body.data {
        assert_eq!(entry.series_id, series);
    }
}

#[tokio::test]
async fn list_series_releases_404_when_series_missing() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake = Uuid::new_v4();
    let req = get_request_with_auth(&format!("/api/v1/series/{}/releases", fake), &token);
    let (status, _err): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn list_series_releases_filters_by_state() {
    let (db, _temp) = setup_test_db().await;
    let series = make_series(&db).await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    let r1 = record_announced(&db, series, source, "rel-1").await;
    let _r2 = record_announced(&db, series, source, "rel-2").await;

    // Dismiss r1.
    ReleaseLedgerRepository::set_state(&db, r1, "dismissed")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth(
        &format!("/api/v1/series/{}/releases?state=announced", series),
        &token,
    );
    let (status, body): (
        StatusCode,
        Option<PaginatedDtoResponse<ReleaseLedgerEntryDto>>,
    ) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();
    assert_eq!(body.total, 1);
    assert_eq!(body.data[0].state, "announced");
}

#[tokio::test]
async fn list_series_releases_rejects_invalid_state() {
    let (db, _temp) = setup_test_db().await;
    let series = make_series(&db).await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth(
        &format!("/api/v1/series/{}/releases?state=garbage", series),
        &token,
    );
    let (status, _err): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_series_releases_requires_auth() {
    let (db, _temp) = setup_test_db().await;
    let series = make_series(&db).await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let req = get_request(&format!("/api/v1/series/{}/releases", series));
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// =============================================================================
// GET /releases (inbox)
// =============================================================================

#[tokio::test]
async fn inbox_lists_announced_by_default() {
    let (db, _temp) = setup_test_db().await;
    let series = make_series(&db).await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    let r1 = record_announced(&db, series, source, "rel-1").await;
    let _r2 = record_announced(&db, series, source, "rel-2").await;
    ReleaseLedgerRepository::set_state(&db, r1, "dismissed")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth("/api/v1/releases", &token);
    let (status, body): (
        StatusCode,
        Option<PaginatedDtoResponse<ReleaseLedgerEntryDto>>,
    ) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();
    assert_eq!(body.total, 1);
    assert_eq!(body.data[0].external_release_id, "rel-2");
    assert_eq!(body.page, 1);
    assert_eq!(body.page_size, 50);
}

#[tokio::test]
async fn inbox_filters_by_series() {
    let (db, _temp) = setup_test_db().await;
    let s1 = make_series(&db).await;
    let s2 = make_series(&db).await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    record_announced(&db, s1, source, "rel-1").await;
    record_announced(&db, s2, source, "rel-2").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth(&format!("/api/v1/releases?seriesId={}", s1), &token);
    let (status, body): (
        StatusCode,
        Option<PaginatedDtoResponse<ReleaseLedgerEntryDto>>,
    ) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();
    assert_eq!(body.total, 1);
    assert_eq!(body.data[0].external_release_id, "rel-1");
}

// =============================================================================
// PATCH /releases/{id}
// =============================================================================

#[tokio::test]
async fn patch_release_state_transitions() {
    let (db, _temp) = setup_test_db().await;
    let series = make_series(&db).await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    let id = record_announced(&db, series, source, "rel-1").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = UpdateReleaseLedgerEntryRequest {
        state: Some("dismissed".to_string()),
    };
    let req = patch_json_request_with_auth(&format!("/api/v1/releases/{}", id), &body, &token);
    let (status, dto): (StatusCode, Option<ReleaseLedgerEntryDto>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dto.unwrap().state, "dismissed");
}

#[tokio::test]
async fn patch_release_state_rejects_invalid() {
    let (db, _temp) = setup_test_db().await;
    let series = make_series(&db).await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    let id = record_announced(&db, series, source, "rel-1").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = UpdateReleaseLedgerEntryRequest {
        state: Some("garbage".to_string()),
    };
    let req = patch_json_request_with_auth(&format!("/api/v1/releases/{}", id), &body, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn patch_release_404_for_missing() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = UpdateReleaseLedgerEntryRequest {
        state: Some("dismissed".to_string()),
    };
    let req = patch_json_request_with_auth(
        &format!("/api/v1/releases/{}", Uuid::new_v4()),
        &body,
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn dismiss_release_convenience_post() {
    let (db, _temp) = setup_test_db().await;
    let series = make_series(&db).await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    let id = record_announced(&db, series, source, "rel-1").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = post_request_with_auth(&format!("/api/v1/releases/{}/dismiss", id), &token);
    let (status, dto): (StatusCode, Option<ReleaseLedgerEntryDto>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dto.unwrap().state, "dismissed");
}

#[tokio::test]
async fn mark_release_acquired_convenience_post() {
    let (db, _temp) = setup_test_db().await;
    let series = make_series(&db).await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    let id = record_announced(&db, series, source, "rel-1").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = post_request_with_auth(&format!("/api/v1/releases/{}/mark-acquired", id), &token);
    let (status, dto): (StatusCode, Option<ReleaseLedgerEntryDto>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dto.unwrap().state, "marked_acquired");
}

#[tokio::test]
async fn patch_release_requires_write_permission() {
    let (db, _temp) = setup_test_db().await;
    let series = make_series(&db).await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    let id = record_announced(&db, series, source, "rel-1").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_reader_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = UpdateReleaseLedgerEntryRequest {
        state: Some("dismissed".to_string()),
    };
    let req = patch_json_request_with_auth(&format!("/api/v1/releases/{}", id), &body, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// =============================================================================
// GET /release-sources (admin)
// =============================================================================

#[tokio::test]
async fn list_release_sources_returns_all() {
    let (db, _temp) = setup_test_db().await;
    make_source(&db, "nyaa:user:tsuna69").await;
    make_source(&db, "nyaa:user:other").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth("/api/v1/release-sources", &token);
    let (status, body): (StatusCode, Option<ReleaseSourceListResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.unwrap().sources.len(), 2);
}

#[tokio::test]
async fn list_release_sources_requires_plugins_manage() {
    let (db, _temp) = setup_test_db().await;
    make_source(&db, "nyaa:user:tsuna69").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_reader_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth("/api/v1/release-sources", &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// =============================================================================
// PATCH /release-sources/{id}
// =============================================================================

#[tokio::test]
async fn patch_source_can_disable_and_change_interval() {
    let (db, _temp) = setup_test_db().await;
    let id = make_source(&db, "nyaa:user:tsuna69").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = UpdateReleaseSourceRequest {
        enabled: Some(false),
        poll_interval_s: Some(7200),
        ..Default::default()
    };
    let req =
        patch_json_request_with_auth(&format!("/api/v1/release-sources/{}", id), &body, &token);
    let (status, dto): (StatusCode, Option<ReleaseSourceDto>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let dto = dto.unwrap();
    assert!(!dto.enabled);
    assert_eq!(dto.poll_interval_s, 7200);
}

#[tokio::test]
async fn patch_source_rejects_non_positive_interval() {
    let (db, _temp) = setup_test_db().await;
    let id = make_source(&db, "nyaa:user:tsuna69").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = UpdateReleaseSourceRequest {
        poll_interval_s: Some(0),
        ..Default::default()
    };
    let req =
        patch_json_request_with_auth(&format!("/api/v1/release-sources/{}", id), &body, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn patch_source_404_for_missing() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = UpdateReleaseSourceRequest {
        enabled: Some(false),
        ..Default::default()
    };
    let req = patch_json_request_with_auth(
        &format!("/api/v1/release-sources/{}", Uuid::new_v4()),
        &body,
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn patch_source_requires_plugins_manage() {
    let (db, _temp) = setup_test_db().await;
    let id = make_source(&db, "nyaa:user:tsuna69").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_reader_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = UpdateReleaseSourceRequest {
        enabled: Some(false),
        ..Default::default()
    };
    let req =
        patch_json_request_with_auth(&format!("/api/v1/release-sources/{}", id), &body, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// =============================================================================
// POST /release-sources/{id}/poll-now (Phase 2 stub)
// =============================================================================

#[tokio::test]
async fn poll_now_returns_501_when_source_exists() {
    let (db, _temp) = setup_test_db().await;
    let id = make_source(&db, "nyaa:user:tsuna69").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = post_request_with_auth(&format!("/api/v1/release-sources/{}/poll-now", id), &token);
    let (status, body): (StatusCode, Option<PollNowResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(body.unwrap().status, "not_implemented");
}

#[tokio::test]
async fn poll_now_404_when_source_missing() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = post_request_with_auth(
        &format!("/api/v1/release-sources/{}/poll-now", Uuid::new_v4()),
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn poll_now_requires_plugins_manage() {
    let (db, _temp) = setup_test_db().await;
    let id = make_source(&db, "nyaa:user:tsuna69").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_reader_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = post_request_with_auth(&format!("/api/v1/release-sources/{}/poll-now", id), &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
