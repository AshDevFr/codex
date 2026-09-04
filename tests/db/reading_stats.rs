//! Reading statistics aggregations, on both engines.
//!
//! These exist because the SQLite-only unit tests missed two PostgreSQL-only
//! failures that only appeared in production:
//!
//! * `SUM` over a `bigint` column widens to `numeric` on PostgreSQL, which does
//!   not decode into an `i64`. SQLite returns an integer and never noticed.
//! * Date bucketing uses `date_trunc`/`to_char` on PostgreSQL and
//!   `strftime`/`date` on SQLite. Those are entirely separate expressions, and
//!   only one of them was ever executed.
//!
//! Anything touching aggregate types or date functions belongs here rather than
//! in a crate-level unit test, because a crate-level test only ever sees SQLite.

#[path = "../common/mod.rs"]
mod common;

use chrono::{DateTime, Duration, TimeZone, Utc};
use codex::db::ScanningStrategy;
use codex::db::entities::reading_sessions;
use codex::db::repositories::{
    BookRepository, LibraryRepository, ReadingStatsRepository, SeriesRepository, StatsGranularity,
    StatsSort, StatsWindow, UserRepository,
};
use common::*;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use uuid::Uuid;

const MINUTE_MS: i64 = 60_000;

fn at(day: u32, hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, day, hour, 0, 0).unwrap()
}

fn june() -> StatsWindow {
    StatsWindow {
        from: at(1, 0),
        to: at(30, 23),
    }
}

/// Fixtures carry a unique suffix because the PostgreSQL test database is
/// shared by every test in the run, and they execute in parallel. Fixed names
/// collide on the unique indexes and the failure looks like a logic bug rather
/// than what it is.
fn unique() -> String {
    Uuid::new_v4().to_string()
}

async fn persist_user(db: &DatabaseConnection, username: &str) -> Uuid {
    let handle = format!("{username}-{}", unique());
    let model = create_test_user(&handle, &format!("{handle}@test.test"), "hash", true);
    UserRepository::create(db, &model).await.unwrap().id
}

async fn persist_book(db: &DatabaseConnection, series_name: &str, format: &str) -> Uuid {
    let library = LibraryRepository::create(
        db,
        "Lib",
        &format!("/lib/{}", unique()),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let series = SeriesRepository::create(db, library.id, series_name, None)
        .await
        .unwrap();
    let book = create_test_book(
        series.id,
        library.id,
        &format!("/lib/{}.{format}", Uuid::new_v4()),
        "book",
        &format!("hash_{}", Uuid::new_v4()),
        format,
        100,
    );
    BookRepository::create(db, &book, None).await.unwrap().id
}

#[allow(clippy::too_many_arguments)]
async fn seed(
    db: &DatabaseConnection,
    user_id: Uuid,
    book_id: Uuid,
    device: &str,
    kind: &str,
    source: &str,
    minutes: i64,
    pages: i32,
    started: DateTime<Utc>,
) {
    reading_sessions::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user_id),
        book_id: Set(book_id),
        device_id: Set(device.to_string()),
        device_name: Set(Some(device.to_string())),
        pass: Set(1),
        kind: Set(kind.to_string()),
        to_page: Set(Some(10)),
        to_percentage: Set(None),
        r2_progression: Set(None),
        active_duration_ms: Set(if source == "unknown" {
            None
        } else {
            Some(minutes * MINUTE_MS)
        }),
        duration_source: Set(source.to_string()),
        pages_read: Set(Some(pages)),
        client_started_at: Set(started),
        client_ended_at: Set(started + Duration::minutes(minutes.max(1))),
        server_recorded_at: Set(started + Duration::minutes(minutes.max(1))),
    }
    .insert(db)
    .await
    .unwrap();
}

