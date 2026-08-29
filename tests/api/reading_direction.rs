//! Integration tests for reading direction: validation on write, and the
//! layered resolution the reader depends on.

#[path = "../common/mod.rs"]
mod common;

use codex::db::repositories::{LibraryRepository, SeriesMetadataRepository, UserRepository};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use serde_json::json;

async fn admin_token(
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
// Validation on write
// ============================================================================

#[tokio::test]
async fn patch_series_metadata_rejects_an_invalid_reading_direction() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = admin_token(&db, &state).await;
    let app = create_test_router(state).await;

    let library = create_test_library(&db, "Test Library", "/test/path").await;
    let series = create_test_series(&db, &library, "Berserk").await;

    let request = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata", series.id),
        &json!({ "readingDirection": "sideways" }),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);

    // The rejected write must not have landed.
    let metadata = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(metadata.reading_direction, None);
}

#[tokio::test]
async fn patch_series_metadata_rejects_the_komga_vocabulary() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = admin_token(&db, &state).await;
    let app = create_test_router(state).await;

    let library = create_test_library(&db, "Test Library", "/test/path").await;
    let series = create_test_series(&db, &library, "Berserk").await;

    // Komga's wire vocabulary is not what this column stores. Accepting it
    // silently is how the two vocabularies diverged in the first place.
    let request = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata", series.id),
        &json!({ "readingDirection": "RIGHT_TO_LEFT" }),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn patch_series_metadata_canonicalises_case() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = admin_token(&db, &state).await;
    let app = create_test_router(state).await;

    let library = create_test_library(&db, "Test Library", "/test/path").await;
    let series = create_test_series(&db, &library, "Berserk").await;

    let request = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata", series.id),
        &json!({ "readingDirection": "RTL" }),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let metadata = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(metadata.reading_direction, Some("rtl".to_string()));
    assert!(metadata.reading_direction_lock);
}

#[tokio::test]
async fn replace_series_metadata_rejects_an_invalid_reading_direction() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = admin_token(&db, &state).await;
    let app = create_test_router(state).await;

    let library = create_test_library(&db, "Test Library", "/test/path").await;
    let series = create_test_series(&db, &library, "Berserk").await;

    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata", series.id),
        &json!({ "title": "Berserk", "readingDirection": "sideways" }),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn creating_a_library_rejects_an_invalid_reading_direction() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = admin_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = post_json_request_with_auth(
        "/api/v1/libraries",
        &json!({
            "name": "Manga",
            "path": "/manga",
            "defaultReadingDirection": "sideways",
        }),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn updating_a_library_rejects_an_invalid_reading_direction() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = admin_token(&db, &state).await;
    let app = create_test_router(state).await;

    let library = create_test_library(&db, "Test Library", "/test/path").await;

    let request = patch_json_request_with_auth(
        &format!("/api/v1/libraries/{}", library.id),
        &json!({ "defaultReadingDirection": "LEFT_TO_RIGHT" }),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);

    let unchanged = LibraryRepository::get_by_id(&db, library.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.default_reading_direction, "ltr");
}

// ============================================================================
// Resolution
// ============================================================================

#[tokio::test]
async fn a_new_library_defaults_to_the_canonical_lowercase_value() {
    let (db, _temp_dir) = setup_test_db().await;

    // The repository default and the web form used to disagree, leaving rows
    // the reader could not parse.
    let library = create_test_library(&db, "Test Library", "/test/path").await;
    assert_eq!(library.default_reading_direction, "ltr");
}

#[tokio::test]
async fn book_reading_direction_prefers_series_over_library_default() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = admin_token(&db, &state).await;
    let app = create_test_router(state).await;

    let mut library = create_test_library(&db, "Test Library", "/test/path").await;
    library.default_reading_direction = "ltr".to_string();
    LibraryRepository::update(&db, &library).await.unwrap();

    let series = create_test_series(&db, &library, "Berserk").await;
    let book = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Vol 1",
        "/test/path/v1.cbz",
        "hash-1",
    )
    .await;

    SeriesMetadataRepository::update_reading_direction(&db, series.id, Some("rtl".to_string()))
        .await
        .unwrap();

    let request = get_request_with_auth(&format!("/api/v1/books/{}", book.id), &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.unwrap()["book"]["readingDirection"], "rtl");
}

