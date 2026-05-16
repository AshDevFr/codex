//! Tests for bulk operations endpoints
//!
//! Tests bulk mark read/unread and analyze operations for books and series.

#[path = "../common/mod.rs"]
mod common;

use codex::api::routes::v1::dto::{
    BulkAnalyzeBooksRequest, BulkAnalyzeResponse, BulkAnalyzeSeriesRequest, BulkBooksRequest,
    BulkSeriesRequest, BulkTaskResponse, MarkReadResponse,
};
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    BookRepository, LibraryRepository, ReadProgressRepository, SeriesRepository, TaskRepository,
    UserRepository,
};
use codex::tasks::handlers::{BulkTrackForReleasesHandler, TaskHandler};
use codex::utils::password;
use common::*;
use hyper::StatusCode;

// Helper to create admin and token
async fn create_admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> (uuid::Uuid, String) {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    (created.id, token)
}

// Helper to create a test book model
fn create_test_book_model(
    series_id: uuid::Uuid,
    library_id: uuid::Uuid,
    path: &str,
    name: &str,
    page_count: i32,
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
        page_count,
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
    }
}

// ============================================================================
// Bulk Mark Books as Read Tests
// ============================================================================

#[tokio::test]
async fn test_bulk_mark_books_as_read() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create 3 test books
    let mut book_ids = Vec::new();
    for i in 1..=3 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            50,
        );
        let book = BookRepository::create(&db, &book, None).await.unwrap();
        book_ids.push(book.id);
    }

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Bulk mark books as read
    let request_body = BulkBooksRequest {
        book_ids: book_ids.clone(),
    };
    let request = post_json_request_with_auth("/api/v1/books/bulk/read", &request_body, &token);
    let (status, response): (StatusCode, Option<MarkReadResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let mark_response = response.unwrap();
    assert_eq!(mark_response.count, 3);
    assert!(mark_response.message.contains("3 books"));

    // Verify all books are marked as read
    for book_id in book_ids {
        let progress = ReadProgressRepository::get_by_user_and_book(&db, user_id, book_id)
            .await
            .unwrap()
            .unwrap();
        assert!(progress.completed);
        assert_eq!(progress.current_page, 50);
    }
}

#[tokio::test]
async fn test_bulk_mark_books_as_read_empty_list() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Bulk mark empty list as read
    let request_body = BulkBooksRequest { book_ids: vec![] };
    let request = post_json_request_with_auth("/api/v1/books/bulk/read", &request_body, &token);
    let (status, response): (StatusCode, Option<MarkReadResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let mark_response = response.unwrap();
    assert_eq!(mark_response.count, 0);
    assert!(mark_response.message.contains("No books"));
}

#[tokio::test]
async fn test_bulk_mark_books_as_read_with_invalid_ids() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create 1 real book
    let book = create_test_book_model(series.id, library.id, "/test/book1.cbz", "book1.cbz", 50);
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Include real book and non-existent book IDs
    let request_body = BulkBooksRequest {
        book_ids: vec![book.id, uuid::Uuid::new_v4(), uuid::Uuid::new_v4()],
    };
    let request = post_json_request_with_auth("/api/v1/books/bulk/read", &request_body, &token);
    let (status, response): (StatusCode, Option<MarkReadResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let mark_response = response.unwrap();
    // Only the real book should be marked
    assert_eq!(mark_response.count, 1);

    // Verify only the real book is marked as read
    let progress = ReadProgressRepository::get_by_user_and_book(&db, user_id, book.id)
        .await
        .unwrap()
        .unwrap();
    assert!(progress.completed);
}

// ============================================================================
// Bulk Mark Books as Unread Tests
// ============================================================================

