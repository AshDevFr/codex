use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::common::PaginatedResponse;
use super::read_progress::ReadProgressResponse;
use super::series::SortDirection;

/// Sort field options for book list queries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BookSortField {
    /// Compound sort: series name alphabetically, then books by number within series
    /// This is the "reading order" sort
    Series,
    /// Sort by book title
    #[default]
    Title,
    /// Sort by date added to library
    DateAdded,
    /// Sort by release date
    ReleaseDate,
    /// Sort by chapter/book number
    ChapterNumber,
    /// Sort by file size
    FileSize,
    /// Sort by filename
    Filename,
    /// Sort by page count
    PageCount,
}

impl fmt::Display for BookSortField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BookSortField::Series => write!(f, "series"),
            BookSortField::Title => write!(f, "title"),
            BookSortField::DateAdded => write!(f, "created_at"),
            BookSortField::ReleaseDate => write!(f, "release_date"),
            BookSortField::ChapterNumber => write!(f, "chapter_number"),
            BookSortField::FileSize => write!(f, "file_size"),
            BookSortField::Filename => write!(f, "filename"),
            BookSortField::PageCount => write!(f, "page_count"),
        }
    }
}

impl FromStr for BookSortField {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "series" => Ok(BookSortField::Series),
            "title" => Ok(BookSortField::Title),
            "created_at" | "date_added" => Ok(BookSortField::DateAdded),
            "release_date" => Ok(BookSortField::ReleaseDate),
            "chapter_number" | "number" => Ok(BookSortField::ChapterNumber),
            "file_size" => Ok(BookSortField::FileSize),
            "filename" => Ok(BookSortField::Filename),
            "page_count" => Ok(BookSortField::PageCount),
            _ => Err(format!("Invalid sort field: {}", s)),
        }
    }
}

/// Parsed sort parameter for book queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BookSortParam {
    pub field: BookSortField,
    pub direction: SortDirection,
}

impl Default for BookSortParam {
    fn default() -> Self {
        Self {
            field: BookSortField::Title,
            direction: SortDirection::Asc,
        }
    }
}

impl BookSortParam {
    /// Parse from "field,direction" format (e.g., "title,asc")
    pub fn parse(s: &str) -> Self {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 2 {
            return Self::default();
        }

        let field = BookSortField::from_str(parts[0]).unwrap_or_default();
        let direction = SortDirection::from_str(parts[1]).unwrap_or_default();

        Self { field, direction }
    }
}

impl fmt::Display for BookSortParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{}", self.field, self.direction)
    }
}

/// Book data transfer object
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookDto {
    /// Book unique identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub id: uuid::Uuid,

    /// Library this book belongs to
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub library_id: uuid::Uuid,

    /// Name of the library
    #[schema(example = "Comics")]
    pub library_name: String,

    /// Series this book belongs to
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub series_id: uuid::Uuid,

    /// Name of the series
    #[schema(example = "Batman: Year One")]
    pub series_name: String,

    /// Book title
    #[schema(example = "Batman: Year One #1")]
    pub title: String,

    /// Title used for sorting (title_sort field)
    #[schema(example = "batman year one 001")]
    pub title_sort: Option<String>,

    /// Filesystem path to the book file
    #[schema(example = "/media/comics/Batman/Batman - Year One 001.cbz")]
    pub file_path: String,

    /// File format (cbz, cbr, epub, pdf)
    #[schema(example = "cbz")]
    pub file_format: String,

    /// File size in bytes
    #[schema(example = 52428800)]
    pub file_size: i64,

    /// File hash for deduplication
    #[schema(example = "a1b2c3d4e5f6g7h8i9j0")]
    pub file_hash: String,

    /// Number of pages in the book
    #[schema(example = 32)]
    pub page_count: i32,

    /// Book number within the series
    #[schema(example = 1)]
    pub number: Option<i32>,

    /// When the book was added to the library
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the book was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,

    /// User's read progress for this book
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_progress: Option<ReadProgressResponse>,

    /// Error message if book analysis failed
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Failed to parse CBZ: invalid archive")]
    pub analysis_error: Option<String>,

    /// Whether the book has been soft-deleted
    #[schema(example = false)]
    pub deleted: bool,

    /// Effective reading direction (from series metadata, or library default if not set)
    /// Values: ltr, rtl, ttb or webtoon
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "ltr")]
    pub reading_direction: Option<String>,
}