/// Every aggregation, on whichever engine is handed in.
async fn exercise_reading_stats(db: &DatabaseConnection) {
    let user = persist_user(db, "reader").await;
    let comic = persist_book(db, "Berserk", "cbz").await;
    let ebook = persist_book(db, "Dune", "epub").await;

    // A measured sitting, a reconstructed one, and one that reported no time,
    // so every branch of the provenance split is exercised.
    seed(
        db,
        user,
        comic,
        "phone",
        "progress",
        "measured",
        30,
        20,
        at(3, 9),
    )
    .await;
    seed(
        db,
        user,
        comic,
        "komic",
        "progress",
        "inferred",
        20,
        10,
        at(3, 14),
    )
    .await;
    seed(
        db,
        user,
        comic,
        "opds",
        "progress",
        "unknown",
        0,
        5,
        at(3, 18),
    )
    .await;
    seed(
        db,
        user,
        ebook,
        "phone",
        "completed",
        "measured",
        45,
        30,
        at(25, 9),
    )
    .await;

    // The ebook's finish reported a second time by another surface. Finishes
    // are counted as distinct (book, pass), and the composite expression
    // behind that leans on uuid-to-text casting that each engine spells the
    // same but executes differently, so the collapsed value is asserted here
    // on both.
    seed(
        db,
        user,
        ebook,
        "opds",
        "completed",
        "unknown",
        0,
        0,
        at(25, 9),
    )
    .await;

    // A reset is bookkeeping, not reading, and must not inflate the counts.
    seed(
        db,
        user,
        comic,
        "phone",
        "reset",
        "unknown",
        0,
        0,
        at(26, 9),
    )
    .await;

    // ---- Summary: the numeric-vs-bigint decode lives here ----
    let summary = ReadingStatsRepository::summary(db, user, june())
        .await
        .expect("summary must decode on this engine");

    assert_eq!(summary.duration.measured_ms, 75 * MINUTE_MS);
    assert_eq!(summary.duration.inferred_ms, 20 * MINUTE_MS);
    assert_eq!(summary.duration.total_ms(), 95 * MINUTE_MS);
    assert_eq!(summary.pages_read, 65);
    assert_eq!(summary.sessions, 5, "the reset is not a sitting");
    assert_eq!(summary.books, 2);
    assert_eq!(
        summary.books_finished, 1,
        "two reports of one finish collapse to one"
    );
    assert_eq!(summary.sessions_without_duration, 2);

    // ---- Bucketing: entirely different SQL per engine ----
    let daily = ReadingStatsRepository::by_period(db, user, june(), StatsGranularity::Day, 0)
        .await
        .expect("daily buckets must decode on this engine");
    assert_eq!(daily.len(), 2, "two calendar days of reading");
    assert_eq!(daily[0].bucket, "2026-06-03");
    assert_eq!(daily[0].duration.total_ms(), 50 * MINUTE_MS);
    assert_eq!(daily[1].bucket, "2026-06-25");

    let monthly = ReadingStatsRepository::by_period(db, user, june(), StatsGranularity::Month, 0)
        .await
        .expect("monthly buckets must decode on this engine");
    assert_eq!(monthly.len(), 1);
    assert_eq!(
        monthly[0].bucket, "2026-06-01",
        "a month bucket is keyed on the first of the month, on both engines"
    );

    // 2026-06-03 is a Wednesday, so its week starts Monday 2026-06-01.
    let weekly = ReadingStatsRepository::by_period(db, user, june(), StatsGranularity::Week, 0)
        .await
        .expect("weekly buckets must decode on this engine");
    assert_eq!(
        weekly[0].bucket, "2026-06-01",
        "a week bucket is keyed on its Monday, on both engines"
    );

    // ---- Viewer offset: the shifted bucket SQL differs per engine ----
    // 09:00 UTC is 23:00 the previous evening at UTC-10, so both 09:00
    // sittings slide back a day while the afternoon ones stay put.
    let shifted = ReadingStatsRepository::by_period(db, user, june(), StatsGranularity::Day, -600)
        .await
        .expect("offset buckets must decode on this engine");
    assert_eq!(shifted.len(), 3);
    assert_eq!(shifted[0].bucket, "2026-06-02");
    assert_eq!(shifted[1].bucket, "2026-06-03");
    assert_eq!(shifted[2].bucket, "2026-06-24");

    // ---- Breakdowns: each carries its own aggregate expressions ----
    let devices = ReadingStatsRepository::by_device(db, user, june(), StatsSort::Time)
        .await
        .expect("device breakdown must decode on this engine");
    assert_eq!(devices.len(), 3);
    assert_eq!(devices[0].device_id, "phone", "ranked by time read");
    assert_eq!(devices[0].duration.measured_ms, 75 * MINUTE_MS);

    let series = ReadingStatsRepository::by_series(db, user, june(), StatsSort::Time, 10)
        .await
        .expect("series breakdown must decode on this engine");
    assert_eq!(series.len(), 2);
    assert_eq!(series[0].series_name, "Berserk");
    assert_eq!(series[0].duration.total_ms(), 50 * MINUTE_MS);
    assert_eq!(series[1].series_name, "Dune");
    assert_eq!(
        series[1].books_finished, 1,
        "the duplicated finish collapses in the series breakdown too"
    );

    let formats = ReadingStatsRepository::by_format(db, user, june(), StatsSort::Time)
        .await
        .expect("format breakdown must decode on this engine");
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].format, "cbz");

    // Each ranking key is a different `ORDER BY` over aggregate expressions,
    // and PostgreSQL is the strict one about what may appear there. Ranking by
    // time alone would leave two of the three statements never executed
    // against the engine that runs in production.
    for sort in [StatsSort::Time, StatsSort::Pages, StatsSort::Completions] {
        ReadingStatsRepository::by_series(db, user, june(), sort, 10)
            .await
            .unwrap_or_else(|e| panic!("series ranked by {sort:?} must run: {e}"));
        ReadingStatsRepository::by_device(db, user, june(), sort)
            .await
            .unwrap_or_else(|e| panic!("devices ranked by {sort:?} must run: {e}"));
        ReadingStatsRepository::by_format(db, user, june(), sort)
            .await
            .unwrap_or_else(|e| panic!("formats ranked by {sort:?} must run: {e}"));
    }

    // ---- Row count, used for the retention question ----
    // Decoding a nullable MIN/MAX over a timestamp column is engine-specific
    // enough to be worth running on both, like the sums above it.
    let coverage = ReadingStatsRepository::coverage(db, user)
        .await
        .expect("coverage must decode on this engine");
    assert!(coverage.first_read_at.is_some());
    assert!(coverage.last_read_at >= coverage.first_read_at);

    let rows = ReadingStatsRepository::row_count(db, user).await.unwrap();
    assert_eq!(rows, 6, "the reset is still a row, just not a sitting");
}

