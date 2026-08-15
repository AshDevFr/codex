//! Integration tests for `POST /api/v1/reading-sessions`.
//!
//! The properties that matter here are the ones an offline client depends on:
//! a replayed batch must not double-count, sessions must order by when the
//! reading happened rather than when it arrived, and one bad entry must not
//! cost a client the rest of its queue.

#[path = "../common/mod.rs"]
mod common;

use chrono::{DateTime, Duration, TimeZone, Utc};
use codex::api::routes::v1::dto::{
    ReadProgressResponse, ReadingSessionRejectionReason, RecordReadingSessionsResponse,
};
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    BookRepository, LibraryRepository, ReadProgressRepository, SeriesRepository, UserRepository,
};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use serde_json::json;
use uuid::Uuid;

async fn create_admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> (Uuid, String) {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    (created.id, token)
}

async fn create_book(
    db: &sea_orm::DatabaseConnection,
    path: &str,
) -> codex::db::entities::books::Model {
    let library = LibraryRepository::create(db, "Test Library", "/test", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(db, library.id, "Test Series", None)
        .await
        .unwrap();

    let book = codex::db::entities::books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        library_id: library.id,
        path: path.to_string(),
        file_name: "book.cbz".to_string(),
        file_size: 1024,
        file_hash: format!("hash_{}", Uuid::new_v4()),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 100,
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

fn at(minutes: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 14, 10, 0, 0).unwrap() + Duration::minutes(minutes)
}

/// A session body. Callers override what the test is about.
fn session(
    id: Uuid,
    book_id: Uuid,
    device: &str,
    page: i32,
    from: i64,
    to: i64,
) -> serde_json::Value {
    json!({
        "id": id,
        "bookId": book_id,
        "deviceId": device,
        "kind": "progress",
        "toPage": page,
        "clientStartedAt": at(from),
        "clientEndedAt": at(to),
    })
}

/// The motivating failure: a device that read less but syncs later must not
/// drag the reader backwards.
#[tokio::test]
async fn a_late_syncing_stale_session_does_not_move_progress_backwards() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = create_book(&db, "/test/book.cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_admin_and_token(&db, &state).await;

    // The phone syncs immediately, having read furthest.
    let app = create_test_router(state.clone()).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-sessions",
        &json!({"sessions": [session(Uuid::new_v4(), book.id, "phone", 40, 45, 60)]}),
        &token,
    );
    let (status, _): (StatusCode, Option<RecordReadingSessionsResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // The tablet, which read earlier and less, syncs afterwards.
    let app = create_test_router(state.clone()).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-sessions",
        &json!({"sessions": [session(Uuid::new_v4(), book.id, "tablet", 12, 15, 30)]}),
        &token,
    );
    let (status, response): (StatusCode, Option<RecordReadingSessionsResponse>) =
        make_json_request(app, request).await;
    let response = response.expect("expected a JSON body");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        response.progress[0].current_page, 40,
        "the session that read latest in client time must win, whatever the arrival order"
    );

    let progress = ReadProgressRepository::get_by_user_and_book(&db, user_id, book.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(progress.current_page, 40);
}

/// Replaying a batch whose response was lost must change nothing.
#[tokio::test]
async fn replaying_an_identical_batch_is_a_no_op() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = create_book(&db, "/test/book.cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_admin_and_token(&db, &state).await;

    let id = Uuid::new_v4();
    let body = json!({"sessions": [{
        "id": id,
        "bookId": book.id,
        "deviceId": "phone",
        "kind": "progress",
        "toPage": 30,
        "activeDurationMs": 600_000,
        "pagesRead": 30,
        "clientStartedAt": at(0),
        "clientEndedAt": at(15),
    }]});

    for _ in 0..3 {
        let app = create_test_router(state.clone()).await;
        let request = post_json_request_with_auth("/api/v1/reading-sessions", &body, &token);
        let (status, response): (StatusCode, Option<RecordReadingSessionsResponse>) =
            make_json_request(app, request).await;
        let response = response.expect("expected a JSON body");

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            response.accepted,
            vec![id],
            "a replay reports accepted so the client can clear its outbox"
        );
        assert!(response.rejected.is_empty());
    }

    let sessions =
        codex::db::repositories::ReadingSessionRepository::load_for_book(&db, user_id, book.id)
            .await
            .unwrap();
    assert_eq!(sessions.len(), 1, "three submissions, one session");
    assert_eq!(
        sessions[0].active_duration_ms,
        Some(600_000),
        "the duration must not accumulate across replays"
    );
}

