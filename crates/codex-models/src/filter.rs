//! Filter operator types shared between the api DTOs and the services
//! filter engine. Repositories and services need to speak this vocabulary
//! without depending on the api layer.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Operators for string and equality comparisons
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", rename_all = "camelCase")]
pub enum FieldOperator {
    /// Exact match
    Is { value: String },
    /// Not equal
    IsNot { value: String },
    /// Field is null/empty
    IsNull,
    /// Field is not null/empty
    IsNotNull,
    /// String contains (case-insensitive)
    Contains { value: String },
    /// String does not contain (case-insensitive)
    DoesNotContain { value: String },
    /// String starts with (case-insensitive)
    BeginsWith { value: String },
    /// String ends with (case-insensitive)
    EndsWith { value: String },
}

/// Operators for UUID comparisons (library_id, series_id, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", rename_all = "camelCase")]
pub enum UuidOperator {
    /// Exact match
    Is { value: Uuid },
    /// Not equal
    IsNot { value: Uuid },
}

/// Operators for boolean comparisons
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", rename_all = "camelCase")]
pub enum BoolOperator {
    /// Is true
    IsTrue,
    /// Is false
    IsFalse,
}

/// Operators for numeric comparisons (year, page count, etc.).
///
/// Values are deserialized as `i64` so the same operator can target either
/// `INTEGER` or `BIGINT` columns. Implementations downcast as needed.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", rename_all = "camelCase")]
pub enum NumberOperator {
    /// Equal to value
    Eq { value: i64 },
    /// Not equal to value
    Ne { value: i64 },
    /// Greater than value (strict)
    Gt { value: i64 },
    /// Greater than or equal to value
    Gte { value: i64 },
    /// Less than value (strict)
    Lt { value: i64 },
    /// Less than or equal to value
    Lte { value: i64 },
    /// Inclusive range, `min <= field <= max`. Either bound may be omitted to
    /// model open-ended ranges (e.g. "year >= 2000").
    Between {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<i64>,
    },
    /// Field is null
    IsNull,
    /// Field is not null
    IsNotNull,
}

/// Operators for date/timestamp comparisons.
///
/// Values are RFC 3339 / ISO 8601 timestamps. For range comparisons either
/// bound may be omitted to express an open-ended range.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", rename_all = "camelCase")]
pub enum DateOperator {
    /// Strictly after the given timestamp
    After { value: DateTime<Utc> },
    /// Strictly before the given timestamp
    Before { value: DateTime<Utc> },
    /// On or after the given timestamp
    OnOrAfter { value: DateTime<Utc> },
    /// On or before the given timestamp
    OnOrBefore { value: DateTime<Utc> },
    /// Inclusive between range. Either bound may be omitted.
    Between {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        start: Option<DateTime<Utc>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        end: Option<DateTime<Utc>>,
    },
    /// Field is null
    IsNull,
    /// Field is not null
    IsNotNull,
}

