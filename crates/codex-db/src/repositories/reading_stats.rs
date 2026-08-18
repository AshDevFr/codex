//! Aggregations over the reading session log.
//!
//! Everything here groups in SQL rather than in memory: the log is the largest
//! table a single user touches, and pulling a year of sessions into the process
//! to sum them would scale with reading rather than with the size of the answer.
//!
//! # Honesty about where the numbers came from
//!
//! Reading time arrives two ways. Clients that measure it report it directly.
//! Clients that cannot (the Komga-compatible, OPDS and KOReader surfaces) have
//! it reconstructed from the gaps between their writes, which systematically
//! undercounts and cannot see reading done from a fully downloaded book at all.
//!
//! Every total here therefore carries both figures separately rather than one
//! blended number. A dashboard can present a single total if it wants, but it
//! has to choose to, and it can say how much of that total is a reconstruction.

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement, Value};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// How finely to bucket a time series.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatsGranularity {
    Day,
    Week,
    Month,
}

impl StatsGranularity {
    /// The SQL expression that truncates a timestamp to this bucket.
    ///
    /// Both engines are asked for an ISO date string rather than a native date,
    /// so a bucket key means the same thing whichever database is underneath
    /// and the API contract does not depend on the deployment.
    fn bucket_expr(self, backend: DatabaseBackend) -> &'static str {
        match (backend, self) {
            // `%W` weeks start on Monday and the offset lands the key on that
            // Monday, so a week bucket is a real date rather than an ordinal.
            (DatabaseBackend::Sqlite, Self::Day) => "strftime('%Y-%m-%d', rs.client_started_at)",
            (DatabaseBackend::Sqlite, Self::Week) => {
                "date(rs.client_started_at, 'weekday 1', '-7 days')"
            }
            (DatabaseBackend::Sqlite, Self::Month) => "strftime('%Y-%m-01', rs.client_started_at)",
            (_, Self::Day) => "to_char(date_trunc('day', rs.client_started_at), 'YYYY-MM-DD')",
            (_, Self::Week) => "to_char(date_trunc('week', rs.client_started_at), 'YYYY-MM-DD')",
            (_, Self::Month) => "to_char(date_trunc('month', rs.client_started_at), 'YYYY-MM-DD')",
        }
    }
}

/// Which measure a breakdown is ranked by.
///
/// Ranking happens in SQL because the series breakdown is limited: re-sorting
/// the returned rows would order a set that was *selected* by a different
/// measure, so the true leader by pages can be absent from a top-N chosen by
/// time. A library whose reading predates session tracking has no time and no
/// pages at all, which is why completions are rankable too.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatsSort {
    Time,
    Pages,
    Completions,
}

impl StatsSort {
    /// The `ORDER BY` clause for this key.
    ///
    /// Every arm falls back through the other measures before the caller's
    /// name tiebreak, so a set that ties on the chosen key is still ordered by
    /// something a reader would recognise rather than by insertion order.
    fn order_by(self, tiebreak: &str) -> String {
        let keys = match self {
            Self::Time => [TOTAL_SUM, PAGES_SUM, COMPLETIONS_SUM],
            Self::Pages => [PAGES_SUM, TOTAL_SUM, COMPLETIONS_SUM],
            Self::Completions => [COMPLETIONS_SUM, TOTAL_SUM, PAGES_SUM],
        };
        format!(
            "ORDER BY {} DESC, {} DESC, {} DESC, {tiebreak} ASC",
            keys[0], keys[1], keys[2]
        )
    }
}

/// Reading time split by how it was arrived at.
///
/// Never summed into one field at this layer. Presenting a combined figure is a
/// choice a caller can make; hiding that part of it was reconstructed is not.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurationBreakdown {
    /// Milliseconds reported by a client that measured its own reading.
    pub measured_ms: i64,
    /// Milliseconds reconstructed server-side from the gaps between writes.
    /// Undercounts, and misses offline reading entirely.
    pub inferred_ms: i64,
}

impl DurationBreakdown {
    pub fn total_ms(&self) -> i64 {
        self.measured_ms + self.inferred_ms
    }
}

/// Totals for a period.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReadingSummary {
    pub duration: DurationBreakdown,
    pub pages_read: i64,
    /// Distinct sittings, after coalescing.
    pub sessions: i64,
    pub books: i64,
    /// Books finished in the window, from `completed` events.
    pub books_finished: i64,
    /// Sessions whose producer could report neither measured nor reconstructed
    /// time. Surfaced so a suspiciously low total has a visible explanation.
    pub sessions_without_duration: i64,
    /// Sessions that reported no page count. Only a client that measures its
    /// own sitting reports one, so this covers everything read before session
    /// tracking existed as well as every app that just saves a position.
    /// Surfaced for the same reason as the duration gap.
    pub sessions_without_pages: i64,
}

/// One bucket of a time series.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReadingPeriod {
    /// ISO date of the bucket's start.
    pub bucket: String,
    pub duration: DurationBreakdown,
    pub pages_read: i64,
    pub sessions: i64,
    pub books_finished: i64,
}

/// Totals for one device.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReadingByDevice {
    pub device_id: String,
    pub device_name: Option<String>,
    pub duration: DurationBreakdown,
    pub pages_read: i64,
    pub sessions: i64,
    pub books_finished: i64,
    pub last_read_at: DateTime<Utc>,
}

/// Totals for one series.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReadingBySeries {
    pub series_id: Uuid,
    pub series_name: String,
    pub duration: DurationBreakdown,
    pub pages_read: i64,
    pub sessions: i64,
    pub books: i64,
    pub books_finished: i64,
}

/// Totals for one file format.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReadingByFormat {
    pub format: String,
    pub duration: DurationBreakdown,
    pub pages_read: i64,
    pub sessions: i64,
    pub books_finished: i64,
}

/// The span a reader's whole history covers, independent of any window.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadingCoverage {
    /// When this reader first read anything, or `None` if they never have.
    pub first_read_at: Option<DateTime<Utc>>,
    pub last_read_at: Option<DateTime<Utc>>,
}