/// A book deleted while the client was offline rejects its own entry and
/// nothing else. Failing the batch would strand the rest of the queue.
#[tokio::test]
async fn an_unknown_book_rejects_only_its_own_entry() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = create_book(&db, "/test/book.cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;

    let good = Uuid::new_v4();
    let orphan = Uuid::new_v4();
    let app = create_test_router(state).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-sessions",
        &json!({"sessions": [
            session(orphan, Uuid::new_v4(), "phone", 5, 0, 10),
            session(good, book.id, "phone", 20, 10, 20),
        ]}),
        &token,
    );
    let (status, response): (StatusCode, Option<RecordReadingSessionsResponse>) =
        make_json_request(app, request).await;
    let response = response.expect("expected a JSON body");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.accepted, vec![good]);
    assert_eq!(response.rejected.len(), 1);
    assert_eq!(response.rejected[0].id, orphan);
    assert_eq!(
        response.rejected[0].reason,
        ReadingSessionRejectionReason::BookNotFound
    );
    assert_eq!(response.progress.len(), 1);
    assert_eq!(response.progress[0].current_page, 20);
}

/// A completion recorded through the sessions API banks a read-through, and a
/// reset followed by another completion banks a second.
#[tokio::test]
async fn completions_and_resets_drive_the_completion_log() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = create_book(&db, "/test/book.cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_admin_and_token(&db, &state).await;

    let app = create_test_router(state.clone()).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-sessions",
        &json!({"sessions": [{
            "id": Uuid::new_v4(),
            "bookId": book.id,
            "deviceId": "phone",
            "kind": "completed",
            "toPage": 100,
            "clientStartedAt": at(0),
            "clientEndedAt": at(30),
        }]}),
        &token,
    );
    let (status, _): (StatusCode, Option<RecordReadingSessionsResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let count =
        codex::db::repositories::ReadCompletionRepository::count_for_book(&db, user_id, book.id)
            .await
            .unwrap();
    assert_eq!(count, 1);

    // Reset, then finish again: a genuine re-read.
    let app = create_test_router(state.clone()).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-sessions",
        &json!({"sessions": [{
            "id": Uuid::new_v4(),
            "bookId": book.id,
            "deviceId": "phone",
            "kind": "reset",
            "clientStartedAt": at(40),
            "clientEndedAt": at(40),
        }]}),
        &token,
    );
    let (status, _): (StatusCode, Option<RecordReadingSessionsResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    assert!(
        ReadProgressRepository::get_by_user_and_book(&db, user_id, book.id)
            .await
            .unwrap()
            .is_none(),
        "a reset with no reading since leaves no progress row"
    );

    let app = create_test_router(state).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-sessions",
        &json!({"sessions": [{
            "id": Uuid::new_v4(),
            "bookId": book.id,
            "deviceId": "phone",
            "kind": "completed",
            "toPage": 100,
            "clientStartedAt": at(50),
            "clientEndedAt": at(70),
        }]}),
        &token,
    );
    let (status, _): (StatusCode, Option<RecordReadingSessionsResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let count =
        codex::db::repositories::ReadCompletionRepository::count_for_book(&db, user_id, book.id)
            .await
            .unwrap();
    assert_eq!(
        count, 2,
        "the reset made the second finish a new read-through"
    );
}

/// Measurements that could not be honest are rejected rather than corrupting
/// the reading statistics.
#[tokio::test]
async fn implausible_measurements_are_rejected() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = create_book(&db, "/test/book.cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;

    let backwards = Uuid::new_v4();
    let bad_percentage = Uuid::new_v4();
    let negative = Uuid::new_v4();

    let app = create_test_router(state).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-sessions",
        &json!({"sessions": [
            {
                "id": backwards, "bookId": book.id, "deviceId": "phone", "kind": "progress",
                "toPage": 10, "clientStartedAt": at(30), "clientEndedAt": at(10),
            },
            {
                "id": bad_percentage, "bookId": book.id, "deviceId": "phone", "kind": "progress",
                "toPercentage": 1.5, "clientStartedAt": at(0), "clientEndedAt": at(10),
            },
            {
                "id": negative, "bookId": book.id, "deviceId": "phone", "kind": "progress",
                "toPage": 10, "activeDurationMs": -5, "clientStartedAt": at(0), "clientEndedAt": at(10),
            },
        ]}),
        &token,
    );
    let (status, response): (StatusCode, Option<RecordReadingSessionsResponse>) =
        make_json_request(app, request).await;
    let response = response.expect("expected a JSON body");

    assert_eq!(status, StatusCode::OK);
    assert!(response.accepted.is_empty());

    let reason_for = |id: Uuid| {
        response
            .rejected
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.reason)
            .unwrap()
    };
    assert_eq!(
        reason_for(backwards),
        ReadingSessionRejectionReason::InvalidTimeRange
    );
    assert_eq!(
        reason_for(bad_percentage),
        ReadingSessionRejectionReason::InvalidPercentage
    );
    assert_eq!(
        reason_for(negative),
        ReadingSessionRejectionReason::InvalidMeasurement
    );
}

