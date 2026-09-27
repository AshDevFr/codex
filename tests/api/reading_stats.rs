//! Integration tests for `GET /api/v1/reading-stats`.

#[path = "../common/mod.rs"]
mod common;

use chrono::{DateTime, Duration, TimeZone, Utc};
use codex::api::routes::v1::dto::{PurgedOrphanedHistoryDto, ReadingStatsResponse};
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    BookRepository, LibraryRepository, SeriesRepository, UserRepository,
};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use uuid::Uuid;

const MINUTE_MS: i64 = 60_000;

async fn admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
    name: &str,
) -> (Uuid, String) {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user(name, &format!("{name}@example.com"), &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    (created.id, token)
}

async fn book_in_series(
    db: &sea_orm::DatabaseConnection,
    series_name: &str,
    format: &str,
) -> codex::db::entities::books::Model {
    let library = LibraryRepository::create(db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(db, library.id, series_name, None)
        .await
        .unwrap();
    let book = codex::db::entities::books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        library_id: library.id,
        path: format!("/lib/{}.{}", Uuid::new_v4(), format),
        file_name: format!("book.{format}"),
        file_size: 1024,
        file_hash: format!("hash_{}", Uuid::new_v4()),
        partial_hash: String::new(),
        format: format.to_string(),
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

fn at(day: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, day, 9, 0, 0).unwrap()
}

/// A timestamp safe to drop straight into a query string.
///
/// `to_rfc3339` emits a `+00:00` offset, and a bare `+` in a query string
/// decodes as a space, so the server never sees a parseable timestamp. The
/// `Z` form sidesteps it, which is also what a real client should send.
fn q(when: DateTime<Utc>) -> String {
    when.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Record a session the way a client would, through the public endpoint,
/// so these tests exercise the contract rather than the storage layer.
async fn record_session(
    state: std::sync::Arc<codex::api::extractors::AuthState>,
    token: &str,
    book_id: Uuid,
    device: &str,
    minutes: i64,
    started: DateTime<Utc>,
) {
    record_reading(
        state, token, book_id, device, minutes, 10, "progress", started,
    )
    .await;
}

/// A session with the page count and kind spelled out, for the cases where
/// time, pages and completions have to disagree with each other.
#[allow(clippy::too_many_arguments)]
async fn record_reading(
    state: std::sync::Arc<codex::api::extractors::AuthState>,
    token: &str,
    book_id: Uuid,
    device: &str,
    minutes: i64,
    pages: i64,
    kind: &str,
    started: DateTime<Utc>,
) {
    let app = create_test_router(state).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-sessions",
        &serde_json::json!({"sessions": [{
            "id": Uuid::new_v4(),
            "bookId": book_id,
            "deviceId": device,
            "deviceName": "Test Device",
            "kind": kind,
            "toPage": 30,
            "activeDurationMs": minutes * MINUTE_MS,
            "pagesRead": pages,
            "clientStartedAt": started,
            "clientEndedAt": started + Duration::minutes(minutes),
        }]}),
        token,
    );
    let (status, _body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
}

async fn fetch_stats(
    state: std::sync::Arc<codex::api::extractors::AuthState>,
    token: &str,
    query: &str,
) -> (StatusCode, Option<ReadingStatsResponse>) {
    let app = create_test_router(state).await;
    let request = get_request_with_auth(&format!("/api/v1/reading-stats{query}"), token);
    make_json_request(app, request).await
}

/// The end-to-end path: reading recorded through the API comes back out as
/// statistics over the same numbers.
#[tokio::test]
async fn recorded_reading_shows_up_in_the_statistics() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = book_in_series(&db, "Berserk", "cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;

    record_session(state.clone(), &token, book.id, "phone", 30, at(3)).await;
    record_session(state.clone(), &token, book.id, "phone", 45, at(4)).await;

    let window = format!("?from={}&to={}", q(at(1)), q(at(30)));
    let (status, response) = fetch_stats(state, &token, &window).await;
    let stats = response.expect("expected a JSON body");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(stats.summary.duration.measured_ms, 75 * MINUTE_MS);
    assert_eq!(stats.summary.duration.total_ms, 75 * MINUTE_MS);
    assert_eq!(stats.summary.duration.inferred_ms, 0);
    assert_eq!(stats.summary.sessions, 2);
    assert_eq!(stats.summary.books, 1);
    assert_eq!(stats.summary.pages_read, 20);
}

/// Every panel is present in one response, so no two can disagree about the
/// window they cover.
#[tokio::test]
async fn one_response_carries_every_breakdown() {
    let (db, _temp_dir) = setup_test_db().await;
    let comic = book_in_series(&db, "Berserk", "cbz").await;
    let ebook = book_in_series(&db, "Dune", "epub").await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;

    record_session(state.clone(), &token, comic.id, "phone", 60, at(3)).await;
    record_session(state.clone(), &token, ebook.id, "laptop", 20, at(4)).await;

    let window = format!("?from={}&to={}", q(at(1)), q(at(30)));
    let (status, response) = fetch_stats(state, &token, &window).await;
    let stats = response.expect("expected a JSON body");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(stats.periods.len(), 2, "two days of reading");
    assert_eq!(stats.devices.len(), 2);
    assert_eq!(stats.series.len(), 2);
    assert_eq!(stats.formats.len(), 2);

    assert_eq!(stats.devices[0].device_id, "phone", "ranked by time read");
    assert_eq!(stats.series[0].series_name.as_deref(), Some("Berserk"));
    assert_eq!(stats.formats[0].format.as_deref(), Some("cbz"));
}

/// A reader with no history gets an empty dashboard rather than an error.
#[tokio::test]
async fn a_reader_with_no_history_gets_zeroes() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;

    let (status, response) = fetch_stats(state, &token, "").await;
    let stats = response.expect("expected a JSON body");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(stats.summary.duration.total_ms, 0);
    assert_eq!(stats.summary.sessions, 0);
    assert!(stats.periods.is_empty());
    assert!(stats.devices.is_empty());
}

/// Statistics are personal. One user's reading must never appear in another's.
#[tokio::test]
async fn statistics_never_leak_between_users() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = book_in_series(&db, "Berserk", "cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (_alice, alice_token) = admin_and_token(&db, &state, "alice").await;
    let (_bob, bob_token) = admin_and_token(&db, &state, "bob").await;

    record_session(state.clone(), &alice_token, book.id, "phone", 90, at(3)).await;

    let window = format!("?from={}&to={}", q(at(1)), q(at(30)));
    let (status, response) = fetch_stats(state, &bob_token, &window).await;
    let stats = response.expect("expected a JSON body");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        stats.summary.duration.total_ms, 0,
        "bob has read nothing, whatever alice did"
    );
}

/// The window bounds what is counted.
#[tokio::test]
async fn the_window_bounds_what_is_counted() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = book_in_series(&db, "Berserk", "cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;

    record_session(state.clone(), &token, book.id, "phone", 30, at(3)).await;
    record_session(state.clone(), &token, book.id, "phone", 30, at(20)).await;

    let window = format!("?from={}&to={}", q(at(1)), q(at(10)));
    let (status, response) = fetch_stats(state, &token, &window).await;
    let stats = response.expect("expected a JSON body");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(stats.summary.sessions, 1);
}

/// Granularity changes the shape of the series, not the totals.
#[tokio::test]
async fn granularity_regroups_without_changing_totals() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = book_in_series(&db, "Berserk", "cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;

    record_session(state.clone(), &token, book.id, "phone", 30, at(3)).await;
    record_session(state.clone(), &token, book.id, "phone", 30, at(25)).await;

    let window = format!("from={}&to={}", q(at(1)), q(at(30)));

    let (_, daily) =
        fetch_stats(state.clone(), &token, &format!("?{window}&granularity=day")).await;
    let daily = daily.expect("expected a JSON body");
    let (_, monthly) = fetch_stats(state, &token, &format!("?{window}&granularity=month")).await;
    let monthly = monthly.expect("expected a JSON body");

    assert_eq!(daily.periods.len(), 2);
    assert_eq!(monthly.periods.len(), 1, "one calendar month");
    assert_eq!(
        daily.summary.duration.total_ms,
        monthly.summary.duration.total_ms
    );
}

/// A backwards window is a client bug, not an empty result: saying so is more
/// useful than returning zeroes that look like "you have not read anything".
#[tokio::test]
async fn a_backwards_window_is_rejected() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;

    let query = format!("?from={}&to={}", q(at(20)), q(at(1)));
    let app = create_test_router(state).await;
    let request = get_request_with_auth(&format!("/api/v1/reading-stats{query}"), &token);
    let (status, _body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// The series list is capped so one request cannot rank an entire library.
#[tokio::test]
async fn the_series_limit_is_applied_and_capped() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;

    for name in ["A", "B", "C"] {
        let book = book_in_series(&db, name, "cbz").await;
        record_session(state.clone(), &token, book.id, "phone", 10, at(3)).await;
    }

    let window = format!("from={}&to={}", q(at(1)), q(at(30)));

    let (_, limited) =
        fetch_stats(state.clone(), &token, &format!("?{window}&seriesLimit=2")).await;
    assert_eq!(limited.expect("expected a JSON body").series.len(), 2);

    // Far above the cap: clamped rather than refused, since asking for "all of
    // them" is a reasonable thing for a client to do naively.
    let (status, huge) = fetch_stats(state, &token, &format!("?{window}&seriesLimit=100000")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(huge.expect("expected a JSON body").series.len(), 3);
}

/// The ranking key is applied before the limit, so it decides which series come
/// back at all rather than merely their order. A client cannot reproduce this by
/// re-sorting the response.
#[tokio::test]
async fn the_sort_key_decides_which_series_survive_the_limit() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;

    let long_sitting = book_in_series(&db, "Long Sitting", "cbz").await;
    let quick_pages = book_in_series(&db, "Quick Pages", "cbz").await;
    record_reading(
        state.clone(),
        &token,
        long_sitting.id,
        "phone",
        120,
        5,
        "progress",
        at(3),
    )
    .await;
    record_reading(
        state.clone(),
        &token,
        quick_pages.id,
        "phone",
        10,
        400,
        "completed",
        at(4),
    )
    .await;

    let window = format!("from={}&to={}", q(at(1)), q(at(30)));
    let top = |query: String| {
        let state = state.clone();
        let token = token.clone();
        async move {
            let (_, body) = fetch_stats(state, &token, &query).await;
            body.expect("expected a JSON body").series[0]
                .series_name
                .clone()
                .unwrap_or_default()
        }
    };

    assert_eq!(
        top(format!("?{window}&seriesLimit=1")).await,
        "Long Sitting"
    );
    assert_eq!(
        top(format!("?{window}&seriesLimit=1&sort=pages")).await,
        "Quick Pages"
    );
    assert_eq!(
        top(format!("?{window}&seriesLimit=1&sort=completions")).await,
        "Quick Pages"
    );
}

/// Books finished is the one measure a backfilled library can answer, so it has
/// to reach the client on every breakdown rather than only the summary.
#[tokio::test]
async fn finished_books_are_reported_everywhere() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;

    let book = book_in_series(&db, "Berserk", "cbz").await;
    record_reading(
        state.clone(),
        &token,
        book.id,
        "phone",
        20,
        10,
        "completed",
        at(3),
    )
    .await;

    let window = format!("from={}&to={}", q(at(1)), q(at(30)));
    let (_, body) = fetch_stats(state, &token, &format!("?{window}")).await;
    let stats = body.expect("expected a JSON body");

    assert_eq!(stats.summary.books_finished, 1);
    assert_eq!(stats.periods[0].books_finished, 1);
    assert_eq!(stats.series[0].books_finished, 1);
    assert_eq!(stats.devices[0].books_finished, 1);
    assert_eq!(stats.formats[0].books_finished, 1);
}

