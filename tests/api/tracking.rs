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
    assert_eq!(dto.latest_known_chapter, Some(143.0));
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

// =============================================================================
// PATCH /tracking — seed on false -> true transition (round D)
// =============================================================================

/// Create a book with a `book_metadata` row carrying volume/chapter so seed
/// can derive `latest_known_*` and `track_*` flags.
async fn add_classified_book(
    db: &sea_orm::DatabaseConnection,
    series_id: Uuid,
    library_id: Uuid,
    path: &str,
    volume: Option<i32>,
    chapter: Option<f32>,
) {
    use chrono::Utc;
    use codex::db::entities::{book_metadata, books};
    use codex::db::repositories::{BookMetadataRepository, BookRepository};
    use sea_orm::{ActiveModelTrait, Set};

    let book = books::Model {
        id: Uuid::new_v4(),
        series_id,
        library_id,
        file_path: path.to_string(),
        file_name: path.rsplit('/').next().unwrap_or(path).to_string(),
        file_size: 1024,
        file_hash: format!("hash_{}", Uuid::new_v4()),
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
    };
    let created = BookRepository::create(db, &book, None).await.unwrap();
    let meta = BookMetadataRepository::create_with_title_and_number(db, created.id, None, None)
        .await
        .unwrap();
    let mut active: book_metadata::ActiveModel = meta.into();
    active.volume = Set(volume);
    active.chapter = Set(chapter);
    active.update(db).await.unwrap();
}

#[tokio::test]
async fn patch_tracking_seeds_on_false_to_true_transition() {
    use codex::db::repositories::SeriesAliasRepository;

    let (db, _temp) = setup_test_db().await;
    let (lib_id, series_id) = create_test_series(&db).await;

    // Two volume-classified books, no chapters. Seed should:
    //   - insert "Test Series" as a metadata-source alias,
    //   - set latest_known_volume = 7, latest_known_chapter = None,
    //   - set track_volumes = true, track_chapters = false.
    add_classified_book(&db, series_id, lib_id, "/v1.cbz", Some(1), None).await;
    add_classified_book(&db, series_id, lib_id, "/v7.cbz", Some(7), None).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = UpdateSeriesTrackingRequest {
        tracked: Some(true),
        ..Default::default()
    };
    let req = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/tracking", series_id),
        &body,
        &token,
    );
    let (status, dto): (StatusCode, Option<SeriesTrackingDto>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let dto = dto.unwrap();

    assert!(dto.tracked, "user-supplied tracked=true must win");
    assert_eq!(
        dto.latest_known_volume,
        Some(7),
        "seed should derive latest_known_volume from local max"
    );
    assert_eq!(
        dto.latest_known_chapter, None,
        "no books have chapters → latest_known_chapter stays None"
    );
    assert!(
        dto.track_volumes,
        "volume-organized series should keep track_volumes on"
    );
    assert!(
        !dto.track_chapters,
        "no chapter classification → track_chapters should default off"
    );

    // Aliases were seeded.
    let aliases = SeriesAliasRepository::get_for_series(&db, series_id)
        .await
        .unwrap();
    assert!(
        aliases.iter().any(|a| a.alias == "Test Series"),
        "seed should insert the series name as a metadata-source alias; got {:?}",
        aliases.iter().map(|a| &a.alias).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn patch_tracking_user_value_wins_over_seed() {
    let (db, _temp) = setup_test_db().await;
    let (lib_id, series_id) = create_test_series(&db).await;

    // Books would seed latest_known_chapter = 50.0 ...
    add_classified_book(&db, series_id, lib_id, "/c50.cbz", None, Some(50.0)).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // ... but the user explicitly overrides to 100.0 in the same PATCH.
    // Seed runs first, then the user's update is applied on top — so the
    // user's value must win. This is the "explicit override beats seed"
    // contract the seed-on-track design relies on.
    let body = UpdateSeriesTrackingRequest {
        tracked: Some(true),
        latest_known_chapter: Some(Some(100.0)),
        ..Default::default()
    };
    let req = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/tracking", series_id),
        &body,
        &token,
    );
    let (status, dto): (StatusCode, Option<SeriesTrackingDto>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let dto = dto.unwrap();
    assert!(dto.tracked);
    assert_eq!(
        dto.latest_known_chapter,
        Some(100.0),
        "explicit user override must beat the seeded value"
    );
}

#[tokio::test]
async fn patch_tracking_does_not_re_seed_on_already_tracked_update() {
    use codex::db::repositories::SeriesAliasRepository;

    let (db, _temp) = setup_test_db().await;
    let (lib_id, series_id) = create_test_series(&db).await;
    add_classified_book(&db, series_id, lib_id, "/v1.cbz", Some(1), None).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Initial flip -> seeds.
    let app = create_test_router(state.clone()).await;
    let body = UpdateSeriesTrackingRequest {
        tracked: Some(true),
        ..Default::default()
    };
    let req = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/tracking", series_id),
        &body,
        &token,
    );
    let (s1, _): (StatusCode, Option<SeriesTrackingDto>) = make_json_request(app, req).await;
    assert_eq!(s1, StatusCode::OK);

    // User deletes the metadata-seeded alias.
    let aliases = SeriesAliasRepository::get_for_series(&db, series_id)
        .await
        .unwrap();
    let seeded = aliases
        .iter()
        .find(|a| a.alias == "Test Series")
        .expect("first PATCH should have seeded the series name");
    SeriesAliasRepository::delete(&db, seeded.id).await.unwrap();

    // A subsequent PATCH that doesn't flip tracked false→true must NOT
    // re-run the seed. If it did, the deleted alias would come back.
    let app = create_test_router(state).await;
    let body = UpdateSeriesTrackingRequest {
        latest_known_chapter: Some(Some(5.0)),
        ..Default::default()
    };
    let req = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/tracking", series_id),
        &body,
        &token,
    );
    let (s2, _): (StatusCode, Option<SeriesTrackingDto>) = make_json_request(app, req).await;
    assert_eq!(s2, StatusCode::OK);

    let after = SeriesAliasRepository::get_for_series(&db, series_id)
        .await
        .unwrap();
    assert!(
        !after.iter().any(|a| a.alias == "Test Series"),
        "seed must not re-run when tracked is not flipping false→true"
    );
}

