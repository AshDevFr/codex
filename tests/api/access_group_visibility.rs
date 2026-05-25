//! Integration tests covering sharing-tag visibility on book and series
//! listing endpoints. These guard the bug where non-series-listing endpoints
//! returned books/series the caller did not have access to via their
//! ContentFilter (either directly or through access groups).

#[path = "../common/mod.rs"]
mod common;

use codex::api::routes::v1::dto::book::BookListResponse;
use codex::api::routes::v1::dto::series::SeriesDto;
use codex::db::ScanningStrategy;
use codex::db::entities::user_sharing_tags::AccessMode;
use codex::db::repositories::{
    BookMetadataRepository, BookRepository, LibraryRepository, SeriesRepository,
    SharingTagRepository, UserRepository,
};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

async fn create_user_and_token(
    db: &DatabaseConnection,
    state: &codex::api::extractors::AuthState,
    username: &str,
    is_admin: bool,
) -> (Uuid, String) {
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

/// Seed two series in one library: one tagged "mature", one tagged "kids".
/// Each series gets one book. Returns (mature_series_id, kids_series_id).
async fn seed_two_tagged_series(db: &DatabaseConnection) -> (Uuid, Uuid, Uuid, Uuid) {
    let library =
        LibraryRepository::create(db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();

    let mature_series = SeriesRepository::create(db, library.id, "Mature Series", None)
        .await
        .unwrap();
    let kids_series = SeriesRepository::create(db, library.id, "Kids Series", None)
        .await
        .unwrap();

    let mature_book = create_test_book(
        mature_series.id,
        library.id,
        "/test/mature.cbz",
        "mature.cbz",
        "hash-mature",
        "cbz",
        10,
    );
    let mature_book = BookRepository::create(db, &mature_book, None)
        .await
        .unwrap();
    BookMetadataRepository::create_with_title_and_number(
        db,
        mature_book.id,
        Some("Mature Book".to_string()),
        None,
    )
    .await
    .unwrap();

    let kids_book = create_test_book(
        kids_series.id,
        library.id,
        "/test/kids.cbz",
        "kids.cbz",
        "hash-kids",
        "cbz",
        10,
    );
    let kids_book = BookRepository::create(db, &kids_book, None).await.unwrap();
    BookMetadataRepository::create_with_title_and_number(
        db,
        kids_book.id,
        Some("Kids Book".to_string()),
        None,
    )
    .await
    .unwrap();

    let mature_tag = SharingTagRepository::create(db, "mature", None)
        .await
        .unwrap();
    let kids_tag = SharingTagRepository::create(db, "kids", None)
        .await
        .unwrap();
    SharingTagRepository::add_tag_to_series(db, mature_series.id, mature_tag.id)
        .await
        .unwrap();
    SharingTagRepository::add_tag_to_series(db, kids_series.id, kids_tag.id)
        .await
        .unwrap();

    (mature_series.id, kids_series.id, mature_tag.id, kids_tag.id)
}

/// User with a `deny` grant on `mature` must not see mature books in
/// `/api/v1/books/recently-added`. The bug we are guarding: this endpoint
/// previously skipped the ContentFilter entirely.
#[tokio::test]
async fn test_recently_added_books_hides_denied_series() {
    let (db, _temp_dir) = setup_test_db().await;
    let (mature_series_id, _kids_series_id, mature_tag_id, _) = seed_two_tagged_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "reader", false).await;
    SharingTagRepository::set_user_grant(&db, user_id, mature_tag_id, AccessMode::Deny)
        .await
        .unwrap();

    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/books/recently-added", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert_eq!(body.data.len(), 1, "should only see the non-denied book");
    assert_eq!(body.total, 1, "total count must reflect filtered view");
    assert!(
        body.data
            .iter()
            .all(|b: &codex::api::routes::v1::dto::book::BookDto| b.series_id != mature_series_id),
        "denied series book must not appear in recently-added"
    );
}

/// User with an `allow` grant only on `kids` should be in whitelist mode and
/// only see kids books on `POST /api/v1/books/list`.
#[tokio::test]
async fn test_books_list_endpoint_honors_whitelist() {
    use codex::api::routes::v1::dto::filter::BookListRequest;

    let (db, _temp_dir) = setup_test_db().await;
    let (mature_series_id, kids_series_id, _, kids_tag_id) = seed_two_tagged_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "reader", false).await;
    SharingTagRepository::set_user_grant(&db, user_id, kids_tag_id, AccessMode::Allow)
        .await
        .unwrap();

    let body = BookListRequest {
        condition: None,
        full_text_search: None,
        include_deleted: false,
    };
    let app = create_test_router(state).await;
    let request = post_json_request_with_auth("/api/v1/books/list", &body, &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert_eq!(body.data.len(), 1, "whitelist mode shows only allowed");
    assert_eq!(body.total, 1);
    let only = &body.data[0];
    assert_eq!(only.series_id, kids_series_id);
    assert_ne!(only.series_id, mature_series_id);
}

/// Sanity: an admin (no grants at all) still sees everything.
#[tokio::test]
async fn test_admin_with_no_grants_sees_all_recently_added() {
    let (db, _temp_dir) = setup_test_db().await;
    let _ = seed_two_tagged_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "admin", true).await;

    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/books/recently-added", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert_eq!(body.data.len(), 2);
    assert_eq!(body.total, 2);
}

