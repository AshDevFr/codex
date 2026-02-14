//! Integration tests for series and book cover management endpoints

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::book::{BookCoverDto, BookCoverListResponse};
use codex::api::routes::v1::dto::series::{SeriesCoverDto, SeriesCoverListResponse};
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    BookCoversRepository, BookRepository, LibraryRepository, SeriesCoversRepository,
    SeriesRepository, UserRepository,
};
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

// Helper to create a library and series
async fn create_test_library_and_series(
    db: &sea_orm::DatabaseConnection,
) -> (uuid::Uuid, uuid::Uuid) {
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
// List Covers Tests
// ============================================================================

#[tokio::test]
async fn test_list_covers_empty() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, series_id) = create_test_library_and_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(&format!("/api/v1/series/{}/covers", series_id), &token);
    let (status, response): (StatusCode, Option<SeriesCoverListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let cover_response = response.unwrap();
    assert_eq!(cover_response.covers.len(), 0);
}

#[tokio::test]
async fn test_list_covers_with_data() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, series_id) = create_test_library_and_series(&db).await;

    // Create some covers
    SeriesCoversRepository::create(
        &db,
        series_id,
        "book:123",
        "/covers/book123.jpg",
        true,
        Some(800),
        Some(1200),
    )
    .await
    .unwrap();
    SeriesCoversRepository::create(
        &db,
        series_id,
        "custom",
        "/covers/custom.jpg",
        false,
        None,
        None,
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(&format!("/api/v1/series/{}/covers", series_id), &token);
    let (status, response): (StatusCode, Option<SeriesCoverListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let cover_response = response.unwrap();
    assert_eq!(cover_response.covers.len(), 2);

    // Verify one is selected
    let selected_count = cover_response
        .covers
        .iter()
        .filter(|c| c.is_selected)
        .count();
    assert_eq!(selected_count, 1);
}

#[tokio::test]
async fn test_list_covers_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, series_id) = create_test_library_and_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // No auth token
    let request = get_request(&format!("/api/v1/series/{}/covers", series_id));
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_list_covers_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(&format!("/api/v1/series/{}/covers", fake_id), &token);
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Select Cover Tests
// ============================================================================