/// One user's statistics must never include another's, on either engine.
async fn exercise_user_isolation(db: &DatabaseConnection) {
    let alice = persist_user(db, "alice").await;
    let bob = persist_user(db, "bob").await;
    let book = persist_book(db, "Berserk", "cbz").await;

    seed(
        db,
        alice,
        book,
        "phone",
        "progress",
        "measured",
        60,
        40,
        at(3, 9),
    )
    .await;

    let bobs = ReadingStatsRepository::summary(db, bob, june())
        .await
        .unwrap();
    assert_eq!(bobs.duration.total_ms(), 0);
    assert_eq!(bobs.sessions, 0);

    let alices = ReadingStatsRepository::summary(db, alice, june())
        .await
        .unwrap();
    assert_eq!(alices.duration.measured_ms, 60 * MINUTE_MS);
}

/// An empty log must answer zeroes rather than failing to decode a NULL sum.
async fn exercise_empty_log(db: &DatabaseConnection) {
    let user = persist_user(db, "newcomer").await;

    let summary = ReadingStatsRepository::summary(db, user, june())
        .await
        .expect("an empty aggregate must still decode");

    assert_eq!(summary.duration.total_ms(), 0);
    assert_eq!(summary.sessions, 0);
    assert!(
        ReadingStatsRepository::by_period(db, user, june(), StatsGranularity::Day, 0)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn reading_stats_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_reading_stats(&db).await;
}

#[tokio::test]
async fn reading_stats_user_isolation_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_user_isolation(&db).await;
}

#[tokio::test]
async fn reading_stats_empty_log_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_empty_log(&db).await;
}

/// Every exercise against PostgreSQL, in one test on purpose.
///
/// `setup_test_db_postgres` truncates a database shared by the whole run, so
/// two PostgreSQL tests executing at once delete each other's fixtures. The
/// failure surfaces as a foreign-key violation on a user that existed a moment
/// earlier, which reads like a logic bug and is not one. Sequencing them inside
/// a single test sidesteps the shared-database assumption rather than fighting
/// the harness.
///
/// The SQLite tests above stay separate: each gets its own fresh database and
/// can run in parallel safely.
#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn reading_stats_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        eprintln!("PostgreSQL test database not available, skipping");
        return;
    };

    exercise_reading_stats(&db).await;
    exercise_user_isolation(&db).await;
    exercise_empty_log(&db).await;
}