/// The window a query covers.
#[derive(Copy, Clone, Debug)]
pub struct StatsWindow {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

pub struct ReadingStatsRepository;

/// Sum of measured time, as a SQL fragment.
///
/// Written as a conditional sum rather than two queries so one pass over the
/// index answers both halves of the breakdown.
///
/// The explicit `CAST(... AS BIGINT)` is load-bearing on PostgreSQL, not
/// decoration: `SUM` over a `bigint` column widens to `numeric` there, which
/// will not decode into an `i64`. SQLite returns an integer either way, so
/// leaving the cast off fails only on PostgreSQL and only at runtime.
const MEASURED_SUM: &str = "CAST(COALESCE(SUM(CASE WHEN rs.duration_source = 'measured' \
     THEN rs.active_duration_ms ELSE 0 END), 0) AS BIGINT)";
const INFERRED_SUM: &str = "CAST(COALESCE(SUM(CASE WHEN rs.duration_source = 'inferred' \
     THEN rs.active_duration_ms ELSE 0 END), 0) AS BIGINT)";
const PAGES_SUM: &str = "CAST(COALESCE(SUM(COALESCE(rs.pages_read, 0)), 0) AS BIGINT)";

/// Books finished, as a SQL fragment.
///
/// The one measure that means the same thing on both sides of the session-log
/// cutover. Backfilled rows carry no duration and no page count, so time and
/// pages are silent about everything a reader did before time tracking; a
/// completion is a completion either way.
const COMPLETIONS_SUM: &str =
    "CAST(COALESCE(SUM(CASE WHEN rs.kind = 'completed' THEN 1 ELSE 0 END), 0) AS BIGINT)";

/// Measured and reconstructed time together, for ranking.
///
/// Spelled out rather than written as `measured_ms + inferred_ms` because
/// PostgreSQL only accepts an output alias as a whole `ORDER BY` key, never
/// inside an expression. SQLite accepts either, so the alias form works
/// everywhere except the engine that matters in production.
const TOTAL_SUM: &str = "CAST(COALESCE(SUM(CASE WHEN rs.duration_source IN ('measured', 'inferred') \
     THEN rs.active_duration_ms ELSE 0 END), 0) AS BIGINT)";

/// Only `progress` and `completed` rows describe reading. A `reset` records
/// that a book was marked unread, which is bookkeeping rather than a sitting,
/// and counting it would inflate session counts with non-events.
const READING_KINDS: &str = "rs.kind IN ('progress', 'completed')";

/// How a series is displayed and ordered.
///
/// `series.name` is the scanned directory name, kept verbatim so files still
/// match after a rename; the title a reader sees is `series_metadata.title`,
/// which preprocessing rules have cleaned (a "(Digital)" suffix stripped, a
/// dash turned into a colon). Statistics must name a series the way the rest of
/// the app does, and order it the way the library orders it.
///
/// Left-joined and coalesced rather than inner-joined: a series whose metadata
/// row is missing has to keep appearing in the reader's own history.
const SERIES_TITLE_SORT: &str = "COALESCE(sm.title_sort, sm.title, s.name)";

#[derive(Debug, FromQueryResult)]
struct SummaryRow {
    measured_ms: i64,
    inferred_ms: i64,
    pages_read: i64,
    sessions: i64,
    books_finished: i64,
    books: i64,
    sessions_without_duration: i64,
    sessions_without_pages: i64,
}

#[derive(Debug, FromQueryResult)]
struct PeriodRow {
    bucket: String,
    measured_ms: i64,
    inferred_ms: i64,
    pages_read: i64,
    sessions: i64,
    books_finished: i64,
}

#[derive(Debug, FromQueryResult)]
struct DeviceRow {
    device_id: String,
    device_name: Option<String>,
    measured_ms: i64,
    inferred_ms: i64,
    pages_read: i64,
    sessions: i64,
    books_finished: i64,
    last_read_at: DateTime<Utc>,
}

#[derive(Debug, FromQueryResult)]
struct SeriesRow {
    series_id: Uuid,
    series_name: String,
    measured_ms: i64,
    inferred_ms: i64,
    pages_read: i64,
    sessions: i64,
    books_finished: i64,
    books: i64,
}

#[derive(Debug, FromQueryResult)]
struct FormatRow {
    format: String,
    measured_ms: i64,
    inferred_ms: i64,
    pages_read: i64,
    sessions: i64,
    books_finished: i64,
}

impl ReadingStatsRepository {
    /// Headline totals for a window.
    pub async fn summary<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        window: StatsWindow,
    ) -> Result<ReadingSummary> {
        let backend = db.get_database_backend();
        let sql = format!(
            "SELECT {MEASURED_SUM} AS measured_ms, \
                    {INFERRED_SUM} AS inferred_ms, \
                    {PAGES_SUM} AS pages_read, \
                    COUNT(*) AS sessions, \
                    {COMPLETIONS_SUM} AS books_finished, \
                    COUNT(DISTINCT rs.book_id) AS books, \
                    CAST(COALESCE(SUM(CASE WHEN rs.active_duration_ms IS NULL THEN 1 ELSE 0 END), 0) \
                        AS BIGINT) AS sessions_without_duration, \
                    CAST(COALESCE(SUM(CASE WHEN rs.pages_read IS NULL THEN 1 ELSE 0 END), 0) \
                        AS BIGINT) AS sessions_without_pages \
             FROM reading_sessions rs \
             WHERE rs.user_id = $1 AND {READING_KINDS} \
               AND rs.client_started_at >= $2 AND rs.client_started_at < $3"
        );

        let row = SummaryRow::find_by_statement(Statement::from_sql_and_values(
            backend,
            &sql,
            window_params(user_id, window),
        ))
        .one(db)
        .await?;

        Ok(
            row.map_or_else(ReadingSummary::default, |r| ReadingSummary {
                duration: DurationBreakdown {
                    measured_ms: r.measured_ms,
                    inferred_ms: r.inferred_ms,
                },
                pages_read: r.pages_read,
                sessions: r.sessions,
                books_finished: r.books_finished,
                books: r.books,
                sessions_without_duration: r.sessions_without_duration,
                sessions_without_pages: r.sessions_without_pages,
            }),
        )
    }