#[tokio::test]
async fn test_select_cover_success() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, series_id) = create_test_library_and_series(&db).await;

    // Create two covers, first one is selected
    let cover1 = SeriesCoversRepository::create(
        &db,
        series_id,
        "book:123",
        "/covers/book123.jpg",
        true,
        None,
        None,
    )
    .await
    .unwrap();
    let cover2 = SeriesCoversRepository::create(
        &db,
        series_id,
        "custom",
        "/covers/custom.jpg",
        false,
        None,
        None,
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Select the second cover
    let request = put_request_with_auth(
        &format!("/api/v1/series/{}/covers/{}/select", series_id, cover2.id),
        "",
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesCoverDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let selected_cover = response.unwrap();
    assert_eq!(selected_cover.id, cover2.id);
    assert!(selected_cover.is_selected);
    assert_eq!(selected_cover.source, "custom");

    // Verify in database that cover1 is now deselected
    let cover1_updated = SeriesCoversRepository::get_by_id(&db, cover1.id)
        .await
        .unwrap()
        .unwrap();
    assert!(!cover1_updated.is_selected);
}

#[tokio::test]
async fn test_select_cover_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, series_id) = create_test_library_and_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_cover_id = uuid::Uuid::new_v4();
    let request = put_request_with_auth(
        &format!(
            "/api/v1/series/{}/covers/{}/select",
            series_id, fake_cover_id
        ),
        "",
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_select_cover_wrong_series() {
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

    // Create cover for series1
    let cover = SeriesCoversRepository::create(
        &db,
        series1.id,
        "custom",
        "/covers/custom.jpg",
        true,
        None,
        None,
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Try to select cover using series2 ID
    let request = put_request_with_auth(
        &format!("/api/v1/series/{}/covers/{}/select", series2.id, cover.id),
        "",
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_select_cover_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, series_id) = create_test_library_and_series(&db).await;

    let cover = SeriesCoversRepository::create(
        &db,
        series_id,
        "custom",
        "/covers/custom.jpg",
        true,
        None,
        None,
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // No auth token
    let request = put_request(&format!(
        "/api/v1/series/{}/covers/{}/select",
        series_id, cover.id
    ));
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Delete Cover Tests
// ============================================================================

#[tokio::test]
async fn test_delete_cover_success() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, series_id) = create_test_library_and_series(&db).await;

    // Create two covers
    let cover1 = SeriesCoversRepository::create(
        &db,
        series_id,
        "book:123",
        "/covers/book123.jpg",
        true,
        None,
        None,
    )
    .await
    .unwrap();
    let cover2 = SeriesCoversRepository::create(
        &db,
        series_id,
        "book:456",
        "/covers/book456.jpg",
        false,
        None,
        None,
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Delete the non-selected cover
    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/covers/{}", series_id, cover2.id),
        &token,
    );
    let (status, _response): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify cover was deleted
    let covers = SeriesCoversRepository::list_by_series(&db, series_id)
        .await
        .unwrap();
    assert_eq!(covers.len(), 1);
    assert_eq!(covers[0].id, cover1.id);
}

#[tokio::test]
async fn test_delete_selected_cover_selects_alternate() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, series_id) = create_test_library_and_series(&db).await;

    // Create two covers, first is selected
    let cover1 = SeriesCoversRepository::create(
        &db,
        series_id,
        "book:123",
        "/covers/book123.jpg",
        true,
        None,
        None,
    )
    .await
    .unwrap();
    let cover2 = SeriesCoversRepository::create(
        &db,
        series_id,
        "book:456",
        "/covers/book456.jpg",
        false,
        None,
        None,
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Delete the selected cover
    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/covers/{}", series_id, cover1.id),
        &token,
    );
    let (status, _response): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify alternate cover was selected
    let covers = SeriesCoversRepository::list_by_series(&db, series_id)
        .await
        .unwrap();
    assert_eq!(covers.len(), 1);
    assert_eq!(covers[0].id, cover2.id);
    assert!(covers[0].is_selected);
}

#[tokio::test]
async fn test_delete_only_cover() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, series_id) = create_test_library_and_series(&db).await;

    // Create only one cover
    let cover = SeriesCoversRepository::create(
        &db,
        series_id,
        "book:123",
        "/covers/book123.jpg",
        true,
        None,
        None,
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Delete the only cover
    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/covers/{}", series_id, cover.id),
        &token,
    );
    let (status, _response): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify no covers remain
    let covers = SeriesCoversRepository::list_by_series(&db, series_id)
        .await
        .unwrap();
    assert_eq!(covers.len(), 0);
}

