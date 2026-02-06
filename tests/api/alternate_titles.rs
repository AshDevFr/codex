//! Integration tests for series alternate title endpoints

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::series::{
    AlternateTitleDto, AlternateTitleListResponse, CreateAlternateTitleRequest,
    UpdateAlternateTitleRequest,
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
// List Alternate Titles Tests
// ============================================================================

#[tokio::test]
async fn test_list_alternate_titles_empty() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/alternate-titles", series_id),
        &token,
    );
    let (status, response): (StatusCode, Option<AlternateTitleListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let title_response = response.unwrap();
    assert_eq!(title_response.titles.len(), 0);
}

#[tokio::test]
async fn test_list_alternate_titles_with_data() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    use codex::db::repositories::AlternateTitleRepository;
    AlternateTitleRepository::create(&db, series_id, "Japanese", "日本語タイトル")
        .await
        .unwrap();
    AlternateTitleRepository::create(&db, series_id, "Romaji", "Nihongo Taitoru")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/alternate-titles", series_id),
        &token,
    );
    let (status, response): (StatusCode, Option<AlternateTitleListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let title_response = response.unwrap();
    assert_eq!(title_response.titles.len(), 2);
}

#[tokio::test]
async fn test_list_alternate_titles_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = Uuid::new_v4();
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/alternate-titles", fake_id),
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_list_alternate_titles_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let request = get_request(&format!("/api/v1/series/{}/alternate-titles", series_id));
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Create Alternate Title Tests
// ============================================================================

#[tokio::test]
async fn test_create_alternate_title() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = CreateAlternateTitleRequest {
        label: "Japanese".to_string(),
        title: "進撃の巨人".to_string(),
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/alternate-titles", series_id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<AlternateTitleDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::CREATED);
    let title = response.unwrap();
    assert_eq!(title.label, "Japanese");
    assert_eq!(title.title, "進撃の巨人");
    assert_eq!(title.series_id, series_id);
}

#[tokio::test]
async fn test_create_alternate_title_trims_whitespace() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = CreateAlternateTitleRequest {
        label: "  Romaji  ".to_string(),
        title: "  Shingeki no Kyojin  ".to_string(),
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/alternate-titles", series_id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<AlternateTitleDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::CREATED);
    let title = response.unwrap();
    assert_eq!(title.label, "Romaji");
    assert_eq!(title.title, "Shingeki no Kyojin");
}

#[tokio::test]
async fn test_create_alternate_title_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = Uuid::new_v4();
    let body = CreateAlternateTitleRequest {
        label: "Japanese".to_string(),
        title: "Title".to_string(),
    };

    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/alternate-titles", fake_id),
        &body,
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Update Alternate Title Tests
// ============================================================================

#[tokio::test]
async fn test_update_alternate_title_label_only() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    use codex::db::repositories::AlternateTitleRepository;
    let title = AlternateTitleRepository::create(&db, series_id, "Japanese", "タイトル")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = UpdateAlternateTitleRequest {
        label: Some("Romaji".to_string()),
        title: None,
    };

    let request = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/alternate-titles/{}", series_id, title.id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<AlternateTitleDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let updated = response.unwrap();
    assert_eq!(updated.label, "Romaji");
    assert_eq!(updated.title, "タイトル"); // unchanged
}

#[tokio::test]
async fn test_update_alternate_title_title_only() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    use codex::db::repositories::AlternateTitleRepository;
    let title = AlternateTitleRepository::create(&db, series_id, "Japanese", "Old Title")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = UpdateAlternateTitleRequest {
        label: None,
        title: Some("New Title".to_string()),
    };

    let request = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/alternate-titles/{}", series_id, title.id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<AlternateTitleDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let updated = response.unwrap();
    assert_eq!(updated.label, "Japanese"); // unchanged
    assert_eq!(updated.title, "New Title");
}

#[tokio::test]
async fn test_update_alternate_title_both_fields() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    use codex::db::repositories::AlternateTitleRepository;
    let title = AlternateTitleRepository::create(&db, series_id, "Old Label", "Old Title")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = UpdateAlternateTitleRequest {
        label: Some("New Label".to_string()),
        title: Some("New Title".to_string()),
    };

    let request = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/alternate-titles/{}", series_id, title.id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<AlternateTitleDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let updated = response.unwrap();
    assert_eq!(updated.label, "New Label");
    assert_eq!(updated.title, "New Title");
}

#[tokio::test]
async fn test_update_alternate_title_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = Uuid::new_v4();
    let body = UpdateAlternateTitleRequest {
        label: Some("Label".to_string()),
        title: None,
    };

    let request = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/alternate-titles/{}", series_id, fake_id),
        &body,
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_alternate_title_wrong_series() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create two series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series1 = SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Series 2", None)
        .await
        .unwrap();

    // Create title on series1
    use codex::db::repositories::AlternateTitleRepository;
    let title = AlternateTitleRepository::create(&db, series1.id, "Japanese", "タイトル")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Try to update using series2's ID
    let body = UpdateAlternateTitleRequest {
        label: Some("Label".to_string()),
        title: None,
    };

    let request = patch_json_request_with_auth(
        &format!(
            "/api/v1/series/{}/alternate-titles/{}",
            series2.id, title.id
        ),
        &body,
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Delete Alternate Title Tests
// ============================================================================

#[tokio::test]
async fn test_delete_alternate_title() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    use codex::db::repositories::AlternateTitleRepository;
    let title = AlternateTitleRepository::create(&db, series_id, "Japanese", "タイトル")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/alternate-titles/{}", series_id, title.id),
        &token,
    );
    let (status, _response): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify it was deleted
    let remaining = AlternateTitleRepository::get_for_series(&db, series_id)
        .await
        .unwrap();
    assert!(remaining.is_empty());
}

#[tokio::test]
async fn test_delete_alternate_title_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_library_id, series_id) = create_test_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = Uuid::new_v4();
    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/alternate-titles/{}", series_id, fake_id),
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_alternate_title_wrong_series() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create two series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series1 = SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Series 2", None)
        .await
        .unwrap();

    // Create title on series1
    use codex::db::repositories::AlternateTitleRepository;
    let title = AlternateTitleRepository::create(&db, series1.id, "Japanese", "タイトル")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Try to delete using series2's ID
    let request = delete_request_with_auth(
        &format!(
            "/api/v1/series/{}/alternate-titles/{}",
            series2.id, title.id
        ),
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);

    // Verify original title still exists
    let remaining = AlternateTitleRepository::get_for_series(&db, series1.id)
        .await
        .unwrap();
    assert_eq!(remaining.len(), 1);
}
