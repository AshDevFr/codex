//! DTOs for reading statistics.
//!
//! One response carries the whole dashboard, because every panel of it answers
//! the same question over the same window and splitting them into five requests
//! would only mean five chances for the window to drift between panels.

use chrono::{DateTime, Utc};
use codex_db::repositories::{
    DurationBreakdown, ReadingByDevice, ReadingByFormat, ReadingBySeries, ReadingCoverage,
    ReadingPeriod, ReadingSummary, StatsGranularity, StatsSort,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// How finely to bucket the time series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ReadingStatsGranularity {
    Day,
    Week,
    Month,
}

impl From<ReadingStatsGranularity> for StatsGranularity {
    fn from(value: ReadingStatsGranularity) -> Self {
        match value {
            ReadingStatsGranularity::Day => Self::Day,
            ReadingStatsGranularity::Week => Self::Week,
            ReadingStatsGranularity::Month => Self::Month,
        }
    }
}

/// Which measure the series, device and format breakdowns are ranked by.
///
/// The series breakdown is limited, so this decides which rows survive that
/// limit, not merely what order they arrive in. Ranking by pages client-side
/// would sort a top-N that was chosen by time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ReadingStatsSort {
    Time,
    Pages,
    /// Books finished. The only measure a library predating session tracking
    /// can answer, since backfilled rows carry no duration and no page count.
    Completions,
}

impl From<ReadingStatsSort> for StatsSort {
    fn from(value: ReadingStatsSort) -> Self {
        match value {
            ReadingStatsSort::Time => Self::Time,
            ReadingStatsSort::Pages => Self::Pages,
            ReadingStatsSort::Completions => Self::Completions,
        }
    }
}

/// Query parameters for the statistics endpoint.
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ReadingStatsQuery {
    /// Start of the window. Defaults to 90 days ago.
    pub from: Option<DateTime<Utc>>,
    /// End of the window, exclusive. Defaults to now.
    pub to: Option<DateTime<Utc>>,
    /// Bucket size for the time series. Defaults to `day`.
    pub granularity: Option<ReadingStatsGranularity>,
    /// How many series to return. Defaults to 10, capped at 50.
    pub series_limit: Option<u64>,
    /// Ranking key for the breakdowns. Defaults to `time`.
    pub sort: Option<ReadingStatsSort>,
}

/// Reading time split by how it was arrived at.
///
/// Deliberately two numbers rather than one. Time from the compatibility
/// surfaces is reconstructed from the gaps between their writes: it undercounts
/// and cannot see reading done from a downloaded book at all. Presenting a
/// combined figure is fine; hiding that part of it is an estimate is not.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DurationBreakdownDto {
    /// Milliseconds reported by a client that measured its own reading.
    #[schema(example = 5_400_000_i64)]
    pub measured_ms: i64,
    /// Milliseconds reconstructed server-side. An underestimate.
    #[schema(example = 900_000_i64)]
    pub inferred_ms: i64,
    /// Convenience sum of the two, so clients do not each reimplement it.
    #[schema(example = 6_300_000_i64)]
    pub total_ms: i64,
}

impl From<DurationBreakdown> for DurationBreakdownDto {
    fn from(value: DurationBreakdown) -> Self {
        Self {
            total_ms: value.total_ms(),
            measured_ms: value.measured_ms,
            inferred_ms: value.inferred_ms,
        }
    }
}

/// Headline totals for the window.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadingSummaryDto {
    pub duration: DurationBreakdownDto,
    #[schema(example = 1240)]
    pub pages_read: i64,
    /// Distinct sittings, after adjacent writes were merged.
    #[schema(example = 48)]
    pub sessions: i64,
    #[schema(example = 12)]
    pub books: i64,
    /// Books finished in the window. Unlike time and pages, this is populated
    /// for reading that predates session tracking.
    #[schema(example = 5)]
    pub books_finished: i64,
    /// Sittings whose client could report no time at all. A large number here
    /// explains a total that looks lower than the reading felt.
    #[schema(example = 3)]
    pub sessions_without_duration: i64,
}

impl From<ReadingSummary> for ReadingSummaryDto {
    fn from(value: ReadingSummary) -> Self {
        Self {
            duration: value.duration.into(),
            pages_read: value.pages_read,
            sessions: value.sessions,
            books: value.books,
            books_finished: value.books_finished,
            sessions_without_duration: value.sessions_without_duration,
        }
    }
}