/// Book list response
pub type BookListResponse = PaginatedResponse<BookDto>;

/// Detailed book response with metadata
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookDetailResponse {
    /// The book data
    pub book: BookDto,

    /// Optional metadata from ComicInfo.xml or similar
    pub metadata: Option<BookMetadataDto>,
}

/// Book metadata DTO
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookMetadataDto {
    /// Metadata record ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440003")]
    pub id: uuid::Uuid,

    /// Associated book ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub book_id: uuid::Uuid,

    /// Book title from metadata
    #[schema(example = "Batman: Year One #1")]
    pub title: Option<String>,

    /// Series name from metadata
    #[schema(example = "Batman: Year One")]
    pub series: Option<String>,

    /// Issue/chapter number from metadata
    #[schema(example = "1")]
    pub number: Option<String>,

    /// Book summary/description
    #[schema(
        example = "Bruce Wayne returns to Gotham City after years abroad to begin his war on crime."
    )]
    pub summary: Option<String>,

    /// Publisher name
    #[schema(example = "DC Comics")]
    pub publisher: Option<String>,

    /// Imprint name
    #[schema(example = "DC Black Label")]
    pub imprint: Option<String>,

    /// Genre
    #[schema(example = "Superhero")]
    pub genre: Option<String>,

    /// Page count from metadata
    #[schema(example = 32)]
    pub page_count: Option<i32>,

    /// ISO language code
    #[schema(example = "en")]
    pub language_iso: Option<String>,

    /// Release/publication date
    #[schema(example = "1987-02-01T00:00:00Z")]
    pub release_date: Option<DateTime<Utc>>,

    /// Writers/authors
    #[schema(example = json!(["Frank Miller"]))]
    pub writers: Vec<String>,

    /// Pencillers (line artists)
    #[schema(example = json!(["David Mazzucchelli"]))]
    pub pencillers: Vec<String>,

    /// Inkers
    #[schema(example = json!(["David Mazzucchelli"]))]
    pub inkers: Vec<String>,

    /// Colorists
    #[schema(example = json!(["Richmond Lewis"]))]
    pub colorists: Vec<String>,

    /// Letterers
    #[schema(example = json!(["Todd Klein"]))]
    pub letterers: Vec<String>,

    /// Cover artists
    #[schema(example = json!(["David Mazzucchelli"]))]
    pub cover_artists: Vec<String>,

    /// Editors
    #[schema(example = json!(["Dennis O'Neil"]))]
    pub editors: Vec<String>,
}

/// PUT request for full replacement of book metadata
///
/// All metadata fields will be replaced with the values in this request.
/// Omitting a field (or setting it to null) will clear that field.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceBookMetadataRequest {
    /// Book summary/description
    #[schema(example = "Bruce Wayne returns to Gotham City after years abroad.")]
    pub summary: Option<String>,

    /// Writer(s) - comma-separated if multiple
    #[schema(example = "Frank Miller")]
    pub writer: Option<String>,

    /// Penciller(s) - comma-separated if multiple
    #[schema(example = "David Mazzucchelli")]
    pub penciller: Option<String>,

    /// Inker(s) - comma-separated if multiple
    #[schema(example = "David Mazzucchelli")]
    pub inker: Option<String>,

    /// Colorist(s) - comma-separated if multiple
    #[schema(example = "Richmond Lewis")]
    pub colorist: Option<String>,

    /// Letterer(s) - comma-separated if multiple
    #[schema(example = "Todd Klein")]
    pub letterer: Option<String>,

    /// Cover artist(s) - comma-separated if multiple
    #[schema(example = "David Mazzucchelli")]
    pub cover_artist: Option<String>,

    /// Editor(s) - comma-separated if multiple
    #[schema(example = "Dennis O'Neil")]
    pub editor: Option<String>,

    /// Publisher name
    #[schema(example = "DC Comics")]
    pub publisher: Option<String>,

    /// Imprint name
    #[schema(example = "DC Black Label")]
    pub imprint: Option<String>,

    /// Genre
    #[schema(example = "Superhero")]
    pub genre: Option<String>,

    /// Web URL for more information
    #[schema(example = "https://dc.com/batman-year-one")]
    pub web: Option<String>,

    /// ISO language code
    #[schema(example = "en")]
    pub language_iso: Option<String>,

    /// Format details
    #[schema(example = "Trade Paperback")]
    pub format_detail: Option<String>,

    /// Whether the book is black and white
    #[schema(example = false)]
    pub black_and_white: Option<bool>,

    /// Whether the book is manga format
    #[schema(example = false)]
    pub manga: Option<bool>,

    /// Publication year
    #[schema(example = 1987)]
    pub year: Option<i32>,

    /// Publication month (1-12)
    #[schema(example = 2)]
    pub month: Option<i32>,

    /// Publication day (1-31)
    #[schema(example = 1)]
    pub day: Option<i32>,

    /// Volume number
    #[schema(example = 1)]
    pub volume: Option<i32>,

    /// Total count in series
    #[schema(example = 4)]
    pub count: Option<i32>,

    /// ISBN(s) - comma-separated if multiple
    #[schema(example = "978-1401207526")]
    pub isbns: Option<String>,
}