    /// A time series, one row per bucket that has reading in it.
    ///
    /// Empty buckets are omitted rather than zero-filled: the caller knows the
    /// window it asked for and can fill gaps for display, and sending a row per
    /// silent day of a multi-year range would dwarf the answer.
    pub async fn by_period<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        window: StatsWindow,
        granularity: StatsGranularity,
    ) -> Result<Vec<ReadingPeriod>> {
        let backend = db.get_database_backend();
        let bucket = granularity.bucket_expr(backend);
        let sql = format!(
            "SELECT {bucket} AS bucket, \
                    {MEASURED_SUM} AS measured_ms, \
                    {INFERRED_SUM} AS inferred_ms, \
                    {PAGES_SUM} AS pages_read, \
                    COUNT(*) AS sessions, \
                    {COMPLETIONS_SUM} AS books_finished \
             FROM reading_sessions rs \
             WHERE rs.user_id = $1 AND {READING_KINDS} \
               AND rs.client_started_at >= $2 AND rs.client_started_at < $3 \
             GROUP BY {bucket} \
             ORDER BY bucket ASC"
        );

        let rows = PeriodRow::find_by_statement(Statement::from_sql_and_values(
            backend,
            &sql,
            window_params(user_id, window),
        ))
        .all(db)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ReadingPeriod {
                bucket: r.bucket,
                duration: DurationBreakdown {
                    measured_ms: r.measured_ms,
                    inferred_ms: r.inferred_ms,
                },
                pages_read: r.pages_read,
                sessions: r.sessions,
                books_finished: r.books_finished,
            })
            .collect())
    }

    /// Totals per device, most-read first.
    pub async fn by_device<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        window: StatsWindow,
        sort: StatsSort,
    ) -> Result<Vec<ReadingByDevice>> {
        let backend = db.get_database_backend();
        let order_by = sort.order_by("device_id");
        let sql = format!(
            "SELECT rs.device_id AS device_id, \
                    MAX(rs.device_name) AS device_name, \
                    {MEASURED_SUM} AS measured_ms, \
                    {INFERRED_SUM} AS inferred_ms, \
                    {PAGES_SUM} AS pages_read, \
                    COUNT(*) AS sessions, \
                    {COMPLETIONS_SUM} AS books_finished, \
                    MAX(rs.client_ended_at) AS last_read_at \
             FROM reading_sessions rs \
             WHERE rs.user_id = $1 AND {READING_KINDS} \
               AND rs.client_started_at >= $2 AND rs.client_started_at < $3 \
             GROUP BY rs.device_id \
             {order_by}"
        );

        let rows = DeviceRow::find_by_statement(Statement::from_sql_and_values(
            backend,
            &sql,
            window_params(user_id, window),
        ))
        .all(db)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ReadingByDevice {
                device_id: r.device_id,
                device_name: r.device_name,
                duration: DurationBreakdown {
                    measured_ms: r.measured_ms,
                    inferred_ms: r.inferred_ms,
                },
                pages_read: r.pages_read,
                sessions: r.sessions,
                books_finished: r.books_finished,
                last_read_at: r.last_read_at,
            })
            .collect())
    }

    /// Most-read series in the window.
    pub async fn by_series<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        window: StatsWindow,
        sort: StatsSort,
        limit: u64,
    ) -> Result<Vec<ReadingBySeries>> {
        let backend = db.get_database_backend();
        let order_by = sort.order_by(SERIES_TITLE_SORT);
        let sql = format!(
            "SELECT s.id AS series_id, \
                    COALESCE(sm.title, s.name) AS series_name, \
                    {MEASURED_SUM} AS measured_ms, \
                    {INFERRED_SUM} AS inferred_ms, \
                    {PAGES_SUM} AS pages_read, \
                    COUNT(*) AS sessions, \
                    {COMPLETIONS_SUM} AS books_finished, \
                    COUNT(DISTINCT rs.book_id) AS books \
             FROM reading_sessions rs \
             JOIN books b ON b.id = rs.book_id \
             JOIN series s ON s.id = b.series_id \
             LEFT JOIN series_metadata sm ON sm.series_id = s.id \
             WHERE rs.user_id = $1 AND {READING_KINDS} \
               AND rs.client_started_at >= $2 AND rs.client_started_at < $3 \
             GROUP BY s.id, s.name, sm.title, sm.title_sort \
             {order_by} \
             LIMIT $4"
        );

        let mut params = window_params(user_id, window);
        params.push(Value::BigInt(Some(limit as i64)));

        let rows =
            SeriesRow::find_by_statement(Statement::from_sql_and_values(backend, &sql, params))
                .all(db)
                .await?;

        Ok(rows
            .into_iter()
            .map(|r| ReadingBySeries {
                series_id: r.series_id,
                series_name: r.series_name,
                duration: DurationBreakdown {
                    measured_ms: r.measured_ms,
                    inferred_ms: r.inferred_ms,
                },
                pages_read: r.pages_read,
                sessions: r.sessions,
                books_finished: r.books_finished,
                books: r.books,
            })
            .collect())
    }

    /// Totals per file format, which is a decent proxy for what kind of
    /// reading a person actually does: comics, ebooks or documents.
    pub async fn by_format<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        window: StatsWindow,
        sort: StatsSort,
    ) -> Result<Vec<ReadingByFormat>> {
        let backend = db.get_database_backend();
        let order_by = sort.order_by("format");
        let sql = format!(
            "SELECT b.format AS format, \
                    {MEASURED_SUM} AS measured_ms, \
                    {INFERRED_SUM} AS inferred_ms, \
                    {PAGES_SUM} AS pages_read, \
                    COUNT(*) AS sessions, \
                    {COMPLETIONS_SUM} AS books_finished \
             FROM reading_sessions rs \
             JOIN books b ON b.id = rs.book_id \
             WHERE rs.user_id = $1 AND {READING_KINDS} \
               AND rs.client_started_at >= $2 AND rs.client_started_at < $3 \
             GROUP BY b.format \
             {order_by}"
        );

        let rows = FormatRow::find_by_statement(Statement::from_sql_and_values(
            backend,
            &sql,
            window_params(user_id, window),
        ))
        .all(db)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ReadingByFormat {
                format: r.format,
                duration: DurationBreakdown {
                    measured_ms: r.measured_ms,
                    inferred_ms: r.inferred_ms,
                },
                pages_read: r.pages_read,
                sessions: r.sessions,
                books_finished: r.books_finished,
            })
            .collect())
    }

    /// The span a reader's history actually covers, ignoring any window.
    ///
    /// Deliberately unwindowed, which is why it is not folded into the
    /// statistics response: that response describes a window, and a field
    /// inside it that ignored the window would be read as obeying it. A client
    /// needs this to know which years it can offer, and the answer changes at
    /// most once a day.
    ///
    /// Both figures are `None` for a reader who has never read.
    pub async fn coverage<C: ConnectionTrait>(db: &C, user_id: Uuid) -> Result<ReadingCoverage> {
        #[derive(Debug, FromQueryResult)]
        struct CoverageRow {
            first_read_at: Option<DateTime<Utc>>,
            last_read_at: Option<DateTime<Utc>>,
        }

        let backend = db.get_database_backend();
        let sql = format!(
            "SELECT MIN(rs.client_started_at) AS first_read_at, \
                    MAX(rs.client_started_at) AS last_read_at \
             FROM reading_sessions rs \
             WHERE rs.user_id = $1 AND {READING_KINDS}"
        );

        let row = CoverageRow::find_by_statement(Statement::from_sql_and_values(
            backend,
            &sql,
            [Value::Uuid(Some(Box::new(user_id)))],
        ))
        .one(db)
        .await?;

        Ok(
            row.map_or_else(ReadingCoverage::default, |r| ReadingCoverage {
                first_read_at: r.first_read_at,
                last_read_at: r.last_read_at,
            }),
        )
    }

    /// How many session rows a user has, for judging whether retention or
    /// compaction is worth building.
    pub async fn row_count<C: ConnectionTrait>(db: &C, user_id: Uuid) -> Result<i64> {
        #[derive(Debug, FromQueryResult)]
        struct CountRow {
            total: i64,
        }

        let backend = db.get_database_backend();
        let row = CountRow::find_by_statement(Statement::from_sql_and_values(
            backend,
            "SELECT COUNT(*) AS total FROM reading_sessions rs WHERE rs.user_id = $1",
            [Value::Uuid(Some(Box::new(user_id)))],
        ))
        .one(db)
        .await?;

        Ok(row.map_or(0, |r| r.total))
    }
}