#[tokio::test]
async fn an_unparseable_stored_direction_falls_through_to_the_library_default() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = admin_token(&db, &state).await;
    let app = create_test_router(state).await;

    let mut library = create_test_library(&db, "Test Library", "/test/path").await;
    library.default_reading_direction = "rtl".to_string();
    LibraryRepository::update(&db, &library).await.unwrap();

    let series = create_test_series(&db, &library, "Berserk").await;
    let book = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Vol 1",
        "/test/path/v1.cbz",
        "hash-1",
    )
    .await;

    // Rows written before validation existed can hold anything. Writing through
    // the repository bypasses the API boundary, which is exactly how such a row
    // came to exist.
    SeriesMetadataRepository::update_reading_direction(
        &db,
        series.id,
        Some("sideways".to_string()),
    )
    .await
    .unwrap();

    let request = get_request_with_auth(&format!("/api/v1/books/{}", book.id), &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    // Junk must not reach the reader, and must not mask the library default.
    assert_eq!(response.unwrap()["book"]["readingDirection"], "rtl");
}

#[tokio::test]
async fn a_series_without_a_direction_inherits_the_library_default() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = admin_token(&db, &state).await;
    let app = create_test_router(state).await;

    let mut library = create_test_library(&db, "Test Library", "/test/path").await;
    library.default_reading_direction = "webtoon".to_string();
    LibraryRepository::update(&db, &library).await.unwrap();

    let series = create_test_series(&db, &library, "Solo Leveling").await;
    let book = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Ch 1",
        "/test/path/c1.cbz",
        "hash-2",
    )
    .await;

    let request = get_request_with_auth(&format!("/api/v1/books/{}", book.id), &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.unwrap()["book"]["readingDirection"], "webtoon");
}

// ============================================================================
// Per-user, per-series reader settings
// ============================================================================

async fn user_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
    username: &str,
    is_admin: bool,
) -> (uuid::Uuid, String) {
    let password_hash = password::hash_password("pw").unwrap();
    let user = create_test_user(
        username,
        &format!("{}@example.com", username),
        &password_hash,
        is_admin,
    );
    let created = UserRepository::create(db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    (created.id, token)
}

#[tokio::test]
async fn a_reader_can_set_its_own_series_reader_settings() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state).await;

    let library = create_test_library(&db, "Test Library", "/test/path").await;
    let series = create_test_series(&db, &library, "Berserk").await;

    // The whole point: a reader role, which has no series:write, can still
    // correct a series for itself.
    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &json!({ "readingDirection": "rtl" }),
        &token,
    );
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.unwrap()["readingDirection"], "rtl");
}

#[tokio::test]
async fn a_reader_still_cannot_change_the_series_metadata() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state).await;

    let library = create_test_library(&db, "Test Library", "/test/path").await;
    let series = create_test_series(&db, &library, "Berserk").await;

    // Personal settings are not a back door into what everyone sees.
    let request = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata", series.id),
        &json!({ "readingDirection": "rtl" }),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn reader_settings_start_empty_rather_than_missing() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state).await;

    let library = create_test_library(&db, "Test Library", "/test/path").await;
    let series = create_test_series(&db, &library, "Berserk").await;

    let request = get_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &token,
    );
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    // A user with no overrides gets a 200 describing what it inherits, never a
    // 404 and never a value it did not set.
    let body = response.unwrap();
    assert!(body.get("readingDirection").is_none());
    assert_eq!(body["inheritedReadingDirectionSource"], "library");
}

#[tokio::test]
async fn a_null_clears_the_override_and_restores_inheritance() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state.clone()).await;

    let library = create_test_library(&db, "Test Library", "/test/path").await;
    let series = create_test_series(&db, &library, "Berserk").await;

    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &json!({ "readingDirection": "rtl" }),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let app = create_test_router(state).await;
    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &json!({ "readingDirection": null }),
        &token,
    );
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    // Back to inheriting, and the record is gone rather than left as an empty
    // husk claiming this user has overrides here.
    let body = response.unwrap();
    assert!(body.get("readingDirection").is_none());
    assert_eq!(body["inheritedReadingDirection"], "ltr");
}

#[tokio::test]
async fn a_patch_that_names_no_setting_changes_nothing() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state.clone()).await;

    let library = create_test_library(&db, "Test Library", "/test/path").await;
    let series = create_test_series(&db, &library, "Berserk").await;

    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &json!({ "readingDirection": "rtl" }),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // An absent key means "leave it alone", not "clear it".
    let app = create_test_router(state).await;
    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &json!({}),
        &token,
    );
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.unwrap()["readingDirection"], "rtl");
}