/// Series listing was already filtered in-memory; this test pins the new
/// SQL-level path: a denied user must not see the series, and the paginated
/// `total` must reflect the filtered view (the regression scenario when we
/// moved from in-memory filtering to SQL filtering).
#[tokio::test]
async fn test_series_list_total_reflects_visibility() {
    let (db, _temp_dir) = setup_test_db().await;
    let (mature_series_id, _kids_series_id, mature_tag_id, _) = seed_two_tagged_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "reader", false).await;
    SharingTagRepository::set_user_grant(&db, user_id, mature_tag_id, AccessMode::Deny)
        .await
        .unwrap();

    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/series", &token);
    let (status, body_bytes) = make_raw_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let total = body["total"].as_u64().unwrap_or(0);
    let data = body["data"].as_array().unwrap();
    assert_eq!(total, 1, "denied series must not count toward total");
    assert_eq!(data.len(), 1);
    let series: SeriesDto = serde_json::from_value(data[0].clone()).unwrap();
    assert_ne!(series.id, mature_series_id);
}

// ============================================================================
// Single-book endpoints: /api/v1/books/{id} must 404 if user can't see series.
// ============================================================================

async fn first_book_in_series(db: &sea_orm::DatabaseConnection, series_id: Uuid) -> Uuid {
    let books = BookRepository::list_by_series(db, series_id, false)
        .await
        .unwrap();
    books.first().unwrap().id
}

#[tokio::test]
async fn test_get_book_returns_404_for_denied_series() {
    let (db, _temp_dir) = setup_test_db().await;
    let (mature_series_id, _kids_series_id, mature_tag_id, _) = seed_two_tagged_series(&db).await;
    let mature_book_id = first_book_in_series(&db, mature_series_id).await;

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "reader", false).await;
    SharingTagRepository::set_user_grant(&db, user_id, mature_tag_id, AccessMode::Deny)
        .await
        .unwrap();

    let app = create_test_router(state).await;
    let request = get_request_with_auth(&format!("/api/v1/books/{}", mature_book_id), &token);
    let (status, _body) = make_raw_request(app, request).await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "denied series must hide its books"
    );
}

#[tokio::test]
async fn test_get_book_file_returns_404_for_denied_series() {
    let (db, _temp_dir) = setup_test_db().await;
    let (mature_series_id, _kids_series_id, mature_tag_id, _) = seed_two_tagged_series(&db).await;
    let mature_book_id = first_book_in_series(&db, mature_series_id).await;

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "reader", false).await;
    SharingTagRepository::set_user_grant(&db, user_id, mature_tag_id, AccessMode::Deny)
        .await
        .unwrap();

    let app = create_test_router(state).await;
    let request = get_request_with_auth(&format!("/api/v1/books/{}/file", mature_book_id), &token);
    let (status, _body) = make_raw_request(app, request).await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "denied series must hide its book file"
    );
}

// ============================================================================
// Library-scoped book endpoints: /api/v1/libraries/{id}/books/recently-added.
// ============================================================================

#[tokio::test]
async fn test_library_recently_added_books_hides_denied_series() {
    let (db, _temp_dir) = setup_test_db().await;
    let (mature_series_id, _kids_series_id, mature_tag_id, _) = seed_two_tagged_series(&db).await;
    let lib_id = LibraryRepository::list_all(&db).await.unwrap()[0].id;

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "reader", false).await;
    SharingTagRepository::set_user_grant(&db, user_id, mature_tag_id, AccessMode::Deny)
        .await
        .unwrap();

    let app = create_test_router(state).await;
    let request = get_request_with_auth(
        &format!("/api/v1/libraries/{}/books/recently-added", lib_id),
        &token,
    );
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert_eq!(body.total, 1);
    assert!(
        body.data.iter().all(|b| b.series_id != mature_series_id),
        "denied series book must not appear",
    );
}

// ============================================================================
// On-deck endpoint: denied series must not have an on-deck book either.
// ============================================================================

