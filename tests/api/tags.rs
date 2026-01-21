//! Integration tests for tag endpoints

#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::series::{
    AddSeriesTagRequest, SetSeriesTagsRequest, TagDto, TagListResponse, TaxonomyCleanupResponse,
};
use codex::api::error::ErrorResponse;
use codex::db::repositories::{LibraryRepository, SeriesRepository, TagRepository, UserRepository};
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
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

// ============================================================================
// List Tags Tests
// ============================================================================

#[tokio::test]
async fn test_list_tags_empty() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/tags", &token);
    let (status, response): (StatusCode, Option<TagListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let tag_response = response.unwrap();
    assert_eq!(tag_response.tags.len(), 0);
}

#[tokio::test]
async fn test_list_tags_with_data() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create some tags
    TagRepository::create(&db, "Completed").await.unwrap();
    TagRepository::create(&db, "Favorite").await.unwrap();
    TagRepository::create(&db, "Reading").await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/tags", &token);
    let (status, response): (StatusCode, Option<TagListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let tag_response = response.unwrap();
    assert_eq!(tag_response.tags.len(), 3);

    // Verify sorted by name
    let names: Vec<&str> = tag_response.tags.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["Completed", "Favorite", "Reading"]);
}

#[tokio::test]
async fn test_list_tags_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // No auth token
    let request = get_request("/api/v1/tags");
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Series Tags Tests
// ============================================================================

#[tokio::test]
async fn test_get_series_tags_empty() {
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

    let request = get_request_with_auth(&format!("/api/v1/series/{}/tags", series.id), &token);
    let (status, response): (StatusCode, Option<TagListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let tag_response = response.unwrap();
    assert_eq!(tag_response.tags.len(), 0);
}

#[tokio::test]
async fn test_set_series_tags() {
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

    // Set tags
    let body = SetSeriesTagsRequest {
        tags: vec!["Completed".to_string(), "Favorite".to_string()],
    };
    let request =
        put_json_request_with_auth(&format!("/api/v1/series/{}/tags", series.id), &body, &token);
    let (status, response): (StatusCode, Option<TagListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let tag_response = response.unwrap();
    assert_eq!(tag_response.tags.len(), 2);

    let names: Vec<&str> = tag_response.tags.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"Completed"));
    assert!(names.contains(&"Favorite"));
}

#[tokio::test]
async fn test_set_series_tags_replaces_existing() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Set initial tags
    TagRepository::set_tags_for_series(
        &db,
        series.id,
        vec!["Completed".to_string(), "Favorite".to_string()],
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Replace with new tags
    let body = SetSeriesTagsRequest {
        tags: vec!["Reading".to_string()],
    };
    let request =
        put_json_request_with_auth(&format!("/api/v1/series/{}/tags", series.id), &body, &token);
    let (status, response): (StatusCode, Option<TagListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let tag_response = response.unwrap();
    assert_eq!(tag_response.tags.len(), 1);
    assert_eq!(tag_response.tags[0].name, "Reading");
}

#[tokio::test]
async fn test_set_series_tags_clear() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Set initial tags
    TagRepository::set_tags_for_series(&db, series.id, vec!["Completed".to_string()])
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Clear tags by setting empty list
    let body = SetSeriesTagsRequest { tags: vec![] };
    let request =
        put_json_request_with_auth(&format!("/api/v1/series/{}/tags", series.id), &body, &token);
    let (status, response): (StatusCode, Option<TagListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let tag_response = response.unwrap();
    assert_eq!(tag_response.tags.len(), 0);
}

#[tokio::test]
async fn test_get_series_tags_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(&format!("/api/v1/series/{}/tags", fake_id), &token);
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Add/Remove Single Tag Tests
// ============================================================================

#[tokio::test]
async fn test_add_single_tag_to_series() {
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

    // Add a single tag
    let body = AddSeriesTagRequest {
        name: "Favorite".to_string(),
    };
    let request =
        post_json_request_with_auth(&format!("/api/v1/series/{}/tags", series.id), &body, &token);
    let (status, response): (StatusCode, Option<TagDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let tag = response.unwrap();
    assert_eq!(tag.name, "Favorite");

    // Verify it was added
    let tags = TagRepository::get_tags_for_series(&db, series.id)
        .await
        .unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].name, "Favorite");
}