/// PATCH request for partial update of book metadata
///
/// Only provided fields will be updated. Absent fields are unchanged.
/// Explicitly null fields will be cleared.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatchBookMetadataRequest {
    /// Book summary/description
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "Bruce Wayne returns to Gotham City.", nullable = true)]
    pub summary: super::patch::PatchValue<String>,

    /// Writer(s) - comma-separated if multiple
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "Frank Miller", nullable = true)]
    pub writer: super::patch::PatchValue<String>,

    /// Penciller(s) - comma-separated if multiple
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "David Mazzucchelli", nullable = true)]
    pub penciller: super::patch::PatchValue<String>,

    /// Inker(s) - comma-separated if multiple
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "David Mazzucchelli", nullable = true)]
    pub inker: super::patch::PatchValue<String>,

    /// Colorist(s) - comma-separated if multiple
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "Richmond Lewis", nullable = true)]
    pub colorist: super::patch::PatchValue<String>,

    /// Letterer(s) - comma-separated if multiple
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "Todd Klein", nullable = true)]
    pub letterer: super::patch::PatchValue<String>,

    /// Cover artist(s) - comma-separated if multiple
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "David Mazzucchelli", nullable = true)]
    pub cover_artist: super::patch::PatchValue<String>,

    /// Editor(s) - comma-separated if multiple
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "Dennis O'Neil", nullable = true)]
    pub editor: super::patch::PatchValue<String>,

    /// Publisher name
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "DC Comics", nullable = true)]
    pub publisher: super::patch::PatchValue<String>,

    /// Imprint name
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "DC Black Label", nullable = true)]
    pub imprint: super::patch::PatchValue<String>,

    /// Genre
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "Superhero", nullable = true)]
    pub genre: super::patch::PatchValue<String>,

    /// Web URL for more information
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "https://dc.com/batman-year-one", nullable = true)]
    pub web: super::patch::PatchValue<String>,

    /// ISO language code
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "en", nullable = true)]
    pub language_iso: super::patch::PatchValue<String>,

    /// Format details
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "Trade Paperback", nullable = true)]
    pub format_detail: super::patch::PatchValue<String>,

    /// Whether the book is black and white
    #[serde(default)]
    #[schema(value_type = Option<bool>, example = false, nullable = true)]
    pub black_and_white: super::patch::PatchValue<bool>,

    /// Whether the book is manga format
    #[serde(default)]
    #[schema(value_type = Option<bool>, example = false, nullable = true)]
    pub manga: super::patch::PatchValue<bool>,

    /// Publication year
    #[serde(default)]
    #[schema(value_type = Option<i32>, example = 1987, nullable = true)]
    pub year: super::patch::PatchValue<i32>,

    /// Publication month (1-12)
    #[serde(default)]
    #[schema(value_type = Option<i32>, example = 2, nullable = true)]
    pub month: super::patch::PatchValue<i32>,

    /// Publication day (1-31)
    #[serde(default)]
    #[schema(value_type = Option<i32>, example = 1, nullable = true)]
    pub day: super::patch::PatchValue<i32>,

    /// Volume number
    #[serde(default)]
    #[schema(value_type = Option<i32>, example = 1, nullable = true)]
    pub volume: super::patch::PatchValue<i32>,

    /// Total count in series
    #[serde(default)]
    #[schema(value_type = Option<i32>, example = 4, nullable = true)]
    pub count: super::patch::PatchValue<i32>,

    /// ISBN(s) - comma-separated if multiple
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "978-1401207526", nullable = true)]
    pub isbns: super::patch::PatchValue<String>,
}