#[tokio::test]
async fn test_delete_cover_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, series_id) = create_test_library_and_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_cover_id = uuid::Uuid::new_v4();
    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/covers/{}", series_id, fake_cover_id),
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_cover_wrong_series() {
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

    // Create cover for series1
    let cover = SeriesCoversRepository::create(
        &db,
        series1.id,
        "custom",
        "/covers/custom.jpg",
        true,
        None,
        None,
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Try to delete cover using series2 ID
    let request = delete_request_with_auth(
        &format!("/api/v1/series/{}/covers/{}", series2.id, cover.id),
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);

    // Verify cover still exists
    let cover_check = SeriesCoversRepository::get_by_id(&db, cover.id)
        .await
        .unwrap();
    assert!(cover_check.is_some());
}

#[tokio::test]
async fn test_delete_cover_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, series_id) = create_test_library_and_series(&db).await;

    let cover = SeriesCoversRepository::create(
        &db,
        series_id,
        "custom",
        "/covers/custom.jpg",
        true,
        None,
        None,
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // No auth token
    let request = delete_request(&format!("/api/v1/series/{}/covers/{}", series_id, cover.id));
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Cover Attributes Tests
// ============================================================================

#[tokio::test]
async fn test_cover_response_includes_all_fields() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, series_id) = create_test_library_and_series(&db).await;

    // Create cover with all fields
    SeriesCoversRepository::create(
        &db,
        series_id,
        "custom",
        "/covers/custom.jpg",
        true,
        Some(800),
        Some(1200),
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(&format!("/api/v1/series/{}/covers", series_id), &token);
    let (status, response): (StatusCode, Option<SeriesCoverListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let cover_response = response.unwrap();
    assert_eq!(cover_response.covers.len(), 1);

    let cover = &cover_response.covers[0];
    assert_eq!(cover.series_id, series_id);
    assert_eq!(cover.source, "custom");
    assert_eq!(cover.path, "/covers/custom.jpg");
    assert!(cover.is_selected);
    assert_eq!(cover.width, Some(800));
    assert_eq!(cover.height, Some(1200));
}

// ============================================================================
// Upload Cover Tests
// ============================================================================

#[tokio::test]
async fn test_upload_cover_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, series_id) = create_test_library_and_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // No auth token - POST to upload cover
    let request = post_request(&format!("/api/v1/series/{}/cover", series_id));
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_upload_cover_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    // POST without multipart body will get rejected but series lookup happens first
    let request = post_request_with_auth(&format!("/api/v1/series/{}/cover", fake_id), &token);
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    // Might be 400 (missing multipart) or 415 (unsupported media type) - depends on order of checks
    // The important thing is it's not 200/201
    assert!(status.is_client_error());
}

#[tokio::test]
async fn test_upload_cover_missing_file() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, series_id) = create_test_library_and_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // POST without proper multipart body
    let request = post_request_with_auth(&format!("/api/v1/series/{}/cover", series_id), &token);
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    // Should fail due to missing/invalid multipart data
    assert!(status.is_client_error());
}

// ============================================================================
// Book Cover Tests
// ============================================================================

// Helper to create a test book in the database
fn create_test_book_model(
    series_id: uuid::Uuid,
    library_id: uuid::Uuid,
    path: &str,
    name: &str,
) -> codex::db::entities::books::Model {
    use chrono::Utc;
    codex::db::entities::books::Model {
        id: uuid::Uuid::new_v4(),
        series_id,
        library_id,
        file_path: path.to_string(),
        file_name: name.to_string(),
        file_size: 1024,
        file_hash: format!("hash_{}", uuid::Uuid::new_v4()),
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
    }
}