/// One finish, however many surfaces reported it. A client that measures its
/// own sessions can also write a completing progress update that arrives
/// unattributed (no device header); the two events describe the same
/// read-through and must count as one finish, not two.
#[tokio::test]
async fn a_finish_reported_by_several_surfaces_counts_once() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;

    let book = book_in_series(&db, "Berserk", "cbz").await;
    record_reading(
        state.clone(),
        &token,
        book.id,
        "phone",
        20,
        10,
        "completed",
        at(3),
    )
    .await;

    // The same finish again, as an unattributed progress write reaching the
    // last page (no x-codex-device-id header, so it lands on the catch-all).
    let app = create_test_router(state.clone()).await;
    let request = put_json_request_with_auth(
        &format!("/api/v1/books/{}/progress", book.id),
        &serde_json::json!({"currentPage": 100, "completed": true}),
        &token,
    );
    let (status, _body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let window = format!(
        "?from={}&to={}",
        q(at(1)),
        q(Utc::now() + Duration::days(1))
    );
    let (_, body) = fetch_stats(state, &token, &window).await;
    let stats = body.expect("expected a JSON body");

    assert_eq!(
        stats.summary.books_finished, 1,
        "two reports of one finish must not count twice"
    );
    assert_eq!(stats.series[0].books_finished, 1);
}

