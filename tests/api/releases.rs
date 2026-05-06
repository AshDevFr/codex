//! Integration tests for the release ledger and release-source admin endpoints.

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::release::{
    BulkReleaseAction, BulkReleaseActionRequest, BulkReleaseActionResponse, DeleteReleaseResponse,
    PollNowResponse, ReleaseFacetsResponse, ReleaseLedgerEntryDto, ReleaseSourceDto,
    ReleaseSourceListResponse, ResetReleaseSourceResponse, UpdateReleaseLedgerEntryRequest,
    UpdateReleaseSourceRequest,
};
use codex::db::ScanningStrategy;
use codex::db::entities::release_sources::kind;
use codex::db::repositories::{
    LibraryRepository, NewReleaseEntry, NewReleaseSource, ReleaseLedgerRepository,
    ReleaseSourceRepository, ReleaseSourceUpdate, SeriesRepository, UserRepository,
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
            media_url: None,
            media_url_kind: None,
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
        assert_eq!(
            entry.series_title, "Series",
            "DTO should carry the series title joined from the series row"
        );
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
    assert_eq!(
        body.data[0].series_title, "Series",
        "inbox DTO should carry the series title for cross-series rendering"
    );
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
        cron_schedule: Some(Some("0 */6 * * *".to_string())),
        ..Default::default()
    };
    let req =
        patch_json_request_with_auth(&format!("/api/v1/release-sources/{}", id), &body, &token);
    let (status, dto): (StatusCode, Option<ReleaseSourceDto>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let dto = dto.unwrap();
    assert!(!dto.enabled);
    assert_eq!(dto.cron_schedule.as_deref(), Some("0 */6 * * *"));
    assert_eq!(dto.effective_cron_schedule, "0 */6 * * *");
}

