//! Read completion history over the native v1 API.
//!
//! The history is a separate record from current progress: marking something
//! unread resets progress and leaves history intact, and clearing history leaves
//! progress intact. These tests pin both directions, the three reset scopes,
//! cross-user isolation, and that completions arriving through the Komga and
//! KOReader compatibility layers are recorded too.

#[path = "../common/mod.rs"]
mod common;

use codex::api::routes::v1::dto::ReadHistoryResponse;
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    BookRepository, LibraryRepository, ReadProgressRepository, SeriesRepository, UserRepository,
};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use uuid::Uuid;

async fn user_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
    username: &str,
) -> (Uuid, String) {
    let password_hash = password::hash_password("pw123456").unwrap();
    let user = create_test_user(
        username,
        &format!("{username}@example.com"),
        &password_hash,
        true,
    );
    let created = UserRepository::create(db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    (created.id, token)
}

/// A library with one series and `count` books, each 50 pages.
async fn library_with_books(
    db: &sea_orm::DatabaseConnection,
    count: usize,
) -> (Uuid, Vec<codex::db::entities::books::Model>) {
    let library = LibraryRepository::create(db, "Lib", "/test", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(db, library.id, "Series", None)
        .await
        .unwrap();

    let mut books = Vec::new();
    for i in 0..count {
        let book = create_test_book(
            series.id,
            library.id,
            &format!("/test/book{i}.cbz"),
            &format!("book{i}.cbz"),
            &format!("hash{i}"),
            "cbz",
            50,
        );
        books.push(BookRepository::create(db, &book, None).await.unwrap());
    }
    (series.id, books)
}

async fn get_book_history(
    app: axum::Router,
    token: &str,
    book_id: Uuid,
) -> (StatusCode, Option<ReadHistoryResponse>) {
    let request = get_request_with_auth(&format!("/api/v1/books/{book_id}/read-history"), token);
    make_json_request(app, request).await
}

async fn get_series_history(
    app: axum::Router,
    token: &str,
    series_id: Uuid,
) -> (StatusCode, Option<ReadHistoryResponse>) {
    let request = get_request_with_auth(&format!("/api/v1/series/{series_id}/read-history"), token);
    make_json_request(app, request).await
}

// ============================================================================
// Reading history
// ============================================================================

#[tokio::test]
async fn test_book_history_is_empty_before_any_completion() {
    let (db, _tmp) = setup_test_db().await;
    let (_series_id, books) = library_with_books(&db, 1).await;
    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "reader").await;

    let (status, history) =
        get_book_history(create_test_router(state).await, &token, books[0].id).await;

    assert_eq!(status, StatusCode::OK);
    let history = history.unwrap();
    assert_eq!(history.read_count, 0);
    assert!(history.last_completed_at.is_none());
    assert!(history.entries.is_empty());
}

#[tokio::test]
async fn test_completing_a_book_records_one_entry() {
    let (db, _tmp) = setup_test_db().await;
    let (_series_id, books) = library_with_books(&db, 1).await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = user_and_token(&db, &state, "reader").await;

    let progress = ReadProgressRepository::mark_as_read(&db, user_id, books[0].id, 50)
        .await
        .unwrap();

    let (status, history) =
        get_book_history(create_test_router(state).await, &token, books[0].id).await;

    assert_eq!(status, StatusCode::OK);
    let history = history.unwrap();
    assert_eq!(history.read_count, 1);
    assert_eq!(history.entries.len(), 1);
    assert_eq!(history.entries[0].started_at, progress.started_at);
    assert_eq!(
        history.entries[0].completed_at,
        progress.completed_at.unwrap()
    );
    assert_eq!(history.last_completed_at, progress.completed_at);
}

/// The point of the feature: marking unread and re-reading yields two entries,
/// and the first one is not lost.
#[tokio::test]
async fn test_re_reading_after_marking_unread_records_two_entries() {
    let (db, _tmp) = setup_test_db().await;
    let (_series_id, books) = library_with_books(&db, 1).await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = user_and_token(&db, &state, "reader").await;

    ReadProgressRepository::mark_as_read(&db, user_id, books[0].id, 50)
        .await
        .unwrap();
    ReadProgressRepository::mark_as_unread(&db, user_id, books[0].id)
        .await
        .unwrap();
    ReadProgressRepository::mark_as_read(&db, user_id, books[0].id, 50)
        .await
        .unwrap();

    let (status, history) =
        get_book_history(create_test_router(state).await, &token, books[0].id).await;

    assert_eq!(status, StatusCode::OK);
    let history = history.unwrap();
    assert_eq!(history.read_count, 2);
    assert_eq!(history.entries.len(), 2);
    // Newest first.
    assert!(history.entries[0].completed_at >= history.entries[1].completed_at);
}

/// Marking unread must not erase history, even though it deletes progress.
#[tokio::test]
async fn test_marking_unread_preserves_history_and_clears_progress() {
    let (db, _tmp) = setup_test_db().await;
    let (_series_id, books) = library_with_books(&db, 1).await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = user_and_token(&db, &state, "reader").await;

    ReadProgressRepository::mark_as_read(&db, user_id, books[0].id, 50)
        .await
        .unwrap();
    ReadProgressRepository::mark_as_unread(&db, user_id, books[0].id)
        .await
        .unwrap();

    assert!(
        ReadProgressRepository::get_by_user_and_book(&db, user_id, books[0].id)
            .await
            .unwrap()
            .is_none(),
        "progress should be gone"
    );

    let (_status, history) =
        get_book_history(create_test_router(state).await, &token, books[0].id).await;
    assert_eq!(
        history.unwrap().read_count,
        1,
        "history should survive marking unread"
    );
}

#[tokio::test]
async fn test_book_history_404s_for_an_unknown_book() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "reader").await;

    let (status, _) =
        get_book_history(create_test_router(state).await, &token, Uuid::new_v4()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Series history: the minimum across its books
// ============================================================================

#[tokio::test]
async fn test_series_history_needs_every_book_completed() {
    let (db, _tmp) = setup_test_db().await;
    let (series_id, books) = library_with_books(&db, 3).await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = user_and_token(&db, &state, "reader").await;

    // Two of three read: the series has not been read.
    for book in &books[..2] {
        ReadProgressRepository::mark_as_read(&db, user_id, book.id, 50)
            .await
            .unwrap();
    }
    let (status, history) =
        get_series_history(create_test_router(state.clone()).await, &token, series_id).await;
    assert_eq!(status, StatusCode::OK);
    let history = history.unwrap();
    assert_eq!(
        history.read_count, 0,
        "a series is read only once every book is"
    );
    assert!(history.entries.is_empty());

    // The last one lands, so the series has been read once.
    ReadProgressRepository::mark_as_read(&db, user_id, books[2].id, 50)
        .await
        .unwrap();
    let (_status, history) =
        get_series_history(create_test_router(state).await, &token, series_id).await;
    let history = history.unwrap();
    assert_eq!(history.read_count, 1);
    assert_eq!(history.entries.len(), 1);
    assert!(history.last_completed_at.is_some());
}

#[tokio::test]
async fn test_series_history_counts_a_full_second_pass() {
    let (db, _tmp) = setup_test_db().await;
    let (series_id, books) = library_with_books(&db, 2).await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = user_and_token(&db, &state, "reader").await;

    let book_ids: Vec<Uuid> = books.iter().map(|b| b.id).collect();
    let pages: Vec<(Uuid, i32)> = book_ids.iter().map(|id| (*id, 50)).collect();

    ReadProgressRepository::mark_series_as_read(&db, user_id, pages.clone())
        .await
        .unwrap();
    ReadProgressRepository::mark_series_as_unread(&db, user_id, book_ids.clone())
        .await
        .unwrap();
    ReadProgressRepository::mark_series_as_read(&db, user_id, pages)
        .await
        .unwrap();

    let (_status, history) =
        get_series_history(create_test_router(state).await, &token, series_id).await;
    let history = history.unwrap();
    assert_eq!(history.read_count, 2);
    assert_eq!(history.entries.len(), 2);
}

/// Re-reading only one volume of a finished series does not advance the series
/// count, because the other volume has not had a second pass.
#[tokio::test]
async fn test_series_count_is_the_minimum_not_the_maximum() {
    let (db, _tmp) = setup_test_db().await;
    let (series_id, books) = library_with_books(&db, 2).await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = user_and_token(&db, &state, "reader").await;

    for book in &books {
        ReadProgressRepository::mark_as_read(&db, user_id, book.id, 50)
            .await
            .unwrap();
    }
    // Second pass of the first volume only.
    ReadProgressRepository::mark_as_unread(&db, user_id, books[0].id)
        .await
        .unwrap();
    ReadProgressRepository::mark_as_read(&db, user_id, books[0].id, 50)
        .await
        .unwrap();

    let (_status, history) =
        get_series_history(create_test_router(state).await, &token, series_id).await;
    assert_eq!(
        history.unwrap().read_count,
        1,
        "one volume read twice does not make the series read twice"
    );
}

/// A series with no books reports 0 rather than erroring or reporting a NULL
/// minimum.
#[tokio::test]
async fn test_empty_series_reports_zero() {
    let (db, _tmp) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Lib", "/test", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Empty", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "reader").await;

    let (status, history) =
        get_series_history(create_test_router(state).await, &token, series.id).await;
    assert_eq!(status, StatusCode::OK);
    let history = history.unwrap();
    assert_eq!(history.read_count, 0);
    assert!(history.last_completed_at.is_none());
}