/// Marking a series read is attributed to the device the request declares,
/// and re-marking it changes nothing: the books are already finished.
#[tokio::test]
async fn marking_a_series_read_is_attributed_and_idempotent() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;
    let book = book_in_series(&db, "Berserk", "cbz").await;

    let mark = || async {
        let app = create_test_router(state.clone()).await;
        let mut request =
            post_request_with_auth(&format!("/api/v1/series/{}/read", book.series_id), &token);
        request
            .headers_mut()
            .insert("x-codex-device-id", "browser-abc".parse().unwrap());
        let (status, _body) = make_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    };
    mark().await;
    mark().await;

    let window = format!(
        "?from={}&to={}",
        q(Utc::now() - Duration::days(1)),
        q(Utc::now() + Duration::days(1))
    );
    let (_, body) = fetch_stats(state.clone(), &token, &window).await;
    let stats = body.expect("expected a JSON body");

    assert_eq!(stats.summary.books_finished, 1);
    assert_eq!(stats.devices.len(), 1, "one device, no anonymous catch-all");
    assert_eq!(
        stats.devices[0].device_id, "browser-abc",
        "the completion belongs to the declared device"
    );
}

/// Coverage exists so a client knows which years it can offer. It must ignore
/// the window, which is exactly why it is not a field on the statistics
/// response.
#[tokio::test]
async fn coverage_reports_the_whole_history() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;

    let book = book_in_series(&db, "Berserk", "cbz").await;
    record_session(state.clone(), &token, book.id, "phone", 30, at(2)).await;
    record_session(state.clone(), &token, book.id, "phone", 30, at(20)).await;

    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/reading-stats/coverage", &token);
    let (status, body): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    let coverage = body.expect("expected a JSON body");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(coverage["firstReadAt"], serde_json::json!(q(at(2))));
    assert_eq!(coverage["lastReadAt"], serde_json::json!(q(at(20))));
}