#[tokio::test]
async fn test_bulk_mark_books_as_unread() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create 3 test books
    let mut book_ids = Vec::new();
    for i in 1..=3 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            50,
        );
        let book = BookRepository::create(&db, &book, None).await.unwrap();
        book_ids.push(book.id);
    }

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_admin_and_token(&db, &state).await;

    // Create progress for all books
    for book_id in &book_ids {
        ReadProgressRepository::upsert(&db, user_id, *book_id, 25, false)
            .await
            .unwrap();
    }

    let app = create_test_router(state).await;

    // Bulk mark books as unread
    let request_body = BulkBooksRequest {
        book_ids: book_ids.clone(),
    };
    let request = post_json_request_with_auth("/api/v1/books/bulk/unread", &request_body, &token);
    let (status, response): (StatusCode, Option<MarkReadResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let mark_response = response.unwrap();
    assert_eq!(mark_response.count, 3);
    assert!(mark_response.message.contains("3 books"));

    // Verify all progress is deleted
    for book_id in book_ids {
        let progress = ReadProgressRepository::get_by_user_and_book(&db, user_id, book_id)
            .await
            .unwrap();
        assert!(progress.is_none());
    }
}

// ============================================================================
// Bulk Analyze Books Tests
// ============================================================================

#[tokio::test]
async fn test_bulk_analyze_books() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create 3 test books
    let mut book_ids = Vec::new();
    for i in 1..=3 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            50,
        );
        let book = BookRepository::create(&db, &book, None).await.unwrap();
        book_ids.push(book.id);
    }

    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Bulk analyze books
    let request_body = BulkAnalyzeBooksRequest {
        book_ids: book_ids.clone(),
        force: true,
    };
    let request = post_json_request_with_auth("/api/v1/books/bulk/analyze", &request_body, &token);
    let (status, response): (StatusCode, Option<BulkAnalyzeResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let analyze_response = response.unwrap();
    assert_eq!(analyze_response.tasks_enqueued, 3);
    assert!(analyze_response.message.contains("3 analysis tasks"));
}

#[tokio::test]
async fn test_bulk_analyze_books_empty_list() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Bulk analyze empty list
    let request_body = BulkAnalyzeBooksRequest {
        book_ids: vec![],
        force: false,
    };
    let request = post_json_request_with_auth("/api/v1/books/bulk/analyze", &request_body, &token);
    let (status, response): (StatusCode, Option<BulkAnalyzeResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let analyze_response = response.unwrap();
    assert_eq!(analyze_response.tasks_enqueued, 0);
    assert!(analyze_response.message.contains("No books"));
}

// ============================================================================
// Bulk Mark Series as Read Tests
// ============================================================================

#[tokio::test]
async fn test_bulk_mark_series_as_read() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    // Create 2 series with books
    let mut series_ids = Vec::new();
    let mut total_books = 0;
    for s in 1..=2 {
        let series = SeriesRepository::create(&db, library.id, &format!("Test Series {}", s), None)
            .await
            .unwrap();
        series_ids.push(series.id);

        // Create 3 books per series
        for i in 1..=3 {
            let book = create_test_book_model(
                series.id,
                library.id,
                &format!("/test/series{}/book{}.cbz", s, i),
                &format!("book{}.cbz", i),
                50,
            );
            BookRepository::create(&db, &book, None).await.unwrap();
            total_books += 1;
        }
    }

    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Bulk mark series as read
    let request_body = BulkSeriesRequest {
        series_ids: series_ids.clone(),
    };
    let request = post_json_request_with_auth("/api/v1/series/bulk/read", &request_body, &token);
    let (status, response): (StatusCode, Option<MarkReadResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let mark_response = response.unwrap();
    assert_eq!(mark_response.count, total_books);
    assert!(
        mark_response
            .message
            .contains(&format!("{} books", total_books))
    );
}

#[tokio::test]
async fn test_bulk_mark_series_as_read_empty_list() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Bulk mark empty series list as read
    let request_body = BulkSeriesRequest { series_ids: vec![] };
    let request = post_json_request_with_auth("/api/v1/series/bulk/read", &request_body, &token);
    let (status, response): (StatusCode, Option<MarkReadResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let mark_response = response.unwrap();
    assert_eq!(mark_response.count, 0);
    assert!(mark_response.message.contains("No series"));
}