fn window_params(user_id: Uuid, window: StatsWindow) -> Vec<Value> {
    vec![
        Value::Uuid(Some(Box::new(user_id))),
        Value::ChronoDateTimeUtc(Some(Box::new(window.from))),
        Value::ChronoDateTimeUtc(Some(Box::new(window.to))),
    ]
}

/// The stored discriminator for measured time, kept next to the SQL that
/// matches on it so the two cannot drift apart.
#[cfg(test)]
mod discriminator_guard {
    use super::*;
    use crate::entities::reading_sessions::DurationSource;

    #[test]
    fn the_sql_matches_the_stored_discriminators() {
        assert!(MEASURED_SUM.contains(DurationSource::Measured.as_str()));
        assert!(INFERRED_SUM.contains(DurationSource::Inferred.as_str()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::reading_sessions::DurationSource;
    use crate::entities::{books, reading_sessions, series_metadata, users};
    use crate::repositories::{
        BookRepository, LibraryRepository, SeriesRepository, UserRepository,
    };
    use crate::test_helpers::setup_test_db;
    use chrono::{Duration, TimeZone};
    use codex_models::ScanningStrategy;
    use codex_utils::password;
    use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};

    const MINUTE_MS: i64 = 60_000;

    fn at(day: u32, hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, day, hour, 0, 0).unwrap()
    }

    fn whole_of_june() -> StatsWindow {
        StatsWindow {
            from: at(1, 0),
            to: at(30, 23),
        }
    }

    async fn create_user(db: &DatabaseConnection, name: &str) -> users::Model {
        let password_hash = password::hash_password("password").unwrap();
        UserRepository::create(
            db,
            &users::Model {
                id: Uuid::new_v4(),
                username: name.to_string(),
                email: format!("{name}@example.com"),
                password_hash,
                role: "admin".to_string(),
                is_active: true,
                email_verified: false,
                permissions: serde_json::json!([]),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                last_login_at: None,
            },
        )
        .await
        .unwrap()
    }

    /// A book in a named series, so series and format aggregations have
    /// something meaningful to group by.
    async fn create_book(db: &DatabaseConnection, series_name: &str, format: &str) -> books::Model {
        create_book_titled(db, series_name, None, format).await
    }

    /// A book whose series carries a metadata title distinct from the scanned
    /// directory name, which is the normal case once preprocessing rules have
    /// cleaned a title.
    async fn create_book_titled(
        db: &DatabaseConnection,
        series_name: &str,
        metadata_title: Option<&str>,
        format: &str,
    ) -> books::Model {
        let library = LibraryRepository::create(db, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = SeriesRepository::create_with_fingerprint_and_title(
            db,
            library.id,
            series_name,
            None,
            series_name.to_string(),
            metadata_title,
            None,
        )
        .await
        .unwrap();
        let book = books::Model {
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

    struct SessionSpec {
        device: &'static str,
        device_name: Option<&'static str>,
        kind: &'static str,
        duration_ms: Option<i64>,
        source: DurationSource,
        pages: Option<i32>,
        started: DateTime<Utc>,
    }

    impl SessionSpec {
        fn measured(device: &'static str, minutes: i64, started: DateTime<Utc>) -> Self {
            Self {
                device,
                device_name: Some("Test Device"),
                kind: "progress",
                duration_ms: Some(minutes * MINUTE_MS),
                source: DurationSource::Measured,
                pages: Some(10),
                started,
            }
        }

        fn inferred(device: &'static str, minutes: i64, started: DateTime<Utc>) -> Self {
            Self {
                source: DurationSource::Inferred,
                ..Self::measured(device, minutes, started)
            }
        }

        fn without_duration(device: &'static str, started: DateTime<Utc>) -> Self {
            Self {
                duration_ms: None,
                source: DurationSource::Unknown,
                pages: None,
                ..Self::measured(device, 0, started)
            }
        }

        fn without_pages(mut self) -> Self {
            self.pages = None;
            self
        }

        fn kind(mut self, kind: &'static str) -> Self {
            self.kind = kind;
            self
        }

        fn pages(mut self, pages: i32) -> Self {
            self.pages = Some(pages);
            self
        }
    }

    async fn insert(db: &DatabaseConnection, user_id: Uuid, book_id: Uuid, spec: SessionSpec) {
        reading_sessions::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            book_id: Set(book_id),
            device_id: Set(spec.device.to_string()),
            device_name: Set(spec.device_name.map(str::to_string)),
            pass: Set(1),
            kind: Set(spec.kind.to_string()),
            to_page: Set(Some(1)),
            to_percentage: Set(None),
            r2_progression: Set(None),
            active_duration_ms: Set(spec.duration_ms),
            duration_source: Set(spec.source.as_str().to_string()),
            pages_read: Set(spec.pages),
            client_started_at: Set(spec.started),
            client_ended_at: Set(spec.started + Duration::minutes(30)),
            server_recorded_at: Set(spec.started + Duration::minutes(30)),
        }
        .insert(db)
        .await
        .unwrap();
    }

    // ----------------------------------------------------------------
    // Summary
    // ----------------------------------------------------------------

    #[tokio::test]
    async fn an_empty_log_reports_zeroes_rather_than_failing() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;

        let summary = ReadingStatsRepository::summary(&db, user.id, whole_of_june())
            .await
            .unwrap();

        assert_eq!(summary, ReadingSummary::default());
    }

    /// The headline number, and the thing the whole project exists to answer.
    #[tokio::test]
    async fn summary_totals_reading_across_sessions() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 30, at(1, 9)),
        )
        .await;
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 45, at(2, 9)),
        )
        .await;

        let summary = ReadingStatsRepository::summary(&db, user.id, whole_of_june())
            .await
            .unwrap();

        assert_eq!(summary.duration.measured_ms, 75 * MINUTE_MS);
        assert_eq!(summary.duration.total_ms(), 75 * MINUTE_MS);
        assert_eq!(summary.pages_read, 20);
        assert_eq!(summary.sessions, 2);
        assert_eq!(summary.books, 1);
    }

    /// Reconstructed time is reported beside measured time, never folded into
    /// it, so a dashboard can say how much of a total is a guess.
    #[tokio::test]
    async fn measured_and_inferred_time_stay_separable() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 30, at(1, 9)),
        )
        .await;
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::inferred("komic", 20, at(1, 14)),
        )
        .await;

        let summary = ReadingStatsRepository::summary(&db, user.id, whole_of_june())
            .await
            .unwrap();

        assert_eq!(summary.duration.measured_ms, 30 * MINUTE_MS);
        assert_eq!(summary.duration.inferred_ms, 20 * MINUTE_MS);
    }

    /// A suspiciously low total should have a visible explanation rather than
    /// looking like the user simply did not read.
    #[tokio::test]
    async fn sessions_with_no_duration_are_counted_separately() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 30, at(1, 9)),
        )
        .await;
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::without_duration("opds", at(1, 12)),
        )
        .await;

        let summary = ReadingStatsRepository::summary(&db, user.id, whole_of_june())
            .await
            .unwrap();

        assert_eq!(summary.sessions, 2);
        assert_eq!(summary.sessions_without_duration, 1);
        assert_eq!(summary.duration.total_ms(), 30 * MINUTE_MS);
    }

    /// Pages are as silent as time about everything read before session
    /// tracking, and about every app that only saves a position. Counted
    /// separately for the same reason duration is: so a low total reads as an
    /// incomplete record rather than as "you barely read".
    ///
    /// Independent of the duration gap: a client can measure its own sitting
    /// and still not report a page count.
    #[tokio::test]
    async fn sessions_with_no_page_count_are_counted_separately() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 30, at(1, 9)),
        )
        .await;
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 15, at(1, 11)).without_pages(),
        )
        .await;
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::without_duration("opds", at(1, 12)),
        )
        .await;

        let summary = ReadingStatsRepository::summary(&db, user.id, whole_of_june())
            .await
            .unwrap();

        assert_eq!(summary.sessions, 3);
        assert_eq!(summary.pages_read, 10);
        assert_eq!(summary.sessions_without_pages, 2);
        assert_eq!(summary.sessions_without_duration, 1);
    }

    /// Marking a book unread is bookkeeping, not a sitting.
    #[tokio::test]
    async fn resets_do_not_count_as_reading() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 30, at(1, 9)),
        )
        .await;
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::without_duration("phone", at(1, 10)).kind("reset"),
        )
        .await;

        let summary = ReadingStatsRepository::summary(&db, user.id, whole_of_june())
            .await
            .unwrap();

        assert_eq!(summary.sessions, 1, "a reset is not a sitting");
    }

    /// Finishing a book is reading and must be counted.
    #[tokio::test]
    async fn completions_count_as_reading() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 30, at(1, 9)).kind("completed"),
        )
        .await;

        let summary = ReadingStatsRepository::summary(&db, user.id, whole_of_june())
            .await
            .unwrap();

        assert_eq!(summary.sessions, 1);
        assert_eq!(summary.duration.measured_ms, 30 * MINUTE_MS);
    }

    #[tokio::test]
    async fn the_window_excludes_reading_outside_it() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 30, at(5, 9)),
        )
        .await;
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 30, at(20, 9)),
        )
        .await;

        let summary = ReadingStatsRepository::summary(
            &db,
            user.id,
            StatsWindow {
                from: at(1, 0),
                to: at(10, 0),
            },
        )
        .await
        .unwrap();

        assert_eq!(summary.sessions, 1);
    }

    /// Statistics are per user. Another reader's sessions must never appear.
    #[tokio::test]
    async fn statistics_are_scoped_to_one_user() {
        let db = setup_test_db().await;
        let alice = create_user(&db, "alice").await;
        let bob = create_user(&db, "bob").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            alice.id,
            book.id,
            SessionSpec::measured("phone", 30, at(1, 9)),
        )
        .await;
        insert(
            &db,
            bob.id,
            book.id,
            SessionSpec::measured("phone", 99, at(1, 9)),
        )
        .await;

        let summary = ReadingStatsRepository::summary(&db, alice.id, whole_of_june())
            .await
            .unwrap();

        assert_eq!(summary.duration.measured_ms, 30 * MINUTE_MS);
    }

    // ----------------------------------------------------------------
    // Time series
    // ----------------------------------------------------------------

    #[tokio::test]
    async fn daily_buckets_group_by_calendar_day() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 10, at(3, 9)),
        )
        .await;
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 20, at(3, 20)),
        )
        .await;
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 5, at(4, 9)),
        )
        .await;

        let periods =
            ReadingStatsRepository::by_period(&db, user.id, whole_of_june(), StatsGranularity::Day)
                .await
                .unwrap();

        assert_eq!(periods.len(), 2);
        assert_eq!(periods[0].bucket, "2026-06-03");
        assert_eq!(periods[0].duration.measured_ms, 30 * MINUTE_MS);
        assert_eq!(periods[1].bucket, "2026-06-04");
        assert_eq!(periods[1].duration.measured_ms, 5 * MINUTE_MS);
    }

    /// Days with no reading are absent, not zero rows: a multi-year range
    /// would otherwise be mostly padding.
    #[tokio::test]
    async fn silent_days_are_omitted_rather_than_zero_filled() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 10, at(1, 9)),
        )
        .await;
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 10, at(28, 9)),
        )
        .await;

        let periods =
            ReadingStatsRepository::by_period(&db, user.id, whole_of_june(), StatsGranularity::Day)
                .await
                .unwrap();

        assert_eq!(periods.len(), 2);
    }

    #[tokio::test]
    async fn monthly_buckets_land_on_the_first_of_the_month() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 10, at(3, 9)),
        )
        .await;
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 10, at(25, 9)),
        )
        .await;

        let periods = ReadingStatsRepository::by_period(
            &db,
            user.id,
            whole_of_june(),
            StatsGranularity::Month,
        )
        .await
        .unwrap();

        assert_eq!(periods.len(), 1);
        assert_eq!(periods[0].bucket, "2026-06-01");
        assert_eq!(periods[0].duration.measured_ms, 20 * MINUTE_MS);
    }

    /// Weeks start on Monday and the key is that Monday's date, so the bucket
    /// means the same thing on both engines.
    #[tokio::test]
    async fn weekly_buckets_start_on_monday() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        // 2026-06-03 is a Wednesday; 2026-06-06 is the Saturday of that week.
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 10, at(3, 9)),
        )
        .await;
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 10, at(6, 9)),
        )
        .await;

        let periods = ReadingStatsRepository::by_period(
            &db,
            user.id,
            whole_of_june(),
            StatsGranularity::Week,
        )
        .await
        .unwrap();

        assert_eq!(periods.len(), 1, "both fall in the same week");
        assert_eq!(periods[0].bucket, "2026-06-01", "the week's Monday");
    }

    // ----------------------------------------------------------------
    // Breakdowns
    // ----------------------------------------------------------------

    #[tokio::test]
    async fn devices_are_ranked_by_time_read() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("laptop", 10, at(1, 9)),
        )
        .await;
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 60, at(1, 10)),
        )
        .await;

        let devices =
            ReadingStatsRepository::by_device(&db, user.id, whole_of_june(), StatsSort::Time)
                .await
                .unwrap();

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].device_id, "phone");
        assert_eq!(devices[0].duration.measured_ms, 60 * MINUTE_MS);
        assert_eq!(devices[1].device_id, "laptop");
    }

    #[tokio::test]
    async fn a_device_reports_when_it_last_read() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 10, at(1, 9)),
        )
        .await;
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 10, at(9, 9)),
        )
        .await;

        let devices =
            ReadingStatsRepository::by_device(&db, user.id, whole_of_june(), StatsSort::Time)
                .await
                .unwrap();

        assert_eq!(devices[0].last_read_at, at(9, 9) + Duration::minutes(30));
    }

    #[tokio::test]
    async fn series_are_ranked_by_time_read() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let berserk = create_book(&db, "Berserk", "cbz").await;
        let vinland = create_book(&db, "Vinland Saga", "cbz").await;

        insert(
            &db,
            user.id,
            berserk.id,
            SessionSpec::measured("phone", 90, at(1, 9)),
        )
        .await;
        insert(
            &db,
            user.id,
            vinland.id,
            SessionSpec::measured("phone", 15, at(2, 9)),
        )
        .await;

        let series =
            ReadingStatsRepository::by_series(&db, user.id, whole_of_june(), StatsSort::Time, 10)
                .await
                .unwrap();

        assert_eq!(series.len(), 2);
        assert_eq!(series[0].series_name, "Berserk");
        assert_eq!(series[0].duration.measured_ms, 90 * MINUTE_MS);
        assert_eq!(series[0].books, 1);
        assert_eq!(series[1].series_name, "Vinland Saga");
    }

    /// Coverage answers "which years can this reader ask for", so it must not
    /// be bounded by whatever window the dashboard happens to be showing.
    #[tokio::test]
    async fn coverage_spans_the_whole_history() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 30, at(2, 9)),
        )
        .await;
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 30, at(20, 9)),
        )
        .await;
        // A reset is bookkeeping rather than reading, so it must not stretch
        // the span into a year the reader never read in.
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::without_duration("phone", at(28, 9)).kind("reset"),
        )
        .await;

        let coverage = ReadingStatsRepository::coverage(&db, user.id)
            .await
            .unwrap();

        assert_eq!(coverage.first_read_at, Some(at(2, 9)));
        assert_eq!(coverage.last_read_at, Some(at(20, 9)));
    }

    #[tokio::test]
    async fn coverage_is_empty_for_a_reader_who_never_read() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;

        let coverage = ReadingStatsRepository::coverage(&db, user.id)
            .await
            .unwrap();

        assert_eq!(coverage.first_read_at, None);
        assert_eq!(coverage.last_read_at, None);
    }

    /// Coverage is per reader, like every other statistic here.
    #[tokio::test]
    async fn coverage_never_leaks_between_readers() {
        let db = setup_test_db().await;
        let reader = create_user(&db, "reader").await;
        let other = create_user(&db, "other").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            other.id,
            book.id,
            SessionSpec::measured("phone", 30, at(2, 9)),
        )
        .await;

        let coverage = ReadingStatsRepository::coverage(&db, reader.id)
            .await
            .unwrap();

        assert_eq!(coverage.first_read_at, None);
    }

    /// The only measure a library backfilled from `read_progress` can answer.
    /// Those rows carry no duration and no page count, but a completion means
    /// the same thing before and after time tracking existed.
    #[tokio::test]
    async fn finished_books_are_counted_from_completions() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 30, at(1, 9)),
        )
        .await;
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 20, at(2, 9)).kind("completed"),
        )
        .await;
        // A reset records a book being marked unread. It is bookkeeping, and is
        // already excluded from sittings; it must not count as finishing one.
        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::without_duration("phone", at(3, 9)).kind("reset"),
        )
        .await;

        let summary = ReadingStatsRepository::summary(&db, user.id, whole_of_june())
            .await
            .unwrap();

        assert_eq!(summary.sessions, 2, "the reset is not a sitting");
        assert_eq!(summary.books_finished, 1);
    }

    #[tokio::test]
    async fn finished_books_appear_in_every_breakdown() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 20, at(1, 9)).kind("completed"),
        )
        .await;

        let window = whole_of_june();
        let periods =
            ReadingStatsRepository::by_period(&db, user.id, window, StatsGranularity::Day)
                .await
                .unwrap();
        let series = ReadingStatsRepository::by_series(&db, user.id, window, StatsSort::Time, 10)
            .await
            .unwrap();
        let devices = ReadingStatsRepository::by_device(&db, user.id, window, StatsSort::Time)
            .await
            .unwrap();
        let formats = ReadingStatsRepository::by_format(&db, user.id, window, StatsSort::Time)
            .await
            .unwrap();

        assert_eq!(periods[0].books_finished, 1);
        assert_eq!(series[0].books_finished, 1);
        assert_eq!(devices[0].books_finished, 1);
        assert_eq!(formats[0].books_finished, 1);
    }

    /// The ranking key has to be applied in SQL because the limit is: sorting
    /// the returned rows would sort a set that was *selected* by a different
    /// measure, so the true leader by pages can be missing from it entirely.
    #[tokio::test]
    async fn the_ranking_key_decides_which_series_survive_the_limit() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let slow_read = create_book(&db, "Long Sitting", "cbz").await;
        let fast_read = create_book(&db, "Quick Pages", "cbz").await;

        insert(
            &db,
            user.id,
            slow_read.id,
            SessionSpec::measured("phone", 120, at(1, 9)).pages(5),
        )
        .await;
        insert(
            &db,
            user.id,
            fast_read.id,
            SessionSpec::measured("phone", 10, at(2, 9))
                .pages(400)
                .kind("completed"),
        )
        .await;

        let window = whole_of_june();
        let top = async |sort| {
            ReadingStatsRepository::by_series(&db, user.id, window, sort, 1)
                .await
                .unwrap()[0]
                .series_name
                .clone()
        };

        assert_eq!(top(StatsSort::Time).await, "Long Sitting");
        assert_eq!(top(StatsSort::Pages).await, "Quick Pages");
        assert_eq!(top(StatsSort::Completions).await, "Quick Pages");
    }

    /// `series.name` is the scanned directory name, which keeps suffixes like
    /// "(Digital)" so files still match. Every other surface displays
    /// `series_metadata.title`, and statistics must agree with them.
    #[tokio::test]
    async fn series_are_named_by_their_metadata_title() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book =
            create_book_titled(&db, "Prison School (Digital)", Some("Prison School"), "cbz").await;

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 30, at(1, 9)),
        )
        .await;

        let series =
            ReadingStatsRepository::by_series(&db, user.id, whole_of_june(), StatsSort::Time, 10)
                .await
                .unwrap();

        assert_eq!(series[0].series_name, "Prison School");
    }

    /// Metadata is created alongside every series today, but a missing row must
    /// leave the series in the statistics under its directory name rather than
    /// dropping it from the reader's history.
    #[tokio::test]
    async fn a_series_without_metadata_still_appears() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;
        series_metadata::Entity::delete_by_id(book.series_id)
            .exec(&db)
            .await
            .unwrap();

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 30, at(1, 9)),
        )
        .await;

        let series =
            ReadingStatsRepository::by_series(&db, user.id, whole_of_june(), StatsSort::Time, 10)
                .await
                .unwrap();

        assert_eq!(series.len(), 1);
        assert_eq!(series[0].series_name, "Berserk");
    }

    #[tokio::test]
    async fn the_series_limit_is_respected() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        for (name, minutes) in [("A", 90), ("B", 60), ("C", 30)] {
            let book = create_book(&db, name, "cbz").await;
            insert(
                &db,
                user.id,
                book.id,
                SessionSpec::measured("phone", minutes, at(1, 9)),
            )
            .await;
        }

        let series =
            ReadingStatsRepository::by_series(&db, user.id, whole_of_june(), StatsSort::Time, 2)
                .await
                .unwrap();

        assert_eq!(series.len(), 2);
        assert_eq!(series[0].series_name, "A");
        assert_eq!(series[1].series_name, "B");
    }

    #[tokio::test]
    async fn formats_are_grouped_and_ranked() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let comic = create_book(&db, "Berserk", "cbz").await;
        let ebook = create_book(&db, "Dune", "epub").await;

        insert(
            &db,
            user.id,
            comic.id,
            SessionSpec::measured("phone", 90, at(1, 9)),
        )
        .await;
        insert(
            &db,
            user.id,
            ebook.id,
            SessionSpec::measured("phone", 30, at(2, 9)),
        )
        .await;

        let formats =
            ReadingStatsRepository::by_format(&db, user.id, whole_of_june(), StatsSort::Time)
                .await
                .unwrap();

        assert_eq!(formats.len(), 2);
        assert_eq!(formats[0].format, "cbz");
        assert_eq!(formats[0].duration.measured_ms, 90 * MINUTE_MS);
        assert_eq!(formats[1].format, "epub");
    }

    /// Pages are summed independently of time, so a client that reports one and
    /// not the other still contributes what it knows.
    #[tokio::test]
    async fn pages_are_summed_independently_of_time() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        insert(
            &db,
            user.id,
            book.id,
            SessionSpec::measured("phone", 30, at(1, 9)).pages(42),
        )
        .await;

        let summary = ReadingStatsRepository::summary(&db, user.id, whole_of_june())
            .await
            .unwrap();

        assert_eq!(summary.pages_read, 42);
    }

    #[tokio::test]
    async fn row_count_reports_what_a_user_has_stored() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let book = create_book(&db, "Berserk", "cbz").await;

        for day in 1..=5 {
            insert(
                &db,
                user.id,
                book.id,
                SessionSpec::measured("phone", 10, at(day, 9)),
            )
            .await;
        }

        assert_eq!(
            ReadingStatsRepository::row_count(&db, user.id)
                .await
                .unwrap(),
            5
        );
    }
}