#[tokio::test]
async fn patch_tracking_skips_seed_when_already_tracked_and_re_setting_true() {
    use codex::db::repositories::{
        SeriesAliasRepository, SeriesTrackingRepository, TrackingUpdate,
    };

    let (db, _temp) = setup_test_db().await;
    let (lib_id, series_id) = create_test_series(&db).await;
    add_classified_book(&db, series_id, lib_id, "/v1.cbz", Some(1), None).await;

    // Pre-set tracked=true directly so the PATCH below sees was_tracked=true.
    SeriesTrackingRepository::upsert(
        &db,
        series_id,
        TrackingUpdate {
            tracked: Some(true),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    // Confirm no aliases seeded yet (we bypassed the handler).
    let before = SeriesAliasRepository::get_for_series(&db, series_id)
        .await
        .unwrap();
    assert!(before.is_empty());

    // Now PATCH with tracked=true again. Since was_tracked is already true,
    // the false→true gate should NOT trigger and seed must not run.
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;
    let body = UpdateSeriesTrackingRequest {
        tracked: Some(true),
        ..Default::default()
    };
    let req = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/tracking", series_id),
        &body,
        &token,
    );
    let (status, _): (StatusCode, Option<SeriesTrackingDto>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);

    let after = SeriesAliasRepository::get_for_series(&db, series_id)
        .await
        .unwrap();
    assert!(
        after.is_empty(),
        "seed must not run when tracked is already true; got {:?}",
        after.iter().map(|a| &a.alias).collect::<Vec<_>>()
    );
}