/// Response containing book metadata
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookMetadataResponse {
    /// Book ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub book_id: uuid::Uuid,

    /// Book summary/description
    #[schema(example = "Bruce Wayne returns to Gotham City.")]
    pub summary: Option<String>,

    /// Writer(s)
    #[schema(example = "Frank Miller")]
    pub writer: Option<String>,

    /// Penciller(s)
    #[schema(example = "David Mazzucchelli")]
    pub penciller: Option<String>,

    /// Inker(s)
    #[schema(example = "David Mazzucchelli")]
    pub inker: Option<String>,

    /// Colorist(s)
    #[schema(example = "Richmond Lewis")]
    pub colorist: Option<String>,

    /// Letterer(s)
    #[schema(example = "Todd Klein")]
    pub letterer: Option<String>,

    /// Cover artist(s)
    #[schema(example = "David Mazzucchelli")]
    pub cover_artist: Option<String>,

    /// Editor(s)
    #[schema(example = "Dennis O'Neil")]
    pub editor: Option<String>,

    /// Publisher name
    #[schema(example = "DC Comics")]
    pub publisher: Option<String>,

    /// Imprint name
    #[schema(example = "DC Black Label")]
    pub imprint: Option<String>,

    /// Genre
    #[schema(example = "Superhero")]
    pub genre: Option<String>,

    /// Web URL
    #[schema(example = "https://dc.com/batman-year-one")]
    pub web: Option<String>,

    /// ISO language code
    #[schema(example = "en")]
    pub language_iso: Option<String>,

    /// Format details
    #[schema(example = "Trade Paperback")]
    pub format_detail: Option<String>,

    /// Whether the book is black and white
    #[schema(example = false)]
    pub black_and_white: Option<bool>,

    /// Whether the book is manga format
    #[schema(example = false)]
    pub manga: Option<bool>,

    /// Publication year
    #[schema(example = 1987)]
    pub year: Option<i32>,

    /// Publication month (1-12)
    #[schema(example = 2)]
    pub month: Option<i32>,

    /// Publication day (1-31)
    #[schema(example = 1)]
    pub day: Option<i32>,

    /// Volume number
    #[schema(example = 1)]
    pub volume: Option<i32>,

    /// Total count in series
    #[schema(example = 4)]
    pub count: Option<i32>,

    /// ISBN(s)
    #[schema(example = "978-1401207526")]
    pub isbns: Option<String>,

    /// Last update timestamp
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Book metadata lock states
///
/// Indicates which metadata fields are locked (protected from automatic updates).
/// When a field is locked, the scanner will not overwrite user-edited values.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookMetadataLocks {
    /// Whether summary is locked
    #[schema(example = false)]
    pub summary_lock: bool,

    /// Whether writer is locked
    #[schema(example = false)]
    pub writer_lock: bool,

    /// Whether penciller is locked
    #[schema(example = false)]
    pub penciller_lock: bool,

    /// Whether inker is locked
    #[schema(example = false)]
    pub inker_lock: bool,

    /// Whether colorist is locked
    #[schema(example = false)]
    pub colorist_lock: bool,

    /// Whether letterer is locked
    #[schema(example = false)]
    pub letterer_lock: bool,

    /// Whether cover artist is locked
    #[schema(example = false)]
    pub cover_artist_lock: bool,

    /// Whether editor is locked
    #[schema(example = false)]
    pub editor_lock: bool,

    /// Whether publisher is locked
    #[schema(example = true)]
    pub publisher_lock: bool,

    /// Whether imprint is locked
    #[schema(example = false)]
    pub imprint_lock: bool,

    /// Whether genre is locked
    #[schema(example = false)]
    pub genre_lock: bool,

    /// Whether web URL is locked
    #[schema(example = false)]
    pub web_lock: bool,

    /// Whether language_iso is locked
    #[schema(example = false)]
    pub language_iso_lock: bool,

    /// Whether format_detail is locked
    #[schema(example = false)]
    pub format_detail_lock: bool,

    /// Whether black_and_white is locked
    #[schema(example = false)]
    pub black_and_white_lock: bool,

    /// Whether manga is locked
    #[schema(example = false)]
    pub manga_lock: bool,

    /// Whether year is locked
    #[schema(example = true)]
    pub year_lock: bool,

    /// Whether month is locked
    #[schema(example = false)]
    pub month_lock: bool,

    /// Whether day is locked
    #[schema(example = false)]
    pub day_lock: bool,

    /// Whether volume is locked
    #[schema(example = false)]
    pub volume_lock: bool,

    /// Whether count is locked
    #[schema(example = false)]
    pub count_lock: bool,

    /// Whether isbns is locked
    #[schema(example = false)]
    pub isbns_lock: bool,
}