#[tokio::test]
async fn deleting_reader_settings_restores_full_inheritance() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state.clone()).await;

    let mut library = create_test_library(&db, "Test Library", "/test/path").await;
    library.default_reading_direction = "ltr".to_string();
    LibraryRepository::update(&db, &library).await.unwrap();
    let series = create_test_series(&db, &library, "Berserk").await;

    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &json!({ "readingDirection": "rtl" }),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let app = create_test_router(state.clone()).await;
    let request = delete_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &token,
    );
    let (status, _) = make_request(app, request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let app = create_test_router(state).await;
    let request = get_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &token,
    );
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert!(body.get("readingDirection").is_none());
    assert_eq!(body["inheritedReadingDirection"], "ltr");
}

#[tokio::test]
async fn deleting_settings_that_were_never_set_is_not_an_error() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state).await;

    let library = create_test_library(&db, "Test Library", "/test/path").await;
    let series = create_test_series(&db, &library, "Berserk").await;

    let request = delete_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &token,
    );
    let (status, _) = make_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn reader_settings_reject_an_invalid_value() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state).await;

    let library = create_test_library(&db, "Test Library", "/test/path").await;
    let series = create_test_series(&db, &library, "Berserk").await;

    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &json!({ "readingDirection": "sideways" }),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    // A 400 naming the valid values, matching the series metadata endpoints,
    // rather than serde's opaque 422.
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn reader_settings_for_an_unknown_series_are_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(
        &format!(
            "/api/v1/user/series/{}/reader-settings",
            uuid::Uuid::new_v4()
        ),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn reader_settings_require_authentication() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let library = create_test_library(&db, "Test Library", "/test/path").await;
    let series = create_test_series(&db, &library, "Berserk").await;

    let request = get_request(&format!(
        "/api/v1/user/series/{}/reader-settings",
        series.id
    ));
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn one_users_reader_settings_are_invisible_to_another() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_reader_id, reader_token) = user_and_token(&db, &state, "reader", false).await;
    let (_other_id, other_token) = user_and_token(&db, &state, "other", false).await;
    let app = create_test_router(state.clone()).await;

    let library = create_test_library(&db, "Test Library", "/test/path").await;
    let series = create_test_series(&db, &library, "Berserk").await;

    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &json!({ "readingDirection": "rtl" }),
        &reader_token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // The identity comes from the token, never the path, so the second user
    // sees its own empty record rather than the first user's.
    let app = create_test_router(state).await;
    let request = get_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &other_token,
    );
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert!(response.unwrap().get("readingDirection").is_none());
}

// ============================================================================
// Three-layer resolution
// ============================================================================

/// The user's own override beats the series metadata, which beats the library.
#[tokio::test]
async fn a_user_override_wins_over_the_series_metadata() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state.clone()).await;

    let mut library = create_test_library(&db, "Test Library", "/test/path").await;
    library.default_reading_direction = "ttb".to_string();
    LibraryRepository::update(&db, &library).await.unwrap();

    let series = create_test_series(&db, &library, "Berserk").await;
    let book = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Vol 1",
        "/test/path/v1.cbz",
        "hash-1",
    )
    .await;

    SeriesMetadataRepository::update_reading_direction(&db, series.id, Some("ltr".to_string()))
        .await
        .unwrap();

    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &json!({ "readingDirection": "rtl" }),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let app = create_test_router(state).await;
    let request = get_request_with_auth(&format!("/api/v1/books/{}", book.id), &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.unwrap()["book"]["readingDirection"], "rtl");
}

/// The same book, two callers, two answers. This is the whole feature.
#[tokio::test]
async fn two_users_see_different_directions_for_the_same_book() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_reader_id, reader_token) = user_and_token(&db, &state, "reader", false).await;
    let (_other_id, other_token) = user_and_token(&db, &state, "other", false).await;
    let app = create_test_router(state.clone()).await;

    let mut library = create_test_library(&db, "Test Library", "/test/path").await;
    library.default_reading_direction = "ltr".to_string();
    LibraryRepository::update(&db, &library).await.unwrap();

    let series = create_test_series(&db, &library, "Berserk").await;
    let book = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Vol 1",
        "/test/path/v1.cbz",
        "hash-1",
    )
    .await;

    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &json!({ "readingDirection": "rtl" }),
        &reader_token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(&format!("/api/v1/books/{}", book.id), &reader_token);
    let (_status, mine): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    let app = create_test_router(state).await;
    let request = get_request_with_auth(&format!("/api/v1/books/{}", book.id), &other_token);
    let (_status, theirs): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(mine.unwrap()["book"]["readingDirection"], "rtl");
    // Untouched by the other user's correction: still the library default.
    assert_eq!(theirs.unwrap()["book"]["readingDirection"], "ltr");
}

