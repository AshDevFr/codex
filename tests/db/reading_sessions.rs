//! How a reading client's writes land in the session log, on both engines.
//!
//! Written after production showed the original supersede never fired. The
//! crate-level tests seeded every position write *before* the measured session,
//! which is the tidy order and not the one a browser produces: closing a reader
//! emits the measured session and a final position save, and the position save
//! frequently arrives second.
//!
//! These reproduce the real arrival order. They also run against PostgreSQL,
//! because the crate-level tests only ever see SQLite.

#[path = "../common/mod.rs"]
mod common;

use chrono::{Duration, Utc};
use codex::db::ScanningStrategy;
use codex::db::entities::reading_sessions;
use codex::db::entities::reading_sessions::SessionKind;
use codex::db::repositories::{
    BookRepository, DeviceContext, LibraryRepository, NewSession, ReadProgressRepository,
    ReadingSessionRepository, SeriesRepository, UserRepository,
};
use common::*;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

const DEVICE: &str = "browser-1";

fn unique() -> String {
    Uuid::new_v4().to_string()
}

async fn persist_user(db: &DatabaseConnection) -> Uuid {
    let handle = format!("reader-{}", unique());
    let model = create_test_user(&handle, &format!("{handle}@test.test"), "hash", true);
    UserRepository::create(db, &model).await.unwrap().id
}