#[tokio::test]
async fn patch_source_rejects_invalid_cron() {
    let (db, _temp) = setup_test_db().await;
    let id = make_source(&db, "nyaa:user:tsuna69").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = UpdateReleaseSourceRequest {
        cron_schedule: Some(Some("not a cron".to_string())),
        ..Default::default()
    };
    let req =
        patch_json_request_with_auth(&format!("/api/v1/release-sources/{}", id), &body, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn patch_source_clears_cron_with_explicit_null() {
    let (db, _temp) = setup_test_db().await;
    let id = make_source(&db, "nyaa:user:tsuna69").await;

    // Seed a per-source override.
    ReleaseSourceRepository::update(
        &db,
        id,
        ReleaseSourceUpdate {
            cron_schedule: Some(Some("0 */6 * * *".to_string())),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Send `cron_schedule: null` to clear the override.
    let body = serde_json::json!({ "cronSchedule": null });
    let req =
        patch_json_request_with_auth(&format!("/api/v1/release-sources/{}", id), &body, &token);
    let (status, dto): (StatusCode, Option<ReleaseSourceDto>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let dto = dto.unwrap();
    assert!(dto.cron_schedule.is_none(), "override cleared");
    // effectiveCronSchedule falls through to the server-wide default.
    assert!(!dto.effective_cron_schedule.is_empty());
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
// POST /release-sources/{id}/poll-now
// =============================================================================

#[tokio::test]
async fn poll_now_enqueues_task_when_source_exists() {
    let (db, _temp) = setup_test_db().await;
    let id = make_source(&db, "nyaa:user:tsuna69").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = post_request_with_auth(&format!("/api/v1/release-sources/{}/poll-now", id), &token);
    let (status, body): (StatusCode, Option<PollNowResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let body = body.unwrap();
    assert_eq!(body.status, "enqueued");
    assert!(body.message.contains("task_id="));

    // Verify the task landed on the queue.
    use codex::db::repositories::TaskRepository;
    let pending = TaskRepository::list(
        &db,
        Some("pending".to_string()),
        Some("poll_release_source".to_string()),
        Some(10),
    )
    .await
    .unwrap();
    assert!(
        !pending.is_empty(),
        "expected a poll_release_source task to be pending"
    );
}

#[tokio::test]
async fn poll_now_dedupes_concurrent_requests_onto_in_flight_task() {
    // Regression: clicking "Poll now" twice quickly previously enqueued
    // two independent tasks. With worker_count >= 2 they'd race on
    // last_summary / last_polled_at writes and overlap upstream fetches.
    // We now coalesce onto the existing pending/processing task.
    use codex::db::repositories::TaskRepository;

    let (db, _temp) = setup_test_db().await;
    let id = make_source(&db, "nyaa:user:tsuna69").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app1 = create_test_router(state.clone()).await;
    let app2 = create_test_router(state).await;

    // First click: enqueues a fresh task.
    let req = post_request_with_auth(&format!("/api/v1/release-sources/{}/poll-now", id), &token);
    let (s1, b1): (StatusCode, Option<PollNowResponse>) = make_json_request(app1, req).await;
    assert_eq!(s1, StatusCode::ACCEPTED);
    let b1 = b1.unwrap();
    assert_eq!(b1.status, "enqueued");

    // Second click while the first is still pending: coalesce.
    let req = post_request_with_auth(&format!("/api/v1/release-sources/{}/poll-now", id), &token);
    let (s2, b2): (StatusCode, Option<PollNowResponse>) = make_json_request(app2, req).await;
    assert_eq!(s2, StatusCode::ACCEPTED);
    let b2 = b2.unwrap();
    assert_eq!(
        b2.status, "already_running",
        "second poll-now must coalesce onto the in-flight task"
    );
    assert!(
        b2.message.contains("coalesced"),
        "human-readable message should explain the coalesce"
    );

    // Only one task should sit on the queue, not two.
    let pending = TaskRepository::list(
        &db,
        Some("pending".to_string()),
        Some("poll_release_source".to_string()),
        Some(10),
    )
    .await
    .unwrap();
    assert_eq!(
        pending.len(),
        1,
        "duplicate poll-now must not stack tasks; got {} pending",
        pending.len()
    );
}

#[tokio::test]
async fn poll_now_conflicts_when_source_disabled() {
    use codex::db::repositories::{ReleaseSourceRepository, ReleaseSourceUpdate};

    let (db, _temp) = setup_test_db().await;
    let id = make_source(&db, "nyaa:user:tsuna69").await;
    // Disable it.
    ReleaseSourceRepository::update(
        &db,
        id,
        ReleaseSourceUpdate {
            enabled: Some(false),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = post_request_with_auth(&format!("/api/v1/release-sources/{}/poll-now", id), &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::CONFLICT);
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

// =============================================================================
// POST /release-sources/{id}/reset
// =============================================================================

#[tokio::test]
async fn reset_clears_ledger_rows_and_poll_state() {
    let (db, _temp) = setup_test_db().await;
    let series = make_series(&db).await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    let other_source = make_source(&db, "nyaa:user:other").await;

    record_announced(&db, series, source, "rel-1").await;
    record_announced(&db, series, source, "rel-2").await;
    // A row on a different source must NOT be touched.
    record_announced(&db, series, other_source, "rel-keep").await;

    // Seed poll state on the target source so we can prove it's cleared.
    ReleaseSourceRepository::record_poll_success(
        &db,
        source,
        chrono::Utc::now(),
        Some("\"etag-1\"".to_string()),
        Some("Fetched 2 items".to_string()),
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = post_request_with_auth(&format!("/api/v1/release-sources/{}/reset", source), &token);
    let (status, body): (StatusCode, Option<ResetReleaseSourceResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.unwrap().deleted_ledger_entries, 2);

    // Target source: ledger rows gone, poll state cleared.
    let after = ReleaseSourceRepository::get_by_id(&db, source)
        .await
        .unwrap()
        .unwrap();
    assert!(after.etag.is_none());
    assert!(after.last_polled_at.is_none());
    assert!(after.last_summary.is_none());

    // Other source's row survives.
    let surviving = ReleaseLedgerRepository::list_for_series(&db, series, None, 100, 0)
        .await
        .unwrap();
    assert_eq!(surviving.len(), 1);
    assert_eq!(surviving[0].source_id, other_source);
    assert_eq!(surviving[0].external_release_id, "rel-keep");
}

#[tokio::test]
async fn reset_preserves_user_managed_source_fields() {
    use codex::db::repositories::ReleaseSourceUpdate;

    let (db, _temp) = setup_test_db().await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;

    // Admin disables the source and overrides the schedule.
    ReleaseSourceRepository::update(
        &db,
        source,
        ReleaseSourceUpdate {
            enabled: Some(false),
            cron_schedule: Some(Some("0 */6 * * *".to_string())),
            display_name: Some("Custom Name".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = post_request_with_auth(&format!("/api/v1/release-sources/{}/reset", source), &token);
    let (status, _): (StatusCode, Option<ResetReleaseSourceResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);

    let after = ReleaseSourceRepository::get_by_id(&db, source)
        .await
        .unwrap()
        .unwrap();
    assert!(!after.enabled, "user-set enabled flag must survive a reset");
    assert_eq!(
        after.cron_schedule.as_deref(),
        Some("0 */6 * * *"),
        "schedule override survives"
    );
    assert_eq!(after.display_name, "Custom Name", "display name preserved");
}

#[tokio::test]
async fn reset_404_when_source_missing() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = post_request_with_auth(
        &format!("/api/v1/release-sources/{}/reset", Uuid::new_v4()),
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn reset_requires_plugins_manage() {
    let (db, _temp) = setup_test_db().await;
    let id = make_source(&db, "nyaa:user:tsuna69").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_reader_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = post_request_with_auth(&format!("/api/v1/release-sources/{}/reset", id), &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// =============================================================================
// GET /release-sources/applicability  (round D)
// =============================================================================

/// Helper: insert an enabled plugin row carrying a manifest with the
/// `releaseSource` capability, optionally scoped to `library_ids`.
async fn make_release_source_plugin(
    db: &DatabaseConnection,
    name: &str,
    display_name: &str,
    library_ids: Vec<Uuid>,
) -> Uuid {
    use codex::db::repositories::PluginsRepository;
    use serde_json::json;

    let plugin = PluginsRepository::create(
        db,
        name,
        display_name,
        Some("test release-source plugin"),
        "system",
        "echo",
        vec!["ok".to_string()],
        vec![],
        None,
        vec![],
        vec![],
        library_ids,
        None,
        "none",
        None,
        true, // enabled
        None,
        None,
    )
    .await
    .unwrap();

    // Manifest must declare the release_source capability for the
    // applicability handler to count this plugin.
    let manifest = json!({
        "name": name,
        "displayName": display_name,
        "version": "1.0.0",
        "protocolVersion": "1.0",
        "capabilities": {
            "releaseSource": {
                "kinds": ["rss-uploader"],
                "requiresAliases": true,
                "canAnnounceChapters": true,
                "canAnnounceVolumes": true,
                "defaultPollIntervalS": 3600
            }
        }
    });
    PluginsRepository::update_manifest(db, plugin.id, Some(manifest))
        .await
        .unwrap();
    plugin.id
}

/// Helper: insert an enabled plugin without the release-source capability.
async fn make_metadata_only_plugin(db: &DatabaseConnection, name: &str) -> Uuid {
    use codex::db::repositories::PluginsRepository;
    use serde_json::json;

    let plugin = PluginsRepository::create(
        db,
        name,
        name,
        None,
        "system",
        "echo",
        vec!["ok".to_string()],
        vec![],
        None,
        vec![],
        vec![],
        vec![],
        None,
        "none",
        None,
        true,
        None,
        None,
    )
    .await
    .unwrap();
    let manifest = json!({
        "name": name,
        "displayName": name,
        "version": "1.0.0",
        "protocolVersion": "1.0",
        "capabilities": {
            "metadataProvider": ["series"]
        }
    });
    PluginsRepository::update_manifest(db, plugin.id, Some(manifest))
        .await
        .unwrap();
    plugin.id
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplicabilityResponseDto {
    applicable: bool,
    plugin_display_names: Vec<String>,
}

#[tokio::test]
async fn applicability_false_when_no_release_source_plugins() {
    let (db, _temp) = setup_test_db().await;
    // A metadata-only plugin must not register as applicable.
    make_metadata_only_plugin(&db, "metadata-only").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth("/api/v1/release-sources/applicability", &token);
    let (status, body): (StatusCode, Option<ApplicabilityResponseDto>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();
    assert!(!body.applicable);
    assert!(body.plugin_display_names.is_empty());
}

#[tokio::test]
async fn applicability_true_when_global_plugin_no_library_filter() {
    let (db, _temp) = setup_test_db().await;
    // Empty library_ids means "all libraries".
    make_release_source_plugin(&db, "release-nyaa", "Nyaa", vec![]).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth("/api/v1/release-sources/applicability", &token);
    let (status, body): (StatusCode, Option<ApplicabilityResponseDto>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();
    assert!(body.applicable);
    assert_eq!(body.plugin_display_names, vec!["Nyaa".to_string()]);
}

#[tokio::test]
async fn applicability_filters_by_library_when_plugin_is_scoped() {
    let (db, _temp) = setup_test_db().await;
    let lib_a = codex::db::repositories::LibraryRepository::create(
        &db,
        "A",
        "/a",
        codex::db::ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let lib_b = codex::db::repositories::LibraryRepository::create(
        &db,
        "B",
        "/b",
        codex::db::ScanningStrategy::Default,
    )
    .await
    .unwrap();
    // Plugin scoped to lib_a only.
    make_release_source_plugin(&db, "release-nyaa", "Nyaa", vec![lib_a.id]).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Query for lib_a → applicable.
    let app = create_test_router(state.clone()).await;
    let req = get_request_with_auth(
        &format!(
            "/api/v1/release-sources/applicability?libraryId={}",
            lib_a.id
        ),
        &token,
    );
    let (s_a, b_a): (StatusCode, Option<ApplicabilityResponseDto>) =
        make_json_request(app, req).await;
    assert_eq!(s_a, StatusCode::OK);
    assert!(b_a.unwrap().applicable);

    // Query for lib_b → not applicable.
    let app = create_test_router(state.clone()).await;
    let req = get_request_with_auth(
        &format!(
            "/api/v1/release-sources/applicability?libraryId={}",
            lib_b.id
        ),
        &token,
    );
    let (s_b, b_b): (StatusCode, Option<ApplicabilityResponseDto>) =
        make_json_request(app, req).await;
    assert_eq!(s_b, StatusCode::OK);
    let b_b = b_b.unwrap();
    assert!(!b_b.applicable);
    assert!(b_b.plugin_display_names.is_empty());

    // No libraryId filter → applicable (the plugin still exists globally).
    let app = create_test_router(state).await;
    let req = get_request_with_auth("/api/v1/release-sources/applicability", &token);
    let (s_all, b_all): (StatusCode, Option<ApplicabilityResponseDto>) =
        make_json_request(app, req).await;
    assert_eq!(s_all, StatusCode::OK);
    assert!(b_all.unwrap().applicable);
}

#[tokio::test]
async fn applicability_global_plugin_applies_to_any_library() {
    let (db, _temp) = setup_test_db().await;
    let lib = codex::db::repositories::LibraryRepository::create(
        &db,
        "L",
        "/l",
        codex::db::ScanningStrategy::Default,
    )
    .await
    .unwrap();
    // Global (empty library_ids) plugin should match any libraryId query.
    make_release_source_plugin(&db, "release-mu", "MangaUpdates", vec![]).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth(
        &format!("/api/v1/release-sources/applicability?libraryId={}", lib.id),
        &token,
    );
    let (status, body): (StatusCode, Option<ApplicabilityResponseDto>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();
    assert!(body.applicable);
    assert_eq!(body.plugin_display_names, vec!["MangaUpdates".to_string()]);
}

#[tokio::test]
async fn applicability_aggregates_multiple_plugins() {
    let (db, _temp) = setup_test_db().await;
    make_release_source_plugin(&db, "release-nyaa", "Nyaa", vec![]).await;
    make_release_source_plugin(&db, "release-mu", "MangaUpdates", vec![]).await;
    // A non-release plugin should not bleed into the response.
    make_metadata_only_plugin(&db, "metadata-only").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth("/api/v1/release-sources/applicability", &token);
    let (status, body): (StatusCode, Option<ApplicabilityResponseDto>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();
    assert!(body.applicable);
    assert_eq!(body.plugin_display_names.len(), 2);
    assert!(body.plugin_display_names.contains(&"Nyaa".to_string()));
    assert!(
        body.plugin_display_names
            .contains(&"MangaUpdates".to_string())
    );
}

#[tokio::test]
async fn applicability_requires_series_read() {
    let (db, _temp) = setup_test_db().await;
    make_release_source_plugin(&db, "release-nyaa", "Nyaa", vec![]).await;

    // A user with no role at all (not even reader) — but our `create_reader_and_token`
    // creates a regular non-admin user who DOES have SeriesRead, so the check
    // would pass. Instead we exercise the unauthenticated path here, which is
    // the only realistic 401/403 surface — every authenticated user has
    // SeriesRead. This still proves the route enforces auth.
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let req = get_request("/api/v1/release-sources/applicability");
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn applicability_skips_disabled_plugins() {
    use codex::db::repositories::PluginsRepository;

    let (db, _temp) = setup_test_db().await;
    let plugin_id = make_release_source_plugin(&db, "release-nyaa", "Nyaa", vec![]).await;
    // Disable it — should drop out of the applicability list.
    PluginsRepository::disable(&db, plugin_id, None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth("/api/v1/release-sources/applicability", &token);
    let (status, body): (StatusCode, Option<ApplicabilityResponseDto>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();
    assert!(
        !body.applicable,
        "disabled plugins must not contribute to applicability"
    );
}

// =============================================================================
// GET /releases (state=all + libraryId filter)
// =============================================================================

async fn make_series_in(db: &DatabaseConnection, library_name: &str, series_name: &str) -> Uuid {
    let library = LibraryRepository::create(
        db,
        library_name,
        &format!("/{}", library_name),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let series = SeriesRepository::create(db, library.id, series_name, None)
        .await
        .unwrap();
    series.id
}

async fn library_id_for_series(db: &DatabaseConnection, series_id: Uuid) -> Uuid {
    SeriesRepository::get_by_id(db, series_id)
        .await
        .unwrap()
        .unwrap()
        .library_id
}

#[tokio::test]
async fn inbox_state_all_returns_all_states() {
    let (db, _temp) = setup_test_db().await;
    let series = make_series(&db).await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    let r1 = record_announced(&db, series, source, "rel-1").await;
    record_announced(&db, series, source, "rel-2").await;
    ReleaseLedgerRepository::set_state(&db, r1, "dismissed")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth("/api/v1/releases?state=all", &token);
    let (status, body): (
        StatusCode,
        Option<PaginatedDtoResponse<ReleaseLedgerEntryDto>>,
    ) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();
    assert_eq!(
        body.total, 2,
        "state=all must return both announced and dismissed rows"
    );
}

#[tokio::test]
async fn inbox_filters_by_library_id() {
    let (db, _temp) = setup_test_db().await;
    let s_manga = make_series_in(&db, "Manga", "Manga Series").await;
    let s_books = make_series_in(&db, "Books", "Book Series").await;
    let manga_lib = library_id_for_series(&db, s_manga).await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    record_announced(&db, s_manga, source, "rel-manga").await;
    record_announced(&db, s_books, source, "rel-book").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth(&format!("/api/v1/releases?libraryId={}", manga_lib), &token);
    let (status, body): (
        StatusCode,
        Option<PaginatedDtoResponse<ReleaseLedgerEntryDto>>,
    ) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();
    assert_eq!(body.total, 1);
    assert_eq!(body.data[0].external_release_id, "rel-manga");
}

// =============================================================================
// GET /releases/facets
// =============================================================================

#[tokio::test]
async fn facets_returns_distinct_languages_libraries_series() {
    let (db, _temp) = setup_test_db().await;
    let s_manga = make_series_in(&db, "Manga", "Manga Series").await;
    let s_books = make_series_in(&db, "Books", "Book Series").await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    record_announced(&db, s_manga, source, "rel-1").await;
    record_announced(&db, s_manga, source, "rel-2").await;
    record_announced(&db, s_books, source, "rel-3").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth("/api/v1/releases/facets", &token);
    let (status, body): (StatusCode, Option<ReleaseFacetsResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();

    // One language ("en"), two libraries, two series.
    assert_eq!(body.languages.len(), 1);
    assert_eq!(body.languages[0].language, "en");
    assert_eq!(body.languages[0].count, 3);

    assert_eq!(body.libraries.len(), 2);
    let manga_lib = body
        .libraries
        .iter()
        .find(|l| l.library_name == "Manga")
        .expect("Manga library facet present");
    assert_eq!(manga_lib.count, 2);

    assert_eq!(body.series.len(), 2);
    let manga_series = body
        .series
        .iter()
        .find(|s| s.series_title == "Manga Series")
        .expect("Manga series facet present");
    assert_eq!(manga_series.library_name, "Manga");
    assert_eq!(manga_series.count, 2);
}

#[tokio::test]
async fn facets_excludes_self_dimension_so_dropdowns_dont_collapse() {
    // When the caller passes seriesId=X, the *series* facet should still
    // list all series (not just X). Otherwise the dropdown would collapse
    // to the active selection and the user couldn't switch series.
    let (db, _temp) = setup_test_db().await;
    let s1 = make_series_in(&db, "Manga", "S1").await;
    let s2 = make_series_in(&db, "Manga", "S2").await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    record_announced(&db, s1, source, "rel-1").await;
    record_announced(&db, s2, source, "rel-2").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth(&format!("/api/v1/releases/facets?seriesId={}", s1), &token);
    let (status, body): (StatusCode, Option<ReleaseFacetsResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();
    assert_eq!(
        body.series.len(),
        2,
        "series facet must not be filtered by the active seriesId"
    );
}

#[tokio::test]
async fn facets_requires_auth() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let req = get_request("/api/v1/releases/facets");
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// =============================================================================
// DELETE /releases/{id}
// =============================================================================

#[tokio::test]
async fn delete_release_removes_row_and_clears_source_etag() {
    let (db, _temp) = setup_test_db().await;
    let series = make_series(&db).await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    let id = record_announced(&db, series, source, "rel-1").await;

    // Seed an etag so we can verify it gets cleared.
    ReleaseSourceRepository::record_poll_success(
        &db,
        source,
        chrono::Utc::now(),
        Some("\"abc123\"".to_string()),
        None,
    )
    .await
    .unwrap();
    let pre = ReleaseSourceRepository::get_by_id(&db, source)
        .await
        .unwrap()
        .unwrap();
    assert!(pre.etag.is_some(), "test setup: etag should be set");

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = delete_request_with_auth(&format!("/api/v1/releases/{}", id), &token);
    let (status, body): (StatusCode, Option<DeleteReleaseResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.unwrap().deleted);

    // Row gone.
    assert!(
        ReleaseLedgerRepository::get_by_id(&db, id)
            .await
            .unwrap()
            .is_none()
    );
    // Etag cleared.
    let post = ReleaseSourceRepository::get_by_id(&db, source)
        .await
        .unwrap()
        .unwrap();
    assert!(
        post.etag.is_none(),
        "delete must clear the source's etag so the next poll re-fetches"
    );
}

#[tokio::test]
async fn delete_release_404_for_missing() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = delete_request_with_auth(&format!("/api/v1/releases/{}", Uuid::new_v4()), &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_release_forbidden_for_reader() {
    let (db, _temp) = setup_test_db().await;
    let series = make_series(&db).await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    let id = record_announced(&db, series, source, "rel-1").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_reader_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = delete_request_with_auth(&format!("/api/v1/releases/{}", id), &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// =============================================================================
// POST /releases/bulk
// =============================================================================

#[tokio::test]
async fn bulk_dismiss_updates_state_for_listed_ids() {
    let (db, _temp) = setup_test_db().await;
    let series = make_series(&db).await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    let id1 = record_announced(&db, series, source, "rel-1").await;
    let id2 = record_announced(&db, series, source, "rel-2").await;
    let id3 = record_announced(&db, series, source, "rel-3").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = BulkReleaseActionRequest {
        ids: vec![id1, id2],
        action: BulkReleaseAction::Dismiss,
    };
    let req = post_json_request_with_auth("/api/v1/releases/bulk", &body, &token);
    let (status, resp): (StatusCode, Option<BulkReleaseActionResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let resp = resp.unwrap();
    assert_eq!(resp.affected, 2);
    assert_eq!(resp.action, BulkReleaseAction::Dismiss);

    // Selected rows were dismissed; the third stays announced.
    assert_eq!(
        ReleaseLedgerRepository::get_by_id(&db, id1)
            .await
            .unwrap()
            .unwrap()
            .state,
        "dismissed"
    );
    assert_eq!(
        ReleaseLedgerRepository::get_by_id(&db, id3)
            .await
            .unwrap()
            .unwrap()
            .state,
        "announced"
    );
}

#[tokio::test]
async fn bulk_delete_clears_etags_on_affected_sources_only() {
    let (db, _temp) = setup_test_db().await;
    let series = make_series(&db).await;
    let src_a = make_source(&db, "nyaa:user:a").await;
    let src_b = make_source(&db, "nyaa:user:b").await;
    let id_a = record_announced(&db, series, src_a, "rel-a").await;
    let _id_b = record_announced(&db, series, src_b, "rel-b").await;

    // Seed etags on both sources.
    for src in [src_a, src_b] {
        ReleaseSourceRepository::record_poll_success(
            &db,
            src,
            chrono::Utc::now(),
            Some("\"etag\"".to_string()),
            None,
        )
        .await
        .unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = BulkReleaseActionRequest {
        ids: vec![id_a],
        action: BulkReleaseAction::Delete,
    };
    let req = post_json_request_with_auth("/api/v1/releases/bulk", &body, &token);
    let (status, resp): (StatusCode, Option<BulkReleaseActionResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp.unwrap().affected, 1);

    // src_a touched (etag cleared), src_b untouched (etag preserved).
    assert!(
        ReleaseSourceRepository::get_by_id(&db, src_a)
            .await
            .unwrap()
            .unwrap()
            .etag
            .is_none()
    );
    assert!(
        ReleaseSourceRepository::get_by_id(&db, src_b)
            .await
            .unwrap()
            .unwrap()
            .etag
            .is_some(),
        "untouched sources must keep their etag"
    );
}

#[tokio::test]
async fn bulk_rejects_empty_ids() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = BulkReleaseActionRequest {
        ids: vec![],
        action: BulkReleaseAction::Dismiss,
    };
    let req = post_json_request_with_auth("/api/v1/releases/bulk", &body, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn bulk_forbidden_for_reader() {
    let (db, _temp) = setup_test_db().await;
    let series = make_series(&db).await;
    let source = make_source(&db, "nyaa:user:tsuna69").await;
    let id = record_announced(&db, series, source, "rel-1").await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_reader_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = BulkReleaseActionRequest {
        ids: vec![id],
        action: BulkReleaseAction::Dismiss,
    };
    let req = post_json_request_with_auth("/api/v1/releases/bulk", &body, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