/// Clearing the override falls back through the layers again rather than
/// leaving the last value stuck.
#[tokio::test]
async fn clearing_the_override_restores_the_series_direction() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state.clone()).await;

    let library = create_test_library(&db, "Test Library", "/test/path").await;
    let series = create_test_series(&db, &library, "Berserk").await;
    let book = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Vol 1",
        "/test/path/v1.cbz",
        "hash-1",
    )
    .await;

    SeriesMetadataRepository::update_reading_direction(&db, series.id, Some("ttb".to_string()))
        .await
        .unwrap();

    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &json!({ "readingDirection": "rtl" }),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let app = create_test_router(state.clone()).await;
    let request = delete_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &token,
    );
    let (status, _) = make_request(app, request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let app = create_test_router(state).await;
    let request = get_request_with_auth(&format!("/api/v1/books/{}", book.id), &token);
    let (_status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(response.unwrap()["book"]["readingDirection"], "ttb");
}

/// The list path resolves per caller too, not just the single-book path.
#[tokio::test]
async fn the_book_list_resolves_the_user_override() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state.clone()).await;

    let mut library = create_test_library(&db, "Test Library", "/test/path").await;
    library.default_reading_direction = "ltr".to_string();
    LibraryRepository::update(&db, &library).await.unwrap();

    let series = create_test_series(&db, &library, "Berserk").await;
    create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Vol 1",
        "/test/path/v1.cbz",
        "hash-1",
    )
    .await;

    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &json!({ "readingDirection": "rtl" }),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/books", &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    let books = body["data"].as_array().expect("a data array");
    assert_eq!(books.len(), 1);
    assert_eq!(books[0]["readingDirection"], "rtl");
}

/// A user override for one series must not bleed into another.
#[tokio::test]
async fn an_override_applies_only_to_its_own_series() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state.clone()).await;

    let mut library = create_test_library(&db, "Test Library", "/test/path").await;
    library.default_reading_direction = "ltr".to_string();
    LibraryRepository::update(&db, &library).await.unwrap();

    let manga = create_test_series(&db, &library, "Berserk").await;
    let comic = create_test_series(&db, &library, "Bone").await;
    let manga_book = create_test_book_with_hash(
        &db,
        &library,
        &manga,
        "Vol 1",
        "/test/path/b1.cbz",
        "hash-1",
    )
    .await;
    let comic_book = create_test_book_with_hash(
        &db,
        &library,
        &comic,
        "Vol 1",
        "/test/path/o1.cbz",
        "hash-2",
    )
    .await;

    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", manga.id),
        &json!({ "readingDirection": "rtl" }),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(&format!("/api/v1/books/{}", manga_book.id), &token);
    let (_s, manga_response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    let app = create_test_router(state).await;
    let request = get_request_with_auth(&format!("/api/v1/books/{}", comic_book.id), &token);
    let (_s, comic_response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(manga_response.unwrap()["book"]["readingDirection"], "rtl");
    assert_eq!(comic_response.unwrap()["book"]["readingDirection"], "ltr");
}

// ============================================================================
// The inherited direction, reported so a reader can drop an override
// ============================================================================
//
// A book response carries the direction already resolved, so the layer beneath
// a user's override is invisible from a client. The settings endpoint reports
// it, which is what lets the reader UI say what resetting would fall back to.

#[tokio::test]
async fn reader_settings_report_the_direction_inherited_from_the_series() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state).await;

    let mut library = create_test_library(&db, "Test Library", "/test/path").await;
    library.default_reading_direction = "ltr".to_string();
    LibraryRepository::update(&db, &library).await.unwrap();

    let series = create_test_series(&db, &library, "Berserk").await;
    SeriesMetadataRepository::update_reading_direction(&db, series.id, Some("rtl".to_string()))
        .await
        .unwrap();

    let request = get_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &token,
    );
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    // The series metadata wins over the library default, and the caller is told
    // which layer answered so it can name it.
    assert_eq!(body["inheritedReadingDirection"], "rtl");
    assert_eq!(body["inheritedReadingDirectionSource"], "series");
    assert!(body.get("readingDirection").is_none());
}