// ============================================================================
// Bulk Mark Series as Unread Tests
// ============================================================================

#[tokio::test]
async fn test_bulk_mark_series_as_unread() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    // Create 2 series with books
    let mut series_ids = Vec::new();
    let mut all_book_ids = Vec::new();
    for s in 1..=2 {
        let series = SeriesRepository::create(&db, library.id, &format!("Test Series {}", s), None)
            .await
            .unwrap();
        series_ids.push(series.id);

        // Create 3 books per series
        for i in 1..=3 {
            let book = create_test_book_model(
                series.id,
                library.id,
                &format!("/test/series{}/book{}.cbz", s, i),
                &format!("book{}.cbz", i),
                50,
            );
            let book = BookRepository::create(&db, &book, None).await.unwrap();
            all_book_ids.push(book.id);
        }
    }

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_admin_and_token(&db, &state).await;

    // Create progress for all books
    for book_id in &all_book_ids {
        ReadProgressRepository::upsert(&db, user_id, *book_id, 25, false)
            .await
            .unwrap();
    }

    let app = create_test_router(state).await;

    // Bulk mark series as unread
    let request_body = BulkSeriesRequest {
        series_ids: series_ids.clone(),
    };
    let request = post_json_request_with_auth("/api/v1/series/bulk/unread", &request_body, &token);
    let (status, response): (StatusCode, Option<MarkReadResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let mark_response = response.unwrap();
    assert_eq!(mark_response.count, 6); // 2 series * 3 books
    assert!(mark_response.message.contains("6 books"));

    // Verify all progress is deleted
    for book_id in all_book_ids {
        let progress = ReadProgressRepository::get_by_user_and_book(&db, user_id, book_id)
            .await
            .unwrap();
        assert!(progress.is_none());
    }
}

// ============================================================================
// Bulk Analyze Series Tests
// ============================================================================

#[tokio::test]
async fn test_bulk_analyze_series() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    // Create 2 series with books
    let mut series_ids = Vec::new();
    let mut total_books = 0;
    for s in 1..=2 {
        let series = SeriesRepository::create(&db, library.id, &format!("Test Series {}", s), None)
            .await
            .unwrap();
        series_ids.push(series.id);

        // Create 3 books per series
        for i in 1..=3 {
            let book = create_test_book_model(
                series.id,
                library.id,
                &format!("/test/series{}/book{}.cbz", s, i),
                &format!("book{}.cbz", i),
                50,
            );
            BookRepository::create(&db, &book, None).await.unwrap();
            total_books += 1;
        }
    }

    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Bulk analyze series
    let request_body = BulkAnalyzeSeriesRequest {
        series_ids: series_ids.clone(),
        force: true,
    };
    let request = post_json_request_with_auth("/api/v1/series/bulk/analyze", &request_body, &token);
    let (status, response): (StatusCode, Option<BulkAnalyzeResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let analyze_response = response.unwrap();
    assert_eq!(analyze_response.tasks_enqueued, total_books);
    assert!(
        analyze_response
            .message
            .contains(&format!("{} analysis tasks", total_books))
    );
}

#[tokio::test]
async fn test_bulk_analyze_series_empty_list() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Bulk analyze empty series list
    let request_body = BulkAnalyzeSeriesRequest {
        series_ids: vec![],
        force: false,
    };
    let request = post_json_request_with_auth("/api/v1/series/bulk/analyze", &request_body, &token);
    let (status, response): (StatusCode, Option<BulkAnalyzeResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let analyze_response = response.unwrap();
    assert_eq!(analyze_response.tasks_enqueued, 0);
    assert!(analyze_response.message.contains("No series"));
}

// ============================================================================
// Authorization Tests
// ============================================================================