/// A client claiming more reading time than the session lasted is truncated,
/// not trusted.
#[tokio::test]
async fn a_duration_beyond_the_session_span_is_clamped() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = create_book(&db, "/test/book.cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_admin_and_token(&db, &state).await;

    let app = create_test_router(state).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-sessions",
        &json!({"sessions": [{
            "id": Uuid::new_v4(),
            "bookId": book.id,
            "deviceId": "phone",
            "kind": "progress",
            "toPage": 10,
            "activeDurationMs": 10 * 60 * 60 * 1000_i64,
            "clientStartedAt": at(0),
            "clientEndedAt": at(5),
        }]}),
        &token,
    );
    let (status, _): (StatusCode, Option<RecordReadingSessionsResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let sessions =
        codex::db::repositories::ReadingSessionRepository::load_for_book(&db, user_id, book.id)
            .await
            .unwrap();
    assert_eq!(sessions[0].active_duration_ms, Some(5 * 60 * 1000));
}

/// Sessions always belong to the caller. There is no field for another user,
/// so a token is the only thing that decides whose reading this is.
#[tokio::test]
async fn sessions_are_recorded_against_the_authenticated_user() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = create_book(&db, "/test/book.cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_admin_and_token(&db, &state).await;

    let password_hash = password::hash_password("other123").unwrap();
    let other = UserRepository::create(
        &db,
        &create_test_user("other", "other@example.com", &password_hash, true),
    )
    .await
    .unwrap();

    let app = create_test_router(state).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-sessions",
        &json!({"sessions": [session(Uuid::new_v4(), book.id, "phone", 20, 0, 10)]}),
        &token,
    );
    let (status, response): (StatusCode, Option<RecordReadingSessionsResponse>) =
        make_json_request(app, request).await;
    let response = response.expect("expected a JSON body");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.progress[0].user_id, user_id);

    assert!(
        ReadProgressRepository::get_by_user_and_book(&db, other.id, book.id)
            .await
            .unwrap()
            .is_none(),
        "another user's reading must be untouched"
    );
}

/// The same id twice in one batch is a client bug, and applying it twice would
/// double-count. The first wins and the second is reported.
#[tokio::test]
async fn a_duplicate_id_within_one_batch_is_rejected() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = create_book(&db, "/test/book.cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;

    let id = Uuid::new_v4();
    let app = create_test_router(state).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-sessions",
        &json!({"sessions": [
            session(id, book.id, "phone", 10, 0, 10),
            session(id, book.id, "phone", 20, 10, 20),
        ]}),
        &token,
    );
    let (status, response): (StatusCode, Option<RecordReadingSessionsResponse>) =
        make_json_request(app, request).await;
    let response = response.expect("expected a JSON body");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.accepted, vec![id]);
    assert_eq!(
        response.rejected[0].reason,
        ReadingSessionRejectionReason::DuplicateInBatch
    );
}