#[tokio::test]
async fn coverage_is_null_for_a_reader_with_no_history() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;

    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/reading-stats/coverage", &token);
    let (status, body): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    let coverage = body.expect("expected a JSON body");

    assert_eq!(status, StatusCode::OK);
    assert!(coverage["firstReadAt"].is_null());
    assert!(coverage["lastReadAt"].is_null());
}

#[tokio::test]
async fn coverage_requires_authentication() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;

    let app = create_test_router(state).await;
    let request = get_request("/api/v1/reading-stats/coverage");
    let (status, _body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn statistics_require_authentication() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;

    let app = create_test_router(state).await;
    let request = get_request("/api/v1/reading-stats");
    let (status, _body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// The original bug: a reader seven hours behind UTC finishing a book at 22:36
/// their time is already past midnight in UTC, and without the offset their
/// evening showed up on tomorrow's calendar.
#[tokio::test]
async fn the_viewers_offset_decides_which_day_a_sitting_belongs_to() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = book_in_series(&db, "Berserk", "cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;

    // 05:36 UTC on June 4th is 22:36 on June 3rd at UTC-7.
    let late_evening = Utc.with_ymd_and_hms(2026, 6, 4, 5, 36, 0).unwrap();
    record_session(state.clone(), &token, book.id, "phone", 84, late_evening).await;

    let window = format!("?from={}&to={}&tzOffsetMinutes=-420", q(at(1)), q(at(30)));
    let (status, response) = fetch_stats(state, &token, &window).await;
    let stats = response.expect("expected a JSON body");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(stats.periods.len(), 1);
    assert_eq!(stats.periods[0].bucket, "2026-06-03");
}

/// Without the parameter the buckets stay UTC days, so a client that predates
/// the parameter keeps getting exactly what it got before.
#[tokio::test]
async fn buckets_default_to_utc_days_when_no_offset_is_sent() {
    let (db, _temp_dir) = setup_test_db().await;
    let book = book_in_series(&db, "Berserk", "cbz").await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;

    let late_evening = Utc.with_ymd_and_hms(2026, 6, 4, 5, 36, 0).unwrap();
    record_session(state.clone(), &token, book.id, "phone", 84, late_evening).await;

    let window = format!("?from={}&to={}", q(at(1)), q(at(30)));
    let (_status, response) = fetch_stats(state, &token, &window).await;
    let stats = response.expect("expected a JSON body");

    assert_eq!(stats.periods[0].bucket, "2026-06-04");
}

/// No place on Earth is more than fourteen hours from UTC. Anything wilder is
/// a client bug, and refusing it loudly beats bucketing by a nonsense day.
#[tokio::test]
async fn an_impossible_offset_is_rejected() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;

    let (status, _response) = fetch_stats(state, &token, "?tzOffsetMinutes=1000").await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// The response echoes the window it actually used, so a client that sent no
/// bounds knows what it got back.
#[tokio::test]
async fn the_response_reports_the_window_it_used() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;

    let (status, response) = fetch_stats(state, &token, "").await;
    let stats = response.expect("expected a JSON body");

    assert_eq!(status, StatusCode::OK);
    assert!(stats.from < stats.to, "the default window runs forwards");
    assert!(
        (stats.to - stats.from).num_days() >= 89,
        "the default window covers roughly the last ninety days"
    );
}

// ============================================================================
// Reading whose book was deleted
// ============================================================================

async fn hard_delete_book(db: &sea_orm::DatabaseConnection, book_id: Uuid) {
    use sea_orm::EntityTrait;
    codex::db::entities::books::Entity::delete_by_id(book_id)
        .exec(db)
        .await
        .unwrap();
}

async fn orphaned_completions(db: &sea_orm::DatabaseConnection, user_id: Uuid) -> u64 {
    use codex::db::entities::read_completions;
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
    read_completions::Entity::find()
        .filter(read_completions::Column::UserId.eq(user_id))
        .filter(read_completions::Column::BookId.is_null())
        .count(db)
        .await
        .unwrap()
}

async fn purge_orphans(
    state: std::sync::Arc<codex::api::extractors::AuthState>,
    token: &str,
) -> (StatusCode, Option<PurgedOrphanedHistoryDto>) {
    let app = create_test_router(state).await;
    let request = delete_request_with_auth("/api/v1/reading-stats/orphaned", token);
    make_json_request(app, request).await
}

/// Reading a deleted book keeps counting, and the series and format panels
/// show it as one flagged row rather than dropping it or naming a null series.
#[tokio::test]
async fn deleted_books_reading_shows_as_one_flagged_row() {
    let (db, _temp_dir) = setup_test_db().await;
    let kept = book_in_series(&db, "Berserk", "cbz").await;
    let removed = book_in_series(&db, "Dune", "epub").await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "reader").await;

    record_session(state.clone(), &token, kept.id, "phone", 60, at(3)).await;
    record_session(state.clone(), &token, removed.id, "phone", 20, at(4)).await;
    hard_delete_book(&db, removed.id).await;

    let window = format!("?from={}&to={}", q(at(1)), q(at(30)));
    let (status, response) = fetch_stats(state, &token, &window).await;
    let stats = response.expect("expected a JSON body");
    assert_eq!(status, StatusCode::OK);

    assert_eq!(
        stats.summary.duration.total_ms,
        80 * MINUTE_MS,
        "the deleted book's time still counts"
    );

    assert_eq!(stats.series.len(), 2);
    assert!(!stats.series[0].removed_from_library);
    assert_eq!(stats.series[0].series_name.as_deref(), Some("Berserk"));
    let bucket = &stats.series[1];
    assert!(bucket.removed_from_library);
    assert!(bucket.series_id.is_none());
    assert!(bucket.series_name.is_none());
    assert_eq!(bucket.duration.total_ms, 20 * MINUTE_MS);

    let formats: Vec<_> = stats
        .formats
        .iter()
        .map(|f| (f.format.clone(), f.removed_from_library))
        .collect();
    assert_eq!(
        formats,
        vec![(Some("cbz".to_string()), false), (None, true)],
        "the epub is gone, so its time is in the removed row, ranked like any other"
    );
}