/// Request to update book metadata locks
///
/// All fields are optional. Only provided fields will be updated.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBookMetadataLocksRequest {
    /// Whether to lock summary
    #[schema(example = true)]
    pub summary_lock: Option<bool>,

    /// Whether to lock writer
    pub writer_lock: Option<bool>,

    /// Whether to lock penciller
    pub penciller_lock: Option<bool>,

    /// Whether to lock inker
    pub inker_lock: Option<bool>,

    /// Whether to lock colorist
    pub colorist_lock: Option<bool>,

    /// Whether to lock letterer
    pub letterer_lock: Option<bool>,

    /// Whether to lock cover artist
    pub cover_artist_lock: Option<bool>,

    /// Whether to lock editor
    pub editor_lock: Option<bool>,

    /// Whether to lock publisher
    pub publisher_lock: Option<bool>,

    /// Whether to lock imprint
    pub imprint_lock: Option<bool>,

    /// Whether to lock genre
    pub genre_lock: Option<bool>,

    /// Whether to lock web URL
    pub web_lock: Option<bool>,

    /// Whether to lock language_iso
    pub language_iso_lock: Option<bool>,

    /// Whether to lock format_detail
    pub format_detail_lock: Option<bool>,

    /// Whether to lock black_and_white
    pub black_and_white_lock: Option<bool>,

    /// Whether to lock manga
    pub manga_lock: Option<bool>,

    /// Whether to lock year
    pub year_lock: Option<bool>,

    /// Whether to lock month
    pub month_lock: Option<bool>,

    /// Whether to lock day
    pub day_lock: Option<bool>,

    /// Whether to lock volume
    pub volume_lock: Option<bool>,

    /// Whether to lock count
    pub count_lock: Option<bool>,

    /// Whether to lock isbns
    pub isbns_lock: Option<bool>,
}

/// Response containing adjacent books in the same series
///
/// Returns the previous and next books relative to the requested book,
/// ordered by book number within the series.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdjacentBooksResponse {
    /// The previous book in the series (lower number), if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev: Option<BookDto>,

    /// The next book in the series (higher number), if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<BookDto>,
}

/// PATCH request for updating book core fields (title, number)
///
/// Only provided fields will be updated. Absent fields are unchanged.
/// Explicitly null fields will be cleared.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatchBookRequest {
    /// Book title (display name)
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "Chapter 1: The Beginning", nullable = true)]
    pub title: super::patch::PatchValue<String>,

    /// Book number (for sorting within series). Supports decimals like 1.5 for special chapters.
    #[serde(default)]
    #[schema(value_type = Option<f64>, example = 1.5, nullable = true)]
    pub number: super::patch::PatchValue<f64>,
}

/// Response for book update
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookUpdateResponse {
    /// Book ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub id: uuid::Uuid,

    /// Updated title
    #[schema(example = "Chapter 1: The Beginning")]
    pub title: Option<String>,

    /// Updated number
    #[schema(example = 1.5)]
    pub number: Option<f64>,

    /// Last update timestamp
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// ============================================================================
// Book Error DTOs
// ============================================================================

/// Book error type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BookErrorTypeDto {
    /// Error detecting the file format
    FormatDetection,
    /// Error parsing the book file (archive extraction, etc.)
    Parser,
    /// Error extracting or parsing metadata
    Metadata,
    /// Error generating thumbnail
    Thumbnail,
    /// Error extracting pages from the book
    PageExtraction,
    /// Error rendering PDF pages (e.g., PDFium not available)
    PdfRendering,
    /// Other uncategorized errors
    Other,
}