#[tokio::test]
async fn test_series_history_404s_for_an_unknown_series() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "reader").await;

    let (status, _) =
        get_series_history(create_test_router(state).await, &token, Uuid::new_v4()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// The three reset scopes
// ============================================================================

#[tokio::test]
async fn test_clearing_a_book_leaves_its_siblings_and_progress_alone() {
    let (db, _tmp) = setup_test_db().await;
    let (_series_id, books) = library_with_books(&db, 2).await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = user_and_token(&db, &state, "reader").await;

    for book in &books {
        ReadProgressRepository::mark_as_read(&db, user_id, book.id, 50)
            .await
            .unwrap();
    }

    let app = create_test_router(state.clone()).await;
    let request = delete_request_with_auth(
        &format!("/api/v1/books/{}/read-history", books[0].id),
        &token,
    );
    let (status, _) = make_raw_request(app, request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_s, cleared) =
        get_book_history(create_test_router(state.clone()).await, &token, books[0].id).await;
    assert_eq!(cleared.unwrap().read_count, 0);

    let (_s, sibling) =
        get_book_history(create_test_router(state).await, &token, books[1].id).await;
    assert_eq!(
        sibling.unwrap().read_count,
        1,
        "clearing one book must not touch another"
    );

    // Current progress is untouched: the book is still marked read.
    let progress = ReadProgressRepository::get_by_user_and_book(&db, user_id, books[0].id)
        .await
        .unwrap()
        .expect("progress should survive a history reset");
    assert!(progress.completed);
}

#[tokio::test]
async fn test_clearing_a_series_clears_all_its_books() {
    let (db, _tmp) = setup_test_db().await;
    let (series_id, books) = library_with_books(&db, 2).await;
    // A second series that must be spared.
    let (other_series, other_books) = library_with_books(&db, 1).await;

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = user_and_token(&db, &state, "reader").await;

    for book in books.iter().chain(other_books.iter()) {
        ReadProgressRepository::mark_as_read(&db, user_id, book.id, 50)
            .await
            .unwrap();
    }

    let app = create_test_router(state.clone()).await;
    let request =
        delete_request_with_auth(&format!("/api/v1/series/{series_id}/read-history"), &token);
    let (status, _) = make_raw_request(app, request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    for book in &books {
        let (_s, history) =
            get_book_history(create_test_router(state.clone()).await, &token, book.id).await;
        assert_eq!(history.unwrap().read_count, 0);
    }

    let (_s, spared) =
        get_series_history(create_test_router(state).await, &token, other_series).await;
    assert_eq!(
        spared.unwrap().read_count,
        1,
        "another series must keep its history"
    );
}

#[tokio::test]
async fn test_clearing_everything_for_the_current_user() {
    let (db, _tmp) = setup_test_db().await;
    let (_series_id, books) = library_with_books(&db, 2).await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = user_and_token(&db, &state, "reader").await;

    for book in &books {
        ReadProgressRepository::mark_as_read(&db, user_id, book.id, 50)
            .await
            .unwrap();
    }

    let app = create_test_router(state.clone()).await;
    let request = delete_request_with_auth("/api/v1/user/read-history", &token);
    let (status, _) = make_raw_request(app, request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    for book in &books {
        let (_s, history) =
            get_book_history(create_test_router(state.clone()).await, &token, book.id).await;
        assert_eq!(history.unwrap().read_count, 0);
    }

    // Progress is untouched by an account-wide history reset.
    assert!(
        ReadProgressRepository::get_by_user_and_book(&db, user_id, books[0].id)
            .await
            .unwrap()
            .is_some_and(|p| p.completed)
    );
}

// ============================================================================
// Cross-user isolation
// ============================================================================

#[tokio::test]
async fn test_one_user_cannot_see_anothers_history() {
    let (db, _tmp) = setup_test_db().await;
    let (series_id, books) = library_with_books(&db, 1).await;
    let state = create_test_auth_state(db.clone()).await;
    let (alice_id, _alice_token) = user_and_token(&db, &state, "alice").await;
    let (_bob_id, bob_token) = user_and_token(&db, &state, "bob").await;

    ReadProgressRepository::mark_as_read(&db, alice_id, books[0].id, 50)
        .await
        .unwrap();

    let (status, history) = get_book_history(
        create_test_router(state.clone()).await,
        &bob_token,
        books[0].id,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        history.unwrap().read_count,
        0,
        "bob must not see alice's completion"
    );

    let (_s, series) =
        get_series_history(create_test_router(state).await, &bob_token, series_id).await;
    assert_eq!(series.unwrap().read_count, 0);
}

#[tokio::test]
async fn test_one_user_cannot_clear_anothers_history() {
    let (db, _tmp) = setup_test_db().await;
    let (series_id, books) = library_with_books(&db, 1).await;
    let state = create_test_auth_state(db.clone()).await;
    let (alice_id, alice_token) = user_and_token(&db, &state, "alice").await;
    let (_bob_id, bob_token) = user_and_token(&db, &state, "bob").await;

    ReadProgressRepository::mark_as_read(&db, alice_id, books[0].id, 50)
        .await
        .unwrap();

    // Bob clears at all three scopes. Each succeeds, but only for his own
    // (empty) history.
    for uri in [
        format!("/api/v1/books/{}/read-history", books[0].id),
        format!("/api/v1/series/{series_id}/read-history"),
        "/api/v1/user/read-history".to_string(),
    ] {
        let app = create_test_router(state.clone()).await;
        let request = delete_request_with_auth(&uri, &bob_token);
        let (status, _) = make_raw_request(app, request).await;
        assert_eq!(status, StatusCode::NO_CONTENT, "clearing {uri}");
    }

    let (_s, history) =
        get_book_history(create_test_router(state).await, &alice_token, books[0].id).await;
    assert_eq!(
        history.unwrap().read_count,
        1,
        "alice's history must survive bob's resets"
    );
}

// ============================================================================
// Detail DTO fields
// ============================================================================

#[tokio::test]
async fn test_book_detail_reports_the_completion_count() {
    use codex::api::routes::v1::dto::FullBookResponse;

    let (db, _tmp) = setup_test_db().await;
    let (_series_id, books) = library_with_books(&db, 1).await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = user_and_token(&db, &state, "reader").await;

    ReadProgressRepository::mark_as_read(&db, user_id, books[0].id, 50)
        .await
        .unwrap();
    ReadProgressRepository::mark_as_unread(&db, user_id, books[0].id)
        .await
        .unwrap();
    ReadProgressRepository::mark_as_read(&db, user_id, books[0].id, 50)
        .await
        .unwrap();

    let app = create_test_router(state).await;
    let request =
        get_request_with_auth(&format!("/api/v1/books/{}?full=true", books[0].id), &token);
    let (status, book): (StatusCode, Option<FullBookResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book = book.unwrap();
    assert_eq!(book.read_count, 2);
    assert!(book.last_completed_at.is_some());
}

// ============================================================================
// Completions arriving through the compatibility layers
// ============================================================================

/// A completion posted through the Komga endpoint must be recorded, since that
/// path funnels through the same upsert.
#[tokio::test]
async fn test_komga_completion_is_recorded() {
    let (db, _tmp) = setup_test_db().await;
    let (_series_id, books) = library_with_books(&db, 1).await;
    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "reader").await;

    let app = create_test_router_with_komga(state.clone());
    let request = patch_request_with_auth_json(
        &format!("/komga/api/v1/books/{}/read-progress", books[0].id),
        &token,
        r#"{"completed":true,"page":50}"#,
    );
    let (status, _) = make_raw_request(app, request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_s, history) =
        get_book_history(create_test_router(state).await, &token, books[0].id).await;
    assert_eq!(
        history.unwrap().read_count,
        1,
        "a Komga-posted completion must be banked"
    );
}

/// The same for KOReader sync, which reports a percentage rather than a flag.
#[tokio::test]
async fn test_koreader_completion_is_recorded() {
    let (db, _tmp) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Lib", "/test", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Series", None)
        .await
        .unwrap();

    let koreader_hash = "abc123def456";
    let mut book = create_test_book(
        series.id,
        library.id,
        "/test/book0.cbz",
        "book0.cbz",
        "hash0",
        "cbz",
        100,
    );
    book.koreader_hash = Some(koreader_hash.to_string());
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    // The KOReader router needs an AppState rather than an AuthState, so the
    // user is created here instead of via the shared helper above.
    let state = create_test_app_state(db.clone()).await;
    let password_hash = password::hash_password("pw123456").unwrap();
    let user = create_test_user("reader", "reader@example.com", &password_hash, true);
    let created = UserRepository::create(&db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();

    // 100% read: the sync handler treats this as completed.
    let progress = serde_json::json!({
        "document": koreader_hash,
        "progress": "100",
        "percentage": 1.0,
        "device": "test-device",
        "device_id": "device-123"
    });

    let app = create_test_router_with_koreader(state.clone());
    let request = put_request_with_auth(
        "/koreader/syncs/progress",
        &serde_json::to_string(&progress).unwrap(),
        &token,
    );
    let (status, _body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let app = create_test_router(state).await;
    let request = get_request_with_auth(&format!("/api/v1/books/{}/read-history", book.id), &token);
    let (status, history): (StatusCode, Option<ReadHistoryResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        history.unwrap().read_count,
        1,
        "a KOReader-synced completion must be banked"
    );
}