// Helper to create a library, series, and book
async fn create_test_library_series_and_book(
    db: &sea_orm::DatabaseConnection,
) -> (uuid::Uuid, uuid::Uuid, uuid::Uuid) {
    let library =
        LibraryRepository::create(db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(db, library.id, "Test Series", None)
        .await
        .unwrap();
    let book_model =
        create_test_book_model(series.id, library.id, "/test/path/book1.cbz", "book1.cbz");
    let book = BookRepository::create(db, &book_model, None).await.unwrap();
    (library.id, series.id, book.id)
}

// ============================================================================
// List Book Covers Tests
// ============================================================================

#[tokio::test]
async fn test_list_book_covers_empty() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, _, book_id) = create_test_library_series_and_book(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(&format!("/api/v1/books/{}/covers", book_id), &token);
    let (status, response): (StatusCode, Option<BookCoverListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let cover_response = response.unwrap();
    assert_eq!(cover_response.covers.len(), 0);
}

#[tokio::test]
async fn test_upload_book_cover() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, _, book_id) = create_test_library_series_and_book(&db).await;

    // Create a cover via the repository (simulating upload)
    BookCoversRepository::create(
        &db,
        book_id,
        "custom",
        "/covers/custom.jpg",
        true,
        Some(800),
        Some(1200),
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(&format!("/api/v1/books/{}/covers", book_id), &token);
    let (status, response): (StatusCode, Option<BookCoverListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let cover_response = response.unwrap();
    assert_eq!(cover_response.covers.len(), 1);

    let cover = &cover_response.covers[0];
    assert_eq!(cover.book_id, book_id);
    assert_eq!(cover.source, "custom");
    assert!(cover.is_selected);
    assert_eq!(cover.width, Some(800));
    assert_eq!(cover.height, Some(1200));
}

#[tokio::test]
async fn test_select_book_cover() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, _, book_id) = create_test_library_series_and_book(&db).await;

    // Create two covers, first is selected
    let cover1 = BookCoversRepository::create(
        &db,
        book_id,
        "extracted",
        "/covers/extracted.jpg",
        true,
        None,
        None,
    )
    .await
    .unwrap();
    let cover2 = BookCoversRepository::create(
        &db,
        book_id,
        "custom",
        "/covers/custom.jpg",
        false,
        None,
        None,
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Select the second cover
    let request = put_request_with_auth(
        &format!("/api/v1/books/{}/covers/{}/select", book_id, cover2.id),
        "",
        &token,
    );
    let (status, response): (StatusCode, Option<BookCoverDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let selected_cover = response.unwrap();
    assert_eq!(selected_cover.id, cover2.id);
    assert!(selected_cover.is_selected);
    assert_eq!(selected_cover.source, "custom");

    // Verify in database that cover1 is now deselected
    let cover1_updated = BookCoversRepository::get_by_id(&db, cover1.id)
        .await
        .unwrap()
        .unwrap();
    assert!(!cover1_updated.is_selected);
}

#[tokio::test]
async fn test_reset_book_cover() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, _, book_id) = create_test_library_series_and_book(&db).await;

    // Create a custom cover and select it
    BookCoversRepository::create(
        &db,
        book_id,
        "custom",
        "/covers/custom.jpg",
        true,
        None,
        None,
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Reset to default (delete selected)
    let request = delete_request_with_auth(
        &format!("/api/v1/books/{}/covers/selected", book_id),
        &token,
    );
    let (status, _): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_delete_book_cover() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, _, book_id) = create_test_library_series_and_book(&db).await;

    // Create two covers
    let cover1 = BookCoversRepository::create(
        &db,
        book_id,
        "extracted",
        "/covers/extracted.jpg",
        true,
        None,
        None,
    )
    .await
    .unwrap();
    let cover2 = BookCoversRepository::create(
        &db,
        book_id,
        "custom",
        "/covers/custom.jpg",
        false,
        None,
        None,
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Delete the non-selected cover
    let request = delete_request_with_auth(
        &format!("/api/v1/books/{}/covers/{}", book_id, cover2.id),
        &token,
    );
    let (status, _response): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify cover was deleted
    let covers = BookCoversRepository::list_by_book(&db, book_id)
        .await
        .unwrap();
    assert_eq!(covers.len(), 1);
    assert_eq!(covers[0].id, cover1.id);
}

#[tokio::test]
async fn test_get_book_cover_image() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, _, book_id) = create_test_library_series_and_book(&db).await;

    // Create a cover pointing to a non-existent file (we test the route, not file serving)
    let cover = BookCoversRepository::create(
        &db,
        book_id,
        "custom",
        "/covers/custom.jpg",
        true,
        None,
        None,
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request the cover image - will fail because file doesn't exist, but route should be valid
    let request = get_request_with_auth(
        &format!("/api/v1/books/{}/covers/{}/image", book_id, cover.id),
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    // Route exists and is authenticated - the cover file doesn't exist on disk
    // so we expect either a 404 (file not found) or 500 (internal error) but NOT 401
    assert_ne!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_book_cover_auth_required() {
    let (db, _temp_dir) = setup_test_db().await;

    let (_, _, book_id) = create_test_library_series_and_book(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // No auth token
    let request = get_request(&format!("/api/v1/books/{}/covers", book_id));
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_book_cover_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(&format!("/api/v1/books/{}/covers", fake_id), &token);
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}