#[tokio::test]
async fn test_add_tag_to_series_idempotent() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Pre-add the tag
    TagRepository::add_tag_to_series(&db, series.id, "Favorite")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Add the same tag again
    let body = AddSeriesTagRequest {
        name: "Favorite".to_string(),
    };
    let request =
        post_json_request_with_auth(&format!("/api/v1/series/{}/tags", series.id), &body, &token);
    let (status, _response): (StatusCode, Option<TagDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);

    // Should still only have one tag
    let tags = TagRepository::get_tags_for_series(&db, series.id)
        .await
        .unwrap();
    assert_eq!(tags.len(), 1);
}

#[tokio::test]
async fn test_remove_tag_from_series() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Add tags
    let tag = TagRepository::add_tag_to_series(&db, series.id, "Favorite")
        .await
        .unwrap();
    TagRepository::add_tag_to_series(&db, series.id, "Completed")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Remove one tag
    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/tags/{}", series.id, tag.id),
        &token,
    );
    let (status, _): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify only Completed remains
    let tags = TagRepository::get_tags_for_series(&db, series.id)
        .await
        .unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].name, "Completed");
}

#[tokio::test]
async fn test_remove_tag_from_series_not_linked() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create a tag but don't link it
    let tag = TagRepository::create(&db, "NotLinked").await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Try to remove a tag that's not linked
    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/tags/{}", series.id, tag.id),
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Delete Tag Tests (Admin)
// ============================================================================

#[tokio::test]
async fn test_delete_tag_admin() {
    let (db, _temp_dir) = setup_test_db().await;

    let tag = TagRepository::create(&db, "ToDelete").await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = delete_request_with_auth(&format!("/api/v1/tags/{}", tag.id), &token);
    let (status, _): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify deleted
    let fetched = TagRepository::get_by_id(&db, tag.id).await.unwrap();
    assert!(fetched.is_none());
}

#[tokio::test]
async fn test_delete_tag_non_admin_forbidden() {
    let (db, _temp_dir) = setup_test_db().await;

    let tag = TagRepository::create(&db, "ToDelete").await.unwrap();

    // Create non-admin user
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user("user", "user@example.com", &password_hash, false);
    let created = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    let app = create_test_router(state).await;

    let request = delete_request_with_auth(&format!("/api/v1/tags/{}", tag.id), &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);

    // Verify NOT deleted
    let fetched = TagRepository::get_by_id(&db, tag.id).await.unwrap();
    assert!(fetched.is_some());
}

#[tokio::test]
async fn test_delete_tag_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = delete_request_with_auth(&format!("/api/v1/tags/{}", fake_id), &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Cleanup Tags Tests (Admin)
// ============================================================================

#[tokio::test]
async fn test_cleanup_tags_admin() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create tags - one used, two unused
    TagRepository::add_tag_to_series(&db, series.id, "UsedTag")
        .await
        .unwrap();
    TagRepository::create(&db, "UnusedTag1").await.unwrap();
    TagRepository::create(&db, "UnusedTag2").await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = post_request_with_auth("/api/v1/tags/cleanup", &token);
    let (status, response): (StatusCode, Option<TaxonomyCleanupResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let cleanup = response.unwrap();
    assert_eq!(cleanup.deleted_count, 2);
    assert!(cleanup.deleted_names.contains(&"UnusedTag1".to_string()));
    assert!(cleanup.deleted_names.contains(&"UnusedTag2".to_string()));

    // Verify only UsedTag remains
    let remaining = TagRepository::list_all(&db).await.unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].name, "UsedTag");
}

#[tokio::test]
async fn test_cleanup_tags_non_admin_forbidden() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create non-admin user
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user("user", "user@example.com", &password_hash, false);
    let created = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    let app = create_test_router(state).await;

    let request = post_request_with_auth("/api/v1/tags/cleanup", &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_cleanup_tags_empty() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = post_request_with_auth("/api/v1/tags/cleanup", &token);
    let (status, response): (StatusCode, Option<TaxonomyCleanupResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let cleanup = response.unwrap();
    assert_eq!(cleanup.deleted_count, 0);
    assert!(cleanup.deleted_names.is_empty());
}