#[tokio::test]
async fn test_bulk_mark_books_as_read_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // Try to bulk mark books as read without auth
    let request_body = BulkBooksRequest {
        book_ids: vec![uuid::Uuid::new_v4()],
    };
    let request = post_json_request("/api/v1/books/bulk/read", &request_body);
    let (status, _): (StatusCode, Option<MarkReadResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_bulk_mark_series_as_read_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // Try to bulk mark series as read without auth
    let request_body = BulkSeriesRequest {
        series_ids: vec![uuid::Uuid::new_v4()],
    };
    let request = post_json_request("/api/v1/series/bulk/read", &request_body);
    let (status, _): (StatusCode, Option<MarkReadResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_bulk_analyze_books_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // Try to bulk analyze books without auth
    let request_body = BulkAnalyzeBooksRequest {
        book_ids: vec![uuid::Uuid::new_v4()],
        force: false,
    };
    let request = post_json_request("/api/v1/books/bulk/analyze", &request_body);
    let (status, _): (StatusCode, Option<BulkAnalyzeResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_bulk_analyze_series_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // Try to bulk analyze series without auth
    let request_body = BulkAnalyzeSeriesRequest {
        series_ids: vec![uuid::Uuid::new_v4()],
        force: false,
    };
    let request = post_json_request("/api/v1/series/bulk/analyze", &request_body);
    let (status, _): (StatusCode, Option<BulkAnalyzeResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Bulk track / untrack for releases
//
// The two endpoints enqueue a `BulkTrackForReleases` task and return its
// `task_id` immediately. These tests assert the HTTP enqueue surface
// (validation, permission, queued task row) and then drive the handler
// in-process to verify the same end-state contract the legacy sync
// handler used to provide. The full task-worker loop is exercised
// separately in `tests/task_queue_e2e.rs`.
// ============================================================================

/// Pull the enqueued task by id and invoke `BulkTrackForReleasesHandler`
/// directly. Mirrors the pattern in `tests/api/series.rs` for the
/// reprocess title tests.
async fn drive_bulk_track_task(
    db: &sea_orm::DatabaseConnection,
    task_id: uuid::Uuid,
) -> serde_json::Value {
    let task = TaskRepository::get_by_id(db, task_id)
        .await
        .unwrap()
        .expect("enqueued task row should be present");
    let handler = BulkTrackForReleasesHandler::new();
    let result = handler
        .handle(&task, db, None)
        .await
        .expect("handler should succeed");
    assert!(result.success, "handler should report success");
    result
        .data
        .expect("BulkTrackForReleases handler always populates result_data")
}

#[tokio::test]
async fn bulk_track_for_releases_enqueues_task_and_flips_tracked_on_drain() {
    use codex::db::repositories::{SeriesAliasRepository, SeriesTrackingRepository};

    let (db, _temp_dir) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let s1 = SeriesRepository::create(&db, library.id, "Vinland Saga", None)
        .await
        .unwrap();
    let s2 = SeriesRepository::create(&db, library.id, "Berserk", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request_body = BulkSeriesRequest {
        series_ids: vec![s1.id, s2.id],
    };
    let request = post_json_request_with_auth(
        "/api/v1/series/bulk/track-for-releases",
        &request_body,
        &token,
    );
    let (status, body): (StatusCode, Option<BulkTaskResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = body.expect("response body present");
    assert!(!body.task_id.is_nil(), "task_id should not be nil");
    assert!(
        body.message.contains("Track update queued"),
        "queued message should mention 'Track update queued': {}",
        body.message
    );

    // The handler hasn't run yet — series should still be untracked.
    for series_id in [s1.id, s2.id] {
        let row = SeriesTrackingRepository::get(&db, series_id).await.unwrap();
        assert!(
            row.map(|r| !r.tracked).unwrap_or(true),
            "series {} should still be untracked pre-drain",
            series_id
        );
    }

    // Drain the task in-process.
    let data = drive_bulk_track_task(&db, body.task_id).await;
    assert_eq!(data["tracked"], true);
    assert_eq!(data["changed"], 2);
    assert_eq!(data["already_in_state"], 0);
    assert_eq!(data["errored"], 0);
    assert_eq!(data["results"].as_array().unwrap().len(), 2);

    for series_id in [s1.id, s2.id] {
        let row = SeriesTrackingRepository::get(&db, series_id)
            .await
            .unwrap()
            .unwrap();
        assert!(row.tracked, "series {} should be tracked", series_id);

        let aliases = SeriesAliasRepository::get_for_series(&db, series_id)
            .await
            .unwrap();
        assert!(
            !aliases.is_empty(),
            "series {} should have a seeded alias",
            series_id
        );
    }
}

#[tokio::test]
async fn bulk_track_for_releases_skips_already_tracked_on_drain() {
    use codex::db::repositories::{SeriesTrackingRepository, TrackingUpdate};

    let (db, _temp_dir) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let already = SeriesRepository::create(&db, library.id, "Already", None)
        .await
        .unwrap();
    let fresh = SeriesRepository::create(&db, library.id, "Fresh", None)
        .await
        .unwrap();

    SeriesTrackingRepository::upsert(
        &db,
        already.id,
        TrackingUpdate {
            tracked: Some(true),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request_body = BulkSeriesRequest {
        series_ids: vec![already.id, fresh.id],
    };
    let request = post_json_request_with_auth(
        "/api/v1/series/bulk/track-for-releases",
        &request_body,
        &token,
    );
    let (status, body): (StatusCode, Option<BulkTaskResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();

    let data = drive_bulk_track_task(&db, body.task_id).await;
    assert_eq!(data["changed"], 1, "only `fresh` should flip");
    assert_eq!(data["already_in_state"], 1, "`already` is a no-op");
    assert_eq!(data["errored"], 0);

    // Per-series outcomes preserved in input order.
    let results = data["results"].as_array().unwrap();
    assert_eq!(
        results[0]["series_id"].as_str().unwrap(),
        already.id.to_string()
    );
    assert_eq!(results[0]["outcome"], "skipped");
    assert_eq!(
        results[1]["series_id"].as_str().unwrap(),
        fresh.id.to_string()
    );
    assert_eq!(results[1]["outcome"], "tracked");
}

#[tokio::test]
async fn bulk_track_for_releases_treats_missing_series_as_skipped_on_drain() {
    let (db, _temp_dir) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let real = SeriesRepository::create(&db, library.id, "Real", None)
        .await
        .unwrap();
    let bogus = uuid::Uuid::new_v4();

    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request_body = BulkSeriesRequest {
        series_ids: vec![bogus, real.id],
    };
    let request = post_json_request_with_auth(
        "/api/v1/series/bulk/track-for-releases",
        &request_body,
        &token,
    );
    let (status, body): (StatusCode, Option<BulkTaskResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();

    let data = drive_bulk_track_task(&db, body.task_id).await;
    // One bad series doesn't poison the others. Missing counts as skipped,
    // matching the legacy sync endpoint's `already_in_state` bookkeeping.
    assert_eq!(data["changed"], 1);
    assert_eq!(data["already_in_state"], 1);
    assert_eq!(data["errored"], 0);

    let results = data["results"].as_array().unwrap();
    let bogus_row = results
        .iter()
        .find(|r| r["series_id"].as_str() == Some(&bogus.to_string()))
        .expect("bogus row present");
    assert_eq!(bogus_row["outcome"], "skipped");
    assert!(
        bogus_row["detail"]
            .as_str()
            .unwrap_or("")
            .contains("not found"),
        "missing series detail should mention 'not found'"
    );
}

#[tokio::test]
async fn bulk_untrack_for_releases_flips_tracked_off_and_preserves_aliases_on_drain() {
    use codex::db::repositories::{
        SeriesAliasRepository, SeriesTrackingRepository, TrackingUpdate,
    };

    let (db, _temp_dir) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let s = SeriesRepository::create(&db, library.id, "Tracked", None)
        .await
        .unwrap();
    SeriesTrackingRepository::upsert(
        &db,
        s.id,
        TrackingUpdate {
            tracked: Some(true),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    SeriesAliasRepository::create(&db, s.id, "User Alias", "manual")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request_body = BulkSeriesRequest {
        series_ids: vec![s.id],
    };
    let request = post_json_request_with_auth(
        "/api/v1/series/bulk/untrack-for-releases",
        &request_body,
        &token,
    );
    let (status, body): (StatusCode, Option<BulkTaskResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();
    assert!(
        body.message.contains("Untrack update queued"),
        "queued message should mention 'Untrack update queued': {}",
        body.message
    );

    let data = drive_bulk_track_task(&db, body.task_id).await;
    assert_eq!(data["tracked"], false);
    assert_eq!(data["changed"], 1);
    assert_eq!(data["results"][0]["outcome"], "untracked");

    let row = SeriesTrackingRepository::get(&db, s.id)
        .await
        .unwrap()
        .unwrap();
    assert!(!row.tracked);

    // Aliases must survive — untrack is a soft toggle, not a delete.
    let aliases = SeriesAliasRepository::get_for_series(&db, s.id)
        .await
        .unwrap();
    assert!(aliases.iter().any(|a| a.alias == "User Alias"));
}

#[tokio::test]
async fn bulk_untrack_for_releases_skips_already_untracked_on_drain() {
    let (db, _temp_dir) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let s = SeriesRepository::create(&db, library.id, "Never tracked", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request_body = BulkSeriesRequest {
        series_ids: vec![s.id],
    };
    let request = post_json_request_with_auth(
        "/api/v1/series/bulk/untrack-for-releases",
        &request_body,
        &token,
    );
    let (status, body): (StatusCode, Option<BulkTaskResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();

    let data = drive_bulk_track_task(&db, body.task_id).await;
    assert_eq!(data["changed"], 0);
    assert_eq!(data["already_in_state"], 1);
    assert_eq!(data["results"][0]["outcome"], "skipped");
}

#[tokio::test]
async fn bulk_track_for_releases_requires_series_write() {
    use codex::api::error::ErrorResponse;

    let (db, _temp_dir) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let s = SeriesRepository::create(&db, library.id, "Anything", None)
        .await
        .unwrap();

    // Regular (non-admin) user — has reads but not SeriesWrite.
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user("regular", "regular@example.com", &password_hash, false);
    let created = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    let app = create_test_router(state).await;

    let request_body = BulkSeriesRequest {
        series_ids: vec![s.id],
    };
    let request = post_json_request_with_auth(
        "/api/v1/series/bulk/track-for-releases",
        &request_body,
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Permission denial must short-circuit before enqueue.
    let stats = TaskRepository::get_stats(&db).await.unwrap();
    assert_eq!(
        stats.total, 0,
        "forbidden request must not enqueue a task, got stats: {stats:?}"
    );
}

#[tokio::test]
async fn bulk_track_for_releases_rejects_empty_request() {
    use codex::api::error::ErrorResponse;

    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request_body = BulkSeriesRequest { series_ids: vec![] };
    let request = post_json_request_with_auth(
        "/api/v1/series/bulk/track-for-releases",
        &request_body,
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn bulk_track_for_releases_rejects_over_max_request_size() {
    use codex::api::error::ErrorResponse;

    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // 101 random UUIDs — over the 100 cap. The IDs don't have to exist;
    // the cap is enforced before any DB lookup.
    let series_ids: Vec<uuid::Uuid> = (0..101).map(|_| uuid::Uuid::new_v4()).collect();
    let request_body = BulkSeriesRequest { series_ids };
    let request = post_json_request_with_auth(
        "/api/v1/series/bulk/track-for-releases",
        &request_body,
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let stats = TaskRepository::get_stats(&db).await.unwrap();
    assert_eq!(
        stats.total, 0,
        "oversize request must not enqueue a task, got stats: {stats:?}"
    );
}

#[tokio::test]
async fn bulk_untrack_for_releases_rejects_over_max_request_size() {
    use codex::api::error::ErrorResponse;

    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let series_ids: Vec<uuid::Uuid> = (0..101).map(|_| uuid::Uuid::new_v4()).collect();
    let request_body = BulkSeriesRequest { series_ids };
    let request = post_json_request_with_auth(
        "/api/v1/series/bulk/untrack-for-releases",
        &request_body,
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