#[tokio::test]
async fn test_on_deck_hides_denied_series() {
    use codex::db::repositories::{BookMetadataRepository, ReadProgressRepository};

    let (db, _temp_dir) = setup_test_db().await;
    let (mature_series_id, kids_series_id, mature_tag_id, _) = seed_two_tagged_series(&db).await;
    let lib_id = LibraryRepository::list_all(&db).await.unwrap()[0].id;

    // Add a second book to each series and mark the first one completed
    // so the second is "on deck".
    let mature_book_id = first_book_in_series(&db, mature_series_id).await;
    let kids_book_id = first_book_in_series(&db, kids_series_id).await;

    let mature_b2 = create_test_book(
        mature_series_id,
        lib_id,
        "/test/mature-2.cbz",
        "mature-2.cbz",
        "hash-m2",
        "cbz",
        10,
    );
    let mature_b2 = BookRepository::create(&db, &mature_b2, None).await.unwrap();
    BookMetadataRepository::create_with_title_and_number(
        &db,
        mature_b2.id,
        Some("Mature 2".to_string()),
        None,
    )
    .await
    .unwrap();

    let kids_b2 = create_test_book(
        kids_series_id,
        lib_id,
        "/test/kids-2.cbz",
        "kids-2.cbz",
        "hash-k2",
        "cbz",
        10,
    );
    let kids_b2 = BookRepository::create(&db, &kids_b2, None).await.unwrap();
    BookMetadataRepository::create_with_title_and_number(
        &db,
        kids_b2.id,
        Some("Kids 2".to_string()),
        None,
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "reader", false).await;
    ReadProgressRepository::upsert(&db, user_id, mature_book_id, 10, true)
        .await
        .unwrap();
    ReadProgressRepository::upsert(&db, user_id, kids_book_id, 10, true)
        .await
        .unwrap();
    SharingTagRepository::set_user_grant(&db, user_id, mature_tag_id, AccessMode::Deny)
        .await
        .unwrap();

    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/books/on-deck", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert_eq!(body.total, 1, "only kids on-deck book should remain");
    assert_eq!(body.data.len(), 1);
    assert_ne!(body.data[0].id, mature_b2.id);
}

// ============================================================================
// Search endpoints honor visibility.
// ============================================================================

// ============================================================================
// Series recently-added: visibility was previously enforced in-memory after a
// repo fetch; now it must be enforced at SQL.
// ============================================================================

#[tokio::test]
async fn test_recently_added_series_hides_denied() {
    let (db, _temp_dir) = setup_test_db().await;
    let (mature_series_id, _kids_series_id, mature_tag_id, _) = seed_two_tagged_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "reader", false).await;
    SharingTagRepository::set_user_grant(&db, user_id, mature_tag_id, AccessMode::Deny)
        .await
        .unwrap();

    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/series/recently-added", &token);
    let (status, body_bytes) = make_raw_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let value: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let series_list = value.as_array().unwrap();
    assert_eq!(series_list.len(), 1);
    let first: SeriesDto = serde_json::from_value(series_list[0].clone()).unwrap();
    assert_ne!(first.id, mature_series_id);
}

#[tokio::test]
async fn test_search_series_endpoint_hides_denied() {
    use codex::api::routes::v1::dto::series::SearchSeriesRequest;

    let (db, _temp_dir) = setup_test_db().await;
    let (mature_series_id, _kids_series_id, mature_tag_id, _) = seed_two_tagged_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "reader", false).await;
    SharingTagRepository::set_user_grant(&db, user_id, mature_tag_id, AccessMode::Deny)
        .await
        .unwrap();

    let body = SearchSeriesRequest {
        query: "Series".to_string(),
        library_id: None,
        full: false,
    };
    let app = create_test_router(state).await;
    let request = post_json_request_with_auth("/api/v1/series/search", &body, &token);
    let (status, body_bytes) = make_raw_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let value: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let arr = value.as_array().unwrap();
    for item in arr {
        let s: SeriesDto = serde_json::from_value(item.clone()).unwrap();
        assert_ne!(
            s.id, mature_series_id,
            "denied series must not appear in search"
        );
    }
}

#[tokio::test]
async fn test_books_filter_endpoint_fuzzy_off_hides_denied() {
    use codex::api::routes::v1::dto::filter::BookListRequest;

    let (db, _temp_dir) = setup_test_db().await;
    let (mature_series_id, _kids_series_id, mature_tag_id, _) = seed_two_tagged_series(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "reader", false).await;
    SharingTagRepository::set_user_grant(&db, user_id, mature_tag_id, AccessMode::Deny)
        .await
        .unwrap();

    // LIKE-search path: full_text_search set, fuzzy off (default in tests).
    let body = BookListRequest {
        condition: None,
        full_text_search: Some("Book".to_string()),
        include_deleted: false,
    };
    let app = create_test_router(state).await;
    let request = post_json_request_with_auth("/api/v1/books/list", &body, &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert!(
        body.data.iter().all(|b| b.series_id != mature_series_id),
        "LIKE-search path must drop denied books"
    );
}