/// Purging removes exactly the caller's orphaned history: their attributed
/// reading stays, and another reader's orphans of the same book are untouched.
#[tokio::test]
async fn purging_orphaned_history_is_scoped_to_the_caller() {
    let (db, _temp_dir) = setup_test_db().await;
    let kept = book_in_series(&db, "Berserk", "cbz").await;
    let removed = book_in_series(&db, "Dune", "epub").await;
    let state = create_test_auth_state(db.clone()).await;
    let (purger, purger_token) = admin_and_token(&db, &state, "purger").await;
    let (_bystander, bystander_token) = admin_and_token(&db, &state, "bystander").await;

    record_session(state.clone(), &purger_token, kept.id, "phone", 60, at(3)).await;
    record_reading(
        state.clone(),
        &purger_token,
        removed.id,
        "phone",
        20,
        10,
        "completed",
        at(4),
    )
    .await;
    record_session(
        state.clone(),
        &bystander_token,
        removed.id,
        "tablet",
        15,
        at(5),
    )
    .await;
    hard_delete_book(&db, removed.id).await;

    let completions = orphaned_completions(&db, purger).await;
    let (status, purged) = purge_orphans(state.clone(), &purger_token).await;
    assert_eq!(status, StatusCode::OK);
    let purged = purged.expect("expected a JSON body");
    assert_eq!(purged.sessions_removed, 1);
    assert_eq!(purged.completions_removed, completions);
    assert_eq!(orphaned_completions(&db, purger).await, 0);

    let window = format!("?from={}&to={}", q(at(1)), q(at(30)));
    let (_, mine) = fetch_stats(state.clone(), &purger_token, &window).await;
    let mine = mine.expect("expected a JSON body");
    assert_eq!(mine.summary.duration.total_ms, 60 * MINUTE_MS);
    assert!(mine.series.iter().all(|s| !s.removed_from_library));

    let (_, theirs) = fetch_stats(state.clone(), &bystander_token, &window).await;
    let theirs = theirs.expect("expected a JSON body");
    assert_eq!(theirs.summary.duration.total_ms, 15 * MINUTE_MS);
    assert!(theirs.series[0].removed_from_library);

    // Nothing left to purge is not an error.
    let (status, again) = purge_orphans(state, &purger_token).await;
    assert_eq!(status, StatusCode::OK);
    let again = again.expect("expected a JSON body");
    assert_eq!((again.sessions_removed, again.completions_removed), (0, 0));
}