#[cfg(test)]
mod benchmarks {
    use super::tests_support::*;
    use super::*;
    use crate::test_helpers::setup_test_db;
    use chrono::Duration;

    /// How much a year of heavy reading actually costs, in rows and in query
    /// time, so the retention question is answered with a number.
    ///
    /// Run with
    /// `cargo test -p codex-db --lib a_year_of_reading -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "benchmark, not a correctness check"]
    async fn a_year_of_reading_costs_little() {
        let db = setup_test_db().await;
        let user = seed_user(&db, "heavy-reader").await;
        let books = seed_books(&db, 40).await;

        // Six sittings a day across three devices for a year: the shape of
        // someone reading several hours daily, after coalescing.
        let start = Utc::now() - Duration::days(365);
        let devices = ["phone", "ipad", "laptop"];
        let mut rows = 0usize;

        for day in 0..365i64 {
            for sitting in 0..6i64 {
                let began = start + Duration::days(day) + Duration::hours(sitting * 3);
                seed_session(
                    &db,
                    user,
                    books[(day as usize + sitting as usize) % books.len()],
                    devices[(sitting % 3) as usize],
                    began,
                    45,
                )
                .await;
                rows += 1;
            }
        }

        let stored = ReadingStatsRepository::row_count(&db, user).await.unwrap();
        let window = StatsWindow {
            from: start,
            to: Utc::now(),
        };

        let started = std::time::Instant::now();
        let summary = ReadingStatsRepository::summary(&db, user, window)
            .await
            .unwrap();
        let summary_ms = started.elapsed();

        let started = std::time::Instant::now();
        ReadingStatsRepository::by_period(&db, user, window, StatsGranularity::Day)
            .await
            .unwrap();
        let period_ms = started.elapsed();

        let started = std::time::Instant::now();
        ReadingStatsRepository::by_device(&db, user, window, StatsSort::Time)
            .await
            .unwrap();
        ReadingStatsRepository::by_series(&db, user, window, StatsSort::Time, 10)
            .await
            .unwrap();
        ReadingStatsRepository::by_format(&db, user, window, StatsSort::Time)
            .await
            .unwrap();
        let breakdowns_ms = started.elapsed();

        let whole_dashboard = summary_ms + period_ms + breakdowns_ms;

        println!("rows inserted:      {rows}");
        println!("rows stored:        {stored}");
        println!("summary:            {summary_ms:?}");
        println!("daily series:       {period_ms:?}");
        println!("three breakdowns:   {breakdowns_ms:?}");
        println!("whole dashboard:    {whole_dashboard:?}");
        println!(
            "time read reported: {} hours",
            summary.duration.total_ms() / 3_600_000
        );

        // The product target is 200ms. This bound is looser because
        // `--run-ignored all` runs benchmarks alongside the whole suite, where
        // a 200ms assertion measures contention rather than the queries. The
        // printed figures above are the ones to read; this only catches an
        // order-of-magnitude regression.
        assert!(
            whole_dashboard < std::time::Duration::from_secs(2),
            "a dashboard load took {whole_dashboard:?} against {stored} sessions"
        );
    }
}