impl From<crate::db::entities::book_error::BookErrorType> for BookErrorTypeDto {
    fn from(t: crate::db::entities::book_error::BookErrorType) -> Self {
        use crate::db::entities::book_error::BookErrorType;
        match t {
            BookErrorType::FormatDetection => BookErrorTypeDto::FormatDetection,
            BookErrorType::Parser => BookErrorTypeDto::Parser,
            BookErrorType::Metadata => BookErrorTypeDto::Metadata,
            BookErrorType::Thumbnail => BookErrorTypeDto::Thumbnail,
            BookErrorType::PageExtraction => BookErrorTypeDto::PageExtraction,
            BookErrorType::PdfRendering => BookErrorTypeDto::PdfRendering,
            BookErrorType::Other => BookErrorTypeDto::Other,
        }
    }
}

impl From<BookErrorTypeDto> for crate::db::entities::book_error::BookErrorType {
    fn from(t: BookErrorTypeDto) -> Self {
        use crate::db::entities::book_error::BookErrorType;
        match t {
            BookErrorTypeDto::FormatDetection => BookErrorType::FormatDetection,
            BookErrorTypeDto::Parser => BookErrorType::Parser,
            BookErrorTypeDto::Metadata => BookErrorType::Metadata,
            BookErrorTypeDto::Thumbnail => BookErrorType::Thumbnail,
            BookErrorTypeDto::PageExtraction => BookErrorType::PageExtraction,
            BookErrorTypeDto::PdfRendering => BookErrorType::PdfRendering,
            BookErrorTypeDto::Other => BookErrorType::Other,
        }
    }
}

/// A single error for a book
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookErrorDto {
    /// Type of the error
    #[schema(example = "parser")]
    pub error_type: BookErrorTypeDto,

    /// Human-readable error message
    #[schema(example = "Failed to parse CBZ: invalid archive")]
    pub message: String,

    /// Additional error details (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,

    /// When the error occurred
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub occurred_at: DateTime<Utc>,
}

/// A book with its associated errors
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookWithErrorsDto {
    /// The book data
    pub book: BookDto,

    /// All errors for this book
    pub errors: Vec<BookErrorDto>,
}

/// Summary of errors grouped by type
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorGroupDto {
    /// Error type
    #[schema(example = "parser")]
    pub error_type: BookErrorTypeDto,

    /// Human-readable label for this error type
    #[schema(example = "Parser Error")]
    pub label: String,

    /// Number of books with this error type
    #[schema(example = 5)]
    pub count: u64,

    /// Books with this error type (paginated)
    pub books: Vec<BookWithErrorsDto>,
}

/// Response for listing books with errors
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BooksWithErrorsResponse {
    /// Total number of books with errors
    #[schema(example = 15)]
    pub total_books_with_errors: u64,

    /// Count of books by error type
    #[schema(example = json!({"parser": 5, "thumbnail": 10}))]
    pub error_counts: std::collections::HashMap<String, u64>,

    /// Error groups with books
    pub groups: Vec<ErrorGroupDto>,

    /// Current page (0-indexed)
    #[schema(example = 0)]
    pub page: u64,

    /// Page size
    #[schema(example = 20)]
    pub page_size: u64,

    /// Total number of pages
    #[schema(example = 1)]
    pub total_pages: u64,
}

/// Request body for retrying book errors
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RetryBookErrorsRequest {
    /// Specific error types to retry. If not provided, retry based on all current error types.
    #[serde(default)]
    #[schema(example = json!(["parser", "thumbnail"]))]
    pub error_types: Option<Vec<BookErrorTypeDto>>,
}

/// Request body for bulk retrying all book errors
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RetryAllErrorsRequest {
    /// Filter to only retry specific error type. If not provided, retry all error types.
    #[serde(default)]
    #[schema(example = "parser")]
    pub error_type: Option<BookErrorTypeDto>,

    /// Filter to only retry errors in a specific library
    #[serde(default)]
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub library_id: Option<uuid::Uuid>,
}

/// Response for retry operations
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RetryErrorsResponse {
    /// Number of tasks enqueued
    #[schema(example = 5)]
    pub tasks_enqueued: u64,

    /// Message describing what was done
    #[schema(example = "Enqueued 5 analysis tasks")]
    pub message: String,
}

// ============================================================================
// Full Book Response (with metadata and locks)
// ============================================================================