/// One bucket of the time series.
///
/// Buckets with no reading are absent rather than zero: a client knows the
/// window it asked for and can fill the gaps, and a quiet year would otherwise
/// be mostly padding.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadingPeriodDto {
    /// ISO date of the bucket's start. Weeks start on Monday.
    #[schema(example = "2026-06-01")]
    pub bucket: String,
    pub duration: DurationBreakdownDto,
    #[schema(example = 120)]
    pub pages_read: i64,
    #[schema(example = 4)]
    pub sessions: i64,
    #[schema(example = 1)]
    pub books_finished: i64,
}

impl From<ReadingPeriod> for ReadingPeriodDto {
    fn from(value: ReadingPeriod) -> Self {
        Self {
            bucket: value.bucket,
            duration: value.duration.into(),
            pages_read: value.pages_read,
            sessions: value.sessions,
            books_finished: value.books_finished,
        }
    }
}

/// Totals for one device, most-read first.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadingByDeviceDto {
    pub device_id: String,
    /// Friendly name where the client sent one, or the API key's label.
    #[schema(example = "Ash's iPhone")]
    pub device_name: Option<String>,
    pub duration: DurationBreakdownDto,
    pub pages_read: i64,
    pub sessions: i64,
    pub books_finished: i64,
    pub last_read_at: DateTime<Utc>,
}

impl From<ReadingByDevice> for ReadingByDeviceDto {
    fn from(value: ReadingByDevice) -> Self {
        Self {
            device_id: value.device_id,
            device_name: value.device_name,
            duration: value.duration.into(),
            pages_read: value.pages_read,
            sessions: value.sessions,
            books_finished: value.books_finished,
            last_read_at: value.last_read_at,
        }
    }
}

/// Totals for one series, most-read first.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadingBySeriesDto {
    pub series_id: Uuid,
    #[schema(example = "Berserk")]
    pub series_name: String,
    pub duration: DurationBreakdownDto,
    pub pages_read: i64,
    pub sessions: i64,
    /// Distinct books of the series read in the window.
    pub books: i64,
    pub books_finished: i64,
}

impl From<ReadingBySeries> for ReadingBySeriesDto {
    fn from(value: ReadingBySeries) -> Self {
        Self {
            series_id: value.series_id,
            series_name: value.series_name,
            duration: value.duration.into(),
            pages_read: value.pages_read,
            sessions: value.sessions,
            books: value.books,
            books_finished: value.books_finished,
        }
    }
}

/// Totals for one file format.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadingByFormatDto {
    #[schema(example = "cbz")]
    pub format: String,
    pub duration: DurationBreakdownDto,
    pub pages_read: i64,
    pub sessions: i64,
    pub books_finished: i64,
}

impl From<ReadingByFormat> for ReadingByFormatDto {
    fn from(value: ReadingByFormat) -> Self {
        Self {
            format: value.format,
            duration: value.duration.into(),
            pages_read: value.pages_read,
            sessions: value.sessions,
            books_finished: value.books_finished,
        }
    }
}

/// The span a reader's history covers, independent of any window.
///
/// Its own endpoint rather than a field on the statistics response: that
/// response describes a window, and these two dates deliberately ignore it. A
/// client uses them to decide which years it can offer, and they change at most
/// once a day, so they are worth caching far longer than the statistics are.
///
/// Both are null for a reader who has never read anything.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadingCoverageDto {
    pub first_read_at: Option<DateTime<Utc>>,
    pub last_read_at: Option<DateTime<Utc>>,
}

impl From<ReadingCoverage> for ReadingCoverageDto {
    fn from(value: ReadingCoverage) -> Self {
        Self {
            first_read_at: value.first_read_at,
            last_read_at: value.last_read_at,
        }
    }
}

/// Everything the reading dashboard shows, over one window.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadingStatsResponse {
    /// The window actually used, after defaults were applied.
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub granularity: ReadingStatsGranularity,
    pub summary: ReadingSummaryDto,
    pub periods: Vec<ReadingPeriodDto>,
    pub devices: Vec<ReadingByDeviceDto>,
    pub series: Vec<ReadingBySeriesDto>,
    pub formats: Vec<ReadingByFormatDto>,
}