/// Series-level search conditions
///
/// Conditions can be composed using `allOf` (AND) and `anyOf` (OR).
/// Uses untagged enum for cleaner JSON without explicit type field.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum SeriesCondition {
    /// All conditions must match (AND)
    AllOf {
        #[serde(rename = "allOf")]
        #[schema(no_recursion)]
        all_of: Vec<SeriesCondition>,
    },
    /// Any condition must match (OR)
    AnyOf {
        #[serde(rename = "anyOf")]
        #[schema(no_recursion)]
        any_of: Vec<SeriesCondition>,
    },
    /// Filter by library ID
    LibraryId {
        #[serde(rename = "libraryId")]
        library_id: UuidOperator,
    },
    /// Filter by genre name
    Genre { genre: FieldOperator },
    /// Filter by tag name
    Tag { tag: FieldOperator },
    /// Filter by series status (ongoing, ended, hiatus, etc.)
    Status { status: FieldOperator },
    /// Filter by publisher
    Publisher { publisher: FieldOperator },
    /// Filter by language
    Language { language: FieldOperator },
    /// Filter by series title (`series_metadata.title`)
    Title { title: FieldOperator },
    /// Filter by series title_sort field (used for alphabetical filtering)
    TitleSort {
        #[serde(rename = "titleSort")]
        title_sort: FieldOperator,
    },
    /// Filter by read status (unread, in_progress, read)
    ReadStatus {
        #[serde(rename = "readStatus")]
        read_status: FieldOperator,
    },
    /// Filter by sharing tag name
    SharingTag {
        #[serde(rename = "sharingTag")]
        sharing_tag: FieldOperator,
    },
    /// Filter by series completion status (complete/incomplete based on book_count vs total_volume_count)
    Completion { completion: BoolOperator },
    /// Filter by whether the series has an external source ID linked
    HasExternalSourceId {
        #[serde(rename = "hasExternalSourceId")]
        has_external_source_id: BoolOperator,
    },
    /// Filter by whether the series has a rating from the current user
    HasUserRating {
        #[serde(rename = "hasUserRating")]
        has_user_rating: BoolOperator,
    },
    /// Filter by whether release tracking is enabled for the series.
    ///
    /// `IsTrue` returns only series whose `series_tracking.tracked` flag is
    /// `true`. `IsFalse` returns everything else, including series with no
    /// `series_tracking` row at all (the common case for a fresh library).
    IsTracked {
        #[serde(rename = "isTracked")]
        is_tracked: BoolOperator,
    },
    /// Filter by whether the series belongs to at least one collection.
    ///
    /// `IsTrue` returns only series that appear in one or more
    /// `collection_series` rows. `IsFalse` returns everything else, including
    /// series that belong to no collection at all (the common case).
    InCollection {
        #[serde(rename = "inCollection")]
        in_collection: BoolOperator,
    },
    /// Filter by release year (from `series_metadata.year`).
    Year { year: NumberOperator },
    /// Filter by author (substring match on `series_metadata.authors_json`).
    ///
    /// The match is performed against the raw JSON text. It is tolerant of
    /// both string-list and object-list shapes but may incidentally match
    /// other fields (e.g. `role`); callers wanting strict matching should
    /// pre-quote the value.
    Author { author: FieldOperator },
    /// Filter by the series' folder path (`series.path`). Useful for matching
    /// series under a given directory.
    Path { path: FieldOperator },
    /// Filter by date the series was added to the library
    /// (`series.created_at`).
    DateAdded {
        #[serde(rename = "dateAdded")]
        date_added: DateOperator,
    },
}

/// Book-level search conditions
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum BookCondition {
    /// All conditions must match (AND)
    AllOf {
        #[serde(rename = "allOf")]
        #[schema(no_recursion)]
        all_of: Vec<BookCondition>,
    },
    /// Any condition must match (OR)
    AnyOf {
        #[serde(rename = "anyOf")]
        #[schema(no_recursion)]
        any_of: Vec<BookCondition>,
    },
    /// Filter by library ID
    LibraryId {
        #[serde(rename = "libraryId")]
        library_id: UuidOperator,
    },
    /// Filter by series ID
    SeriesId {
        #[serde(rename = "seriesId")]
        series_id: UuidOperator,
    },
    /// Filter by genre name (from parent series)
    Genre { genre: FieldOperator },
    /// Filter by tag name (from parent series)
    Tag { tag: FieldOperator },
    /// Filter by book title (`book_metadata.title`)
    Title { title: FieldOperator },
    /// Filter by book title_sort field (`book_metadata.title_sort`,
    /// used for alphabetical filtering)
    TitleSort {
        #[serde(rename = "titleSort")]
        title_sort: FieldOperator,
    },
    /// Filter by read status (unread, in_progress, read)
    ReadStatus {
        #[serde(rename = "readStatus")]
        read_status: FieldOperator,
    },
    /// Filter by books with analysis errors
    HasError {
        #[serde(rename = "hasError")]
        has_error: BoolOperator,
    },
    /// Filter by whether the book belongs to at least one read list.
    ///
    /// `IsTrue` returns only books that appear in one or more
    /// `read_list_books` rows. `IsFalse` returns everything else, including
    /// books that belong to no read list at all (the common case).
    InReadList {
        #[serde(rename = "inReadList")]
        in_read_list: BoolOperator,
    },
    /// Filter by book type (comic, manga, novel, etc.)
    BookType {
        #[serde(rename = "bookType")]
        book_type: FieldOperator,
    },
    /// Filter by the book's file path (`books.path`). Useful for matching
    /// books under a given directory or with a specific filename fragment.
    Path { path: FieldOperator },
    /// Filter by file format (`books.format`, e.g. `cbz`, `cbr`, `epub`,
    /// `pdf`). Distinct from `BookType`, which classifies content (comic,
    /// manga, novel, ...).
    Format { format: FieldOperator },
    /// Filter by page count (`books.page_count`).
    PageCount {
        #[serde(rename = "pageCount")]
        page_count: NumberOperator,
    },
    /// Filter by date the book was added to the library (`books.created_at`).
    DateAdded {
        #[serde(rename = "dateAdded")]
        date_added: DateOperator,
    },
}