/// An oversized batch is refused outright, so one request cannot hold a
/// transaction open indefinitely. The client chunks instead.
#[tokio::test]
async fn an_oversized_batch_is_refused() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = create_book(&db, "/test/book.cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;

    let sessions: Vec<serde_json::Value> = (0..501)
        .map(|i| session(Uuid::new_v4(), book.id, "phone", i % 100, 0, 10))
        .collect();

    let app = create_test_router(state).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-sessions",
        &json!({ "sessions": sessions }),
        &token,
    );
    let (status, _body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// Recording sessions requires authentication like every other progress write.
#[tokio::test]
async fn recording_sessions_requires_authentication() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = create_book(&db, "/test/book.cbz").await;
    let state = create_test_auth_state(db.clone()).await;

    let app = create_test_router(state).await;
    let request = post_json_request(
        "/api/v1/reading-sessions",
        &json!({"sessions": [session(Uuid::new_v4(), book.id, "phone", 10, 0, 10)]}),
    );
    let (status, _body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// The whole round trip a reading client actually performs: position writes
/// while reading, then one measured session when the sitting ends.
///
/// The position writes keep the stored position live before the session
/// arrives, and the session then absorbs them, so one sitting leaves one row
/// rather than one per page turn plus a phantom device.
#[tokio::test]
async fn a_sitting_of_progress_writes_plus_a_session_leaves_one_row() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = create_book(&db, "/test/book.cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_admin_and_token(&db, &state).await;

    let started = Utc::now() - Duration::minutes(20);

    // Reading: a position write per page turn, each declaring the device.
    for page in 1..=15 {
        let app = create_test_router(state.clone()).await;
        let mut request = put_json_request_with_auth(
            &format!("/api/v1/books/{}/progress", book.id),
            &json!({ "currentPage": page }),
            &token,
        );
        request
            .headers_mut()
            .insert("x-codex-device-id", "browser-abc".parse().unwrap());
        let (status, _body) = make_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }

    // The sitting ends and the reader reports what it measured.
    let app = create_test_router(state).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-sessions",
        &json!({"sessions": [{
            "id": Uuid::new_v4(),
            "bookId": book.id,
            "deviceId": "browser-abc",
            "deviceName": "Codex Web (Mac)",
            "kind": "progress",
            "toPage": 15,
            "activeDurationMs": 900_000,
            "pagesRead": 15,
            "clientStartedAt": started,
            "clientEndedAt": Utc::now(),
        }]}),
        &token,
    );
    let (status, _): (StatusCode, Option<RecordReadingSessionsResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let sessions =
        codex::db::repositories::ReadingSessionRepository::load_for_book(&db, user_id, book.id)
            .await
            .unwrap();

    assert_eq!(
        sessions.len(),
        1,
        "fifteen page turns and one session close are one sitting"
    );
    assert_eq!(sessions[0].device_id, "browser-abc");
    assert_eq!(sessions[0].active_duration_ms, Some(900_000));
    assert!(
        !sessions.iter().any(|s| s.device_id == "legacy"),
        "a client that declares its device must not also appear as the anonymous one"
    );

    let progress = ReadProgressRepository::get_by_user_and_book(&db, user_id, book.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(progress.current_page, 15);
}

/// A client that declares nothing keeps the behaviour it had before, so
/// third-party callers of the native API are unaffected.
#[tokio::test]
async fn a_client_without_a_device_header_still_records_progress() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = create_book(&db, "/test/book.cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_admin_and_token(&db, &state).await;

    let app = create_test_router(state).await;
    let request = put_json_request_with_auth(
        &format!("/api/v1/books/{}/progress", book.id),
        &json!({ "currentPage": 7 }),
        &token,
    );
    let (status, _body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let sessions =
        codex::db::repositories::ReadingSessionRepository::load_for_book(&db, user_id, book.id)
            .await
            .unwrap();
    assert_eq!(sessions[0].device_id, "legacy");

    let progress = ReadProgressRepository::get_by_user_and_book(&db, user_id, book.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(progress.current_page, 7);
}

/// Several books in one batch each get their progress back, so a client can
/// reconcile a whole sync in one round trip.
#[tokio::test]
async fn a_batch_spanning_books_returns_progress_for_each() {
    let (db, _temp_dir) = setup_test_db().await;
    let first = create_book(&db, "/test/first.cbz").await;
    let second = create_book(&db, "/test/second.cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;

    let app = create_test_router(state).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-sessions",
        &json!({"sessions": [
            session(Uuid::new_v4(), first.id, "phone", 10, 0, 10),
            session(Uuid::new_v4(), second.id, "phone", 25, 10, 20),
            session(Uuid::new_v4(), first.id, "phone", 40, 20, 30),
        ]}),
        &token,
    );
    let (status, response): (StatusCode, Option<RecordReadingSessionsResponse>) =
        make_json_request(app, request).await;
    let response = response.expect("expected a JSON body");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.accepted.len(), 3);
    assert_eq!(response.progress.len(), 2);

    let page_for = |book_id: Uuid| -> i32 {
        response
            .progress
            .iter()
            .find(|p: &&ReadProgressResponse| p.book_id == book_id)
            .unwrap()
            .current_page
    };
    assert_eq!(
        page_for(first.id),
        40,
        "the later session for this book wins"
    );
    assert_eq!(page_for(second.id), 25);
}
