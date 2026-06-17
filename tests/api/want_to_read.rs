#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::{
    BulkAddWantToReadResponse, SeriesDto, WantToReadEntryDto, WantToReadItemType,
    WantToReadListResponse,
};
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    BookRepository, LibraryRepository, SeriesRepository, UserRepository,
};
use codex::utils::password;
use common::*;
use hyper::StatusCode;

/// Create a user and return (id, token).
async fn user_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
    username: &str,
    is_admin: bool,
) -> (uuid::Uuid, String) {
    let password_hash = password::hash_password("pw123456").unwrap();
    let user = create_test_user(
        username,
        &format!("{username}@example.com"),
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

async fn a_book(
    db: &sea_orm::DatabaseConnection,
    series_id: uuid::Uuid,
    library_id: uuid::Uuid,
) -> codex::db::entities::books::Model {
    use chrono::Utc;
    let book = codex::db::entities::books::Model {
        id: uuid::Uuid::new_v4(),
        series_id,
        library_id,
        path: format!("/test/{}.cbz", uuid::Uuid::new_v4()),
        file_name: "book.cbz".to_string(),
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
        koreader_hash: None,
        epub_positions: None,
        epub_spine_items: None,
    };
    BookRepository::create(db, &book, None).await.unwrap()
}

async fn library_and_series(
    db: &sea_orm::DatabaseConnection,
) -> (
    codex::db::entities::libraries::Model,
    codex::db::entities::series::Model,
) {
    let library = LibraryRepository::create(db, "Lib", "/test", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(db, library.id, "Series", None)
        .await
        .unwrap();
    (library, series)
}

#[tokio::test]
async fn test_add_series_and_book_to_queue() {
    let (db, _t) = setup_test_db().await;
    let (_library, series) = library_and_series(&db).await;
    let book = a_book(&db, series.id, series.library_id).await;

    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "alice", false).await;
    let app = create_test_router(state).await;

    // Add a series.
    let req = post_json_request_with_auth(
        "/api/v1/want-to-read",
        &serde_json::json!({ "seriesId": series.id }),
        &token,
    );
    let (status, entry): (StatusCode, Option<WantToReadEntryDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::CREATED);
    let entry = entry.unwrap();
    assert_eq!(entry.item_type, WantToReadItemType::Series);
    assert_eq!(entry.series_id, Some(series.id));
    assert_eq!(entry.book_id, None);

    // Add a book.
    let req = post_json_request_with_auth(
        "/api/v1/want-to-read",
        &serde_json::json!({ "bookId": book.id }),
        &token,
    );
    let (status, entry): (StatusCode, Option<WantToReadEntryDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(entry.unwrap().item_type, WantToReadItemType::Book);

    // List the queue.
    let req = get_request_with_auth("/api/v1/want-to-read", &token);
    let (status, list): (StatusCode, Option<WantToReadListResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let list = list.unwrap();
    assert_eq!(list.total, 2);
    assert_eq!(list.items.len(), 2);
}

#[tokio::test]
async fn test_bulk_add_series_counts_added_and_already_present() {
    let (db, _t) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Lib", "/test", ScanningStrategy::Default)
        .await
        .unwrap();
    let s1 = SeriesRepository::create(&db, library.id, "S1", None)
        .await
        .unwrap();
    let s2 = SeriesRepository::create(&db, library.id, "S2", None)
        .await
        .unwrap();
    let s3 = SeriesRepository::create(&db, library.id, "S3", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "alice", false).await;
    let app = create_test_router(state).await;

    // Pre-flag s1 so the bulk call sees it as already present.
    let req = post_json_request_with_auth(
        "/api/v1/want-to-read",
        &serde_json::json!({ "seriesId": s1.id }),
        &token,
    );
    let _: (StatusCode, Option<WantToReadEntryDto>) = make_json_request(app.clone(), req).await;

    // Bulk add: s1 (already present), s2 twice (deduped), s3 (new), plus a
    // phantom id that should be silently skipped.
    let phantom = uuid::Uuid::new_v4();
    let req = post_json_request_with_auth(
        "/api/v1/want-to-read/bulk",
        &serde_json::json!({ "seriesIds": [s1.id, s2.id, s2.id, s3.id, phantom] }),
        &token,
    );
    let (status, resp): (StatusCode, Option<BulkAddWantToReadResponse>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);
    let resp = resp.unwrap();
    assert_eq!(resp.added, 2); // s2, s3
    assert_eq!(resp.already_present, 1); // s1

    // Queue now holds s1, s2, s3.
    let req = get_request_with_auth("/api/v1/want-to-read", &token);
    let (_s, list): (StatusCode, Option<WantToReadListResponse>) =
        make_json_request(app, req).await;
    assert_eq!(list.unwrap().total, 3);
}

#[tokio::test]
async fn test_bulk_add_books_and_mixed() {
    let (db, _t) = setup_test_db().await;
    let (_library, series) = library_and_series(&db).await;
    let b1 = a_book(&db, series.id, series.library_id).await;
    let b2 = a_book(&db, series.id, series.library_id).await;

    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "alice", false).await;
    let app = create_test_router(state).await;

    // A single bulk call carrying both series and books.
    let req = post_json_request_with_auth(
        "/api/v1/want-to-read/bulk",
        &serde_json::json!({ "seriesIds": [series.id], "bookIds": [b1.id, b2.id] }),
        &token,
    );
    let (status, resp): (StatusCode, Option<BulkAddWantToReadResponse>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);
    let resp = resp.unwrap();
    assert_eq!(resp.added, 3);
    assert_eq!(resp.already_present, 0);

    let req = get_request_with_auth("/api/v1/want-to-read", &token);
    let (_s, list): (StatusCode, Option<WantToReadListResponse>) =
        make_json_request(app, req).await;
    assert_eq!(list.unwrap().total, 3);
}

#[tokio::test]
async fn test_bulk_add_requires_authentication() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let req = post_json_request(
        "/api/v1/want-to-read/bulk",
        &serde_json::json!({ "seriesIds": [uuid::Uuid::new_v4()] }),
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_add_requires_exactly_one_target() {
    let (db, _t) = setup_test_db().await;
    let (_library, series) = library_and_series(&db).await;
    let book = a_book(&db, series.id, series.library_id).await;

    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "alice", false).await;
    let app = create_test_router(state).await;

    // Neither provided -> 400.
    let req = post_json_request_with_auth("/api/v1/want-to-read", &serde_json::json!({}), &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Both provided -> 400.
    let req = post_json_request_with_auth(
        "/api/v1/want-to-read",
        &serde_json::json!({ "seriesId": series.id, "bookId": book.id }),
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_add_nonexistent_returns_404() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "alice", false).await;
    let app = create_test_router(state).await;

    let req = post_json_request_with_auth(
        "/api/v1/want-to-read",
        &serde_json::json!({ "seriesId": uuid::Uuid::new_v4() }),
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_idempotent_add_and_remove() {
    let (db, _t) = setup_test_db().await;
    let (_library, series) = library_and_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "alice", false).await;
    let app = create_test_router(state).await;

    let body = serde_json::json!({ "seriesId": series.id });
    for _ in 0..2 {
        let req = post_json_request_with_auth("/api/v1/want-to-read", &body, &token);
        let (status, _): (StatusCode, Option<WantToReadEntryDto>) =
            make_json_request(app.clone(), req).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    // Still only one entry.
    let req = get_request_with_auth("/api/v1/want-to-read", &token);
    let (_s, list): (StatusCode, Option<WantToReadListResponse>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(list.unwrap().total, 1);

    // Remove it.
    let req = delete_request_with_auth(
        &format!("/api/v1/want-to-read/series/{}", series.id),
        &token,
    );
    let (status, _): (StatusCode, Option<String>) = make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let req = get_request_with_auth("/api/v1/want-to-read", &token);
    let (_s, list): (StatusCode, Option<WantToReadListResponse>) =
        make_json_request(app, req).await;
    assert_eq!(list.unwrap().total, 0);
}

#[tokio::test]
async fn test_queue_is_per_user() {
    let (db, _t) = setup_test_db().await;
    let (_library, series) = library_and_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let (_alice, alice_token) = user_and_token(&db, &state, "alice", false).await;
    let (_bob, bob_token) = user_and_token(&db, &state, "bob", false).await;
    let app = create_test_router(state).await;

    // Alice adds a series.
    let req = post_json_request_with_auth(
        "/api/v1/want-to-read",
        &serde_json::json!({ "seriesId": series.id }),
        &alice_token,
    );
    let (status, _): (StatusCode, Option<WantToReadEntryDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::CREATED);

    // Bob's queue is empty.
    let req = get_request_with_auth("/api/v1/want-to-read", &bob_token);
    let (_s, list): (StatusCode, Option<WantToReadListResponse>) =
        make_json_request(app, req).await;
    assert_eq!(list.unwrap().total, 0);
}

#[tokio::test]
async fn test_list_sort_direction() {
    let (db, _t) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Lib", "/test", ScanningStrategy::Default)
        .await
        .unwrap();
    let first = SeriesRepository::create(&db, library.id, "First", None)
        .await
        .unwrap();
    let second = SeriesRepository::create(&db, library.id, "Second", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "alice", false).await;
    let app = create_test_router(state).await;

    // Add `first`, then `second`, with a gap so added_at is strictly ordered.
    let req = post_json_request_with_auth(
        "/api/v1/want-to-read",
        &serde_json::json!({ "seriesId": first.id }),
        &token,
    );
    let _: (StatusCode, Option<WantToReadEntryDto>) = make_json_request(app.clone(), req).await;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    let req = post_json_request_with_auth(
        "/api/v1/want-to-read",
        &serde_json::json!({ "seriesId": second.id }),
        &token,
    );
    let _: (StatusCode, Option<WantToReadEntryDto>) = make_json_request(app.clone(), req).await;

    // Default (desc): newest first => `second` before `first`.
    let req = get_request_with_auth("/api/v1/want-to-read", &token);
    let (_s, list): (StatusCode, Option<WantToReadListResponse>) =
        make_json_request(app.clone(), req).await;
    let desc = list.unwrap().items;
    assert_eq!(desc[0].series_id, Some(second.id));
    assert_eq!(desc[1].series_id, Some(first.id));

    // Ascending: oldest first => `first` before `second`.
    let req = get_request_with_auth("/api/v1/want-to-read?sort=added_at:asc", &token);
    let (_s, list): (StatusCode, Option<WantToReadListResponse>) =
        make_json_request(app, req).await;
    let asc = list.unwrap().items;
    assert_eq!(asc[0].series_id, Some(first.id));
    assert_eq!(asc[1].series_id, Some(second.id));
}

#[tokio::test]
async fn test_requires_authentication() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let req = get_request("/api/v1/want-to-read");
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_series_dto_exposes_want_to_read_flag() {
    let (db, _t) = setup_test_db().await;
    let (_library, series) = library_and_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "alice", true).await;
    let app = create_test_router(state).await;

    // Before flagging, the detail DTO reports false.
    let req = get_request_with_auth(&format!("/api/v1/series/{}", series.id), &token);
    let (status, dto): (StatusCode, Option<SeriesDto>) = make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dto.unwrap().want_to_read, Some(false));

    // Flag it.
    let req = post_json_request_with_auth(
        "/api/v1/want-to-read",
        &serde_json::json!({ "seriesId": series.id }),
        &token,
    );
    let _: (StatusCode, Option<WantToReadEntryDto>) = make_json_request(app.clone(), req).await;

    // Now the detail DTO reports true.
    let req = get_request_with_auth(&format!("/api/v1/series/{}", series.id), &token);
    let (_s, dto): (StatusCode, Option<SeriesDto>) = make_json_request(app, req).await;
    assert_eq!(dto.unwrap().want_to_read, Some(true));
}