/// Seeding helpers shared by the tests and the benchmark.
#[cfg(test)]
mod tests_support {
    use crate::entities::{books, reading_sessions, users};
    use crate::repositories::{
        BookRepository, LibraryRepository, SeriesRepository, UserRepository,
    };
    use chrono::{DateTime, Duration, Utc};
    use codex_models::ScanningStrategy;
    use codex_utils::password;
    use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
    use uuid::Uuid;

    pub async fn seed_user(db: &DatabaseConnection, name: &str) -> Uuid {
        let password_hash = password::hash_password("password").unwrap();
        UserRepository::create(
            db,
            &users::Model {
                id: Uuid::new_v4(),
                username: name.to_string(),
                email: format!("{name}@example.com"),
                password_hash,
                role: "admin".to_string(),
                is_active: true,
                email_verified: false,
                permissions: serde_json::json!([]),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                last_login_at: None,
            },
        )
        .await
        .unwrap()
        .id
    }

    pub async fn seed_books(db: &DatabaseConnection, count: usize) -> Vec<Uuid> {
        let library = LibraryRepository::create(db, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = SeriesRepository::create(db, library.id, "Bench Series", None)
            .await
            .unwrap();

        let mut ids = Vec::with_capacity(count);
        for _ in 0..count {
            let book = books::Model {
                id: Uuid::new_v4(),
                series_id: series.id,
                library_id: library.id,
                path: format!("/lib/{}.cbz", Uuid::new_v4()),
                file_name: "book.cbz".to_string(),
                file_size: 1024,
                file_hash: format!("hash_{}", Uuid::new_v4()),
                partial_hash: String::new(),
                format: "cbz".to_string(),
                page_count: 200,
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
            ids.push(BookRepository::create(db, &book, None).await.unwrap().id);
        }
        ids
    }

    pub async fn seed_session(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
        device: &str,
        began: DateTime<Utc>,
        minutes: i64,
    ) {
        reading_sessions::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            book_id: Set(book_id),
            device_id: Set(device.to_string()),
            device_name: Set(Some(device.to_string())),
            pass: Set(1),
            kind: Set("progress".to_string()),
            to_page: Set(Some(42)),
            to_percentage: Set(None),
            r2_progression: Set(None),
            active_duration_ms: Set(Some(minutes * 60_000)),
            duration_source: Set("measured".to_string()),
            pages_read: Set(Some(20)),
            client_started_at: Set(began),
            client_ended_at: Set(began + Duration::minutes(minutes)),
            server_recorded_at: Set(began + Duration::minutes(minutes)),
        }
        .insert(db)
        .await
        .unwrap();
    }
}