/// Book full list response (with metadata and locks)
pub type FullBookListResponse = PaginatedResponse<FullBookResponse>;

/// Full book metadata including all fields and their lock states
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookFullMetadata {
    /// Book title from metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Batman: Year One #1")]
    pub title: Option<String>,

    /// Sort title for ordering
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "batman year one 001")]
    pub title_sort: Option<String>,

    /// Chapter/book number
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "1")]
    pub number: Option<String>,

    /// Book summary/description
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Bruce Wayne returns to Gotham City after years abroad.")]
    pub summary: Option<String>,

    /// Writer(s)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Frank Miller")]
    pub writer: Option<String>,

    /// Penciller(s)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "David Mazzucchelli")]
    pub penciller: Option<String>,

    /// Inker(s)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "David Mazzucchelli")]
    pub inker: Option<String>,

    /// Colorist(s)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Richmond Lewis")]
    pub colorist: Option<String>,

    /// Letterer(s)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Todd Klein")]
    pub letterer: Option<String>,

    /// Cover artist(s)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "David Mazzucchelli")]
    pub cover_artist: Option<String>,

    /// Editor(s)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Dennis O'Neil")]
    pub editor: Option<String>,

    /// Publisher name
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "DC Comics")]
    pub publisher: Option<String>,

    /// Imprint name
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "DC Black Label")]
    pub imprint: Option<String>,

    /// Genre
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Superhero")]
    pub genre: Option<String>,

    /// Web URL
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "https://dc.com/batman-year-one")]
    pub web: Option<String>,

    /// ISO language code
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "en")]
    pub language_iso: Option<String>,

    /// Format details
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Trade Paperback")]
    pub format_detail: Option<String>,

    /// Whether the book is black and white
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub black_and_white: Option<bool>,

    /// Whether the book is manga format
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub manga: Option<bool>,

    /// Publication year
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 1987)]
    pub year: Option<i32>,

    /// Publication month (1-12)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 2)]
    pub month: Option<i32>,

    /// Publication day (1-31)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 1)]
    pub day: Option<i32>,

    /// Volume number
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 1)]
    pub volume: Option<i32>,

    /// Total count in series
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 4)]
    pub count: Option<i32>,

    /// ISBN(s)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "978-1401207526")]
    pub isbns: Option<String>,

    /// Lock states for all metadata fields
    pub locks: BookMetadataLocks,

    /// When the metadata was created
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the metadata was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Full book response including book data and complete metadata with locks
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FullBookResponse {
    /// Book unique identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub id: uuid::Uuid,

    /// Library this book belongs to
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub library_id: uuid::Uuid,

    /// Name of the library
    #[schema(example = "Comics")]
    pub library_name: String,

    /// Series this book belongs to
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub series_id: uuid::Uuid,

    /// Name of the series
    #[schema(example = "Batman: Year One")]
    pub series_name: String,

    /// Book title (display name)
    #[schema(example = "Batman: Year One #1")]
    pub title: String,

    /// Title used for sorting
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "batman year one 001")]
    pub title_sort: Option<String>,

    /// Filesystem path to the book file
    #[schema(example = "/media/comics/Batman/Batman - Year One 001.cbz")]
    pub file_path: String,

    /// File format (cbz, cbr, epub, pdf)
    #[schema(example = "cbz")]
    pub file_format: String,

    /// File size in bytes
    #[schema(example = 52428800)]
    pub file_size: i64,

    /// File hash for deduplication
    #[schema(example = "a1b2c3d4e5f6g7h8i9j0")]
    pub file_hash: String,

    /// Number of pages in the book
    #[schema(example = 32)]
    pub page_count: i32,

    /// Book number within the series
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 1)]
    pub number: Option<i32>,

    /// Whether the book has been soft-deleted
    #[schema(example = false)]
    pub deleted: bool,

    /// Error message if book analysis failed
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Failed to parse CBZ: invalid archive")]
    pub analysis_error: Option<String>,

    /// Effective reading direction (from series metadata, or library default)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "ltr")]
    pub reading_direction: Option<String>,

    /// User's read progress for this book
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_progress: Option<ReadProgressResponse>,

    /// Complete book metadata with lock states
    pub metadata: BookFullMetadata,

    /// When the book was added to the library
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the book was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}