#[tokio::test]
async fn reader_settings_fall_back_to_the_library_default_when_the_series_has_none() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state).await;

    let mut library = create_test_library(&db, "Test Library", "/test/path").await;
    library.default_reading_direction = "webtoon".to_string();
    LibraryRepository::update(&db, &library).await.unwrap();

    let series = create_test_series(&db, &library, "Tower of God").await;

    let request = get_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &token,
    );
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert_eq!(body["inheritedReadingDirection"], "webtoon");
    assert_eq!(body["inheritedReadingDirectionSource"], "library");
}

#[tokio::test]
async fn reader_settings_report_no_inherited_direction_when_no_layer_resolves() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state).await;

    let mut library = create_test_library(&db, "Test Library", "/test/path").await;
    library.default_reading_direction = String::new();
    LibraryRepository::update(&db, &library).await.unwrap();

    let series = create_test_series(&db, &library, "Bone").await;

    let request = get_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &token,
    );
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    // Nothing to inherit is reported as nothing, not as a guessed default: the
    // UI has to be able to word the reset honestly.
    assert_eq!(response.unwrap(), json!({}));
}

#[tokio::test]
async fn an_unparseable_stored_direction_is_reported_as_nothing_inherited() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state).await;

    // A library predating validation, still holding the Komga vocabulary.
    let mut library = create_test_library(&db, "Test Library", "/test/path").await;
    library.default_reading_direction = "LEFT_TO_RIGHT".to_string();
    LibraryRepository::update(&db, &library).await.unwrap();

    let series = create_test_series(&db, &library, "Berserk").await;
    SeriesMetadataRepository::update_reading_direction(
        &db,
        series.id,
        Some("sideways".to_string()),
    )
    .await
    .unwrap();

    let request = get_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &token,
    );
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    // Junk degrades the same way it does on the book path: skipped, not shown.
    assert_eq!(response.unwrap(), json!({}));
}

#[tokio::test]
async fn the_inherited_direction_is_reported_alongside_an_active_override() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state.clone()).await;

    let library = create_test_library(&db, "Test Library", "/test/path").await;
    let series = create_test_series(&db, &library, "Berserk").await;
    SeriesMetadataRepository::update_reading_direction(&db, series.id, Some("rtl".to_string()))
        .await
        .unwrap();

    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &json!({ "readingDirection": "ltr" }),
        &token,
    );
    let (status, patched): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // The response to the write carries the inherited pair too. The client
    // writes this straight into its cache, so a leaner response here would
    // blank the reset affordance until the next refetch.
    let patched = patched.unwrap();
    assert_eq!(patched["readingDirection"], "ltr");
    assert_eq!(patched["inheritedReadingDirection"], "rtl");
    assert_eq!(patched["inheritedReadingDirectionSource"], "series");

    let app = create_test_router(state).await;
    let request = get_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &token,
    );
    let (status, fetched): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    // An override does not hide the layer beneath it.
    assert_eq!(fetched.unwrap(), patched);
}

#[tokio::test]
async fn one_users_override_is_not_reported_as_anothers_inheritance() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, reader_token) = user_and_token(&db, &state, "reader", false).await;
    let (_other_id, other_token) = user_and_token(&db, &state, "other", false).await;
    let app = create_test_router(state.clone()).await;

    let mut library = create_test_library(&db, "Test Library", "/test/path").await;
    library.default_reading_direction = "ltr".to_string();
    LibraryRepository::update(&db, &library).await.unwrap();
    let series = create_test_series(&db, &library, "Berserk").await;

    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &json!({ "readingDirection": "rtl" }),
        &reader_token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let app = create_test_router(state).await;
    let request = get_request_with_auth(
        &format!("/api/v1/user/series/{}/reader-settings", series.id),
        &other_token,
    );
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    // The user layer is per-user by definition, so it can never be what another
    // user inherits.
    assert_eq!(body["inheritedReadingDirection"], "ltr");
    assert_eq!(body["inheritedReadingDirectionSource"], "library");
    assert!(body.get("readingDirection").is_none());
}