async fn persist_book(db: &DatabaseConnection) -> Uuid {
    let library = LibraryRepository::create(
        db,
        "Lib",
        &format!("/lib/{}", unique()),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let series = SeriesRepository::create(db, library.id, "Series", None)
        .await
        .unwrap();
    let book = create_test_book(
        series.id,
        library.id,
        &format!("/lib/{}.cbz", unique()),
        "book",
        &format!("hash_{}", unique()),
        "cbz",
        200,
    );
    BookRepository::create(db, &book, None).await.unwrap().id
}

/// A measured session, as a reading client posts one when a sitting ends.
#[allow(clippy::too_many_arguments)]
async fn post_measured(
    db: &DatabaseConnection,
    user: Uuid,
    book: Uuid,
    device: &str,
    kind: SessionKind,
    page: i32,
    minutes_ago_start: i64,
    minutes_ago_end: i64,
) {
    let now = Utc::now();
    let session = NewSession::from_client(
        Uuid::new_v4(),
        user,
        book,
        device,
        Some("Codex Web".to_string()),
        kind,
        Some(60_000),
        Some(10),
        now - Duration::minutes(minutes_ago_start),
        now - Duration::minutes(minutes_ago_end),
    )
    .with_page(page);

    ReadProgressRepository::record_session(db, session)
        .await
        .unwrap();
}

/// A position save, as the reader emits while reading and again on close.
async fn save_position(db: &DatabaseConnection, user: Uuid, book: Uuid, page: i32, done: bool) {
    ReadProgressRepository::upsert_with_device(
        db,
        user,
        book,
        page,
        done,
        &DeviceContext::session_reporting_client(DEVICE),
    )
    .await
    .unwrap();
}

async fn sessions(db: &DatabaseConnection, user: Uuid, book: Uuid) -> Vec<reading_sessions::Model> {
    ReadingSessionRepository::load_for_book(db, user, book)
        .await
        .unwrap()
}

/// The exact shape production produced: a measured session, then a trailing
/// position save carrying the completion.
///
/// Before the fix this left two rows, the second carrying no time and never
/// absorbed by anything, because the forward sweep only looks at what already
/// exists when the measured session arrives.
async fn exercise_trailing_position_save(db: &DatabaseConnection) {
    let user = persist_user(db).await;
    let book = persist_book(db).await;

    // The sitting ends: the client posts what it measured...
    post_measured(db, user, book, DEVICE, SessionKind::Progress, 160, 20, 1).await;
    // ...and the reader saves its final position a moment later, having
    // reached the end.
    save_position(db, user, book, 200, true).await;

    let rows = sessions(db, user, book).await;
    assert_eq!(
        rows.len(),
        1,
        "a trailing position save belongs to the session it follows"
    );
    assert_eq!(
        rows[0].active_duration_ms,
        Some(60_000),
        "the surviving row keeps its measured time"
    );
    assert_eq!(rows[0].to_page, Some(200), "and the later position");
    assert_eq!(
        rows[0].session_kind(),
        SessionKind::Completed,
        "dropping the completion would leave the book reading as unfinished"
    );

    let progress = ReadProgressRepository::get_by_user_and_book(db, user, book)
        .await
        .unwrap()
        .expect("progress survives the merge");
    assert!(progress.completed);
    assert_eq!(progress.current_page, 200);
}

/// Position saves made *before* the measured session are absorbed too, which is
/// the direction the original sweep already handled. Both orders now converge.
async fn exercise_leading_position_saves(db: &DatabaseConnection) {
    let user = persist_user(db).await;
    let book = persist_book(db).await;

    for page in [10, 20, 30] {
        save_position(db, user, book, page, false).await;
    }
    post_measured(db, user, book, DEVICE, SessionKind::Progress, 30, 20, 0).await;

    let rows = sessions(db, user, book).await;
    assert_eq!(rows.len(), 1, "one sitting, one row, whichever order");
    assert_eq!(rows[0].active_duration_ms, Some(60_000));
}

/// Another device's writes are never absorbed, however close in time.
async fn exercise_other_device_is_untouched(db: &DatabaseConnection) {
    let user = persist_user(db).await;
    let book = persist_book(db).await;

    post_measured(db, user, book, DEVICE, SessionKind::Progress, 40, 20, 1).await;
    ReadProgressRepository::upsert_with_device(
        db,
        user,
        book,
        55,
        false,
        &DeviceContext::session_reporting_client("other-device"),
    )
    .await
    .unwrap();

    let rows = sessions(db, user, book).await;
    assert_eq!(rows.len(), 2, "each device keeps its own sitting");
    assert!(rows.iter().any(|r| r.device_id == "other-device"));
}

/// A position save long after the sitting is its own reading, not a straggler.
async fn exercise_stale_position_save_is_not_absorbed(db: &DatabaseConnection) {
    let user = persist_user(db).await;
    let book = persist_book(db).await;

    // The measured session ended half an hour ago, well outside the window.
    post_measured(db, user, book, DEVICE, SessionKind::Progress, 40, 40, 30).await;
    save_position(db, user, book, 90, false).await;

    let rows = sessions(db, user, book).await;
    assert_eq!(
        rows.len(),
        2,
        "reading resumed later is a separate sitting, not a trailing save"
    );
}

/// A device that reports no measured sessions at all behaves exactly as it did
/// before: its writes coalesce among themselves and nothing else changes.
async fn exercise_compat_client_unaffected(db: &DatabaseConnection) {
    let user = persist_user(db).await;
    let book = persist_book(db).await;

    let komic = DeviceContext::user_agent("Komic/1.0");
    for page in 1..=5 {
        ReadProgressRepository::upsert_with_device(db, user, book, page, false, &komic)
            .await
            .unwrap();
    }

    let rows = sessions(db, user, book).await;
    assert_eq!(
        rows.len(),
        1,
        "page turns still coalesce for a compat client"
    );
    assert_eq!(
        rows[0].duration_source(),
        codex::db::entities::reading_sessions::DurationSource::Inferred,
        "and its time is still reconstructed"
    );
}

#[tokio::test]
async fn trailing_position_save_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_trailing_position_save(&db).await;
}

#[tokio::test]
async fn leading_position_saves_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_leading_position_saves(&db).await;
}

#[tokio::test]
async fn other_device_is_untouched_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_other_device_is_untouched(&db).await;
}

#[tokio::test]
async fn stale_position_save_is_not_absorbed_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_stale_position_save_is_not_absorbed(&db).await;
}

#[tokio::test]
async fn compat_client_unaffected_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_compat_client_unaffected(&db).await;
}

/// All of the above against PostgreSQL, sequenced in one test on purpose:
/// `setup_test_db_postgres` truncates a database shared by the whole run, so
/// two PostgreSQL tests running at once delete each other's fixtures.
#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn reading_sessions_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        eprintln!("PostgreSQL test database not available, skipping");
        return;
    };

    exercise_trailing_position_save(&db).await;
    exercise_leading_position_saves(&db).await;
    exercise_other_device_is_untouched(&db).await;
    exercise_stale_position_save_is_not_absorbed(&db).await;
    exercise_compat_client_unaffected(&db).await;
}
