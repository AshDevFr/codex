use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::common::PaginatedResponse;
use super::read_progress::ReadProgressResponse;
use super::series::SortDirection;

// Re-export BookType from entity for API use
pub use crate::db::entities::book_metadata::BookType;

// =============================================================================
// Book Type DTO (API representation)
// =============================================================================

/// Book type enum for API responses
///
/// This mirrors the database BookType enum for use in DTOs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BookTypeDto {
    /// Western comic book
    Comic,
    /// Japanese manga
    Manga,
    /// Full-length novel
    Novel,
    /// Short novel (typically 17,500-40,000 words)
    Novella,
    /// Collection of short stories or works by multiple authors
    Anthology,
    /// Art collection book
    Artbook,
    /// Standalone story (single issue)
    Oneshot,
    /// Collection of multiple volumes/issues in one book
    Omnibus,
    /// Long-form comic narrative (typically standalone)
    GraphicNovel,
    /// Periodical publication
    Magazine,
}

impl From<BookType> for BookTypeDto {
    fn from(bt: BookType) -> Self {
        match bt {
            BookType::Comic => BookTypeDto::Comic,
            BookType::Manga => BookTypeDto::Manga,
            BookType::Novel => BookTypeDto::Novel,
            BookType::Novella => BookTypeDto::Novella,
            BookType::Anthology => BookTypeDto::Anthology,
            BookType::Artbook => BookTypeDto::Artbook,
            BookType::Oneshot => BookTypeDto::Oneshot,
            BookType::Omnibus => BookTypeDto::Omnibus,
            BookType::GraphicNovel => BookTypeDto::GraphicNovel,
            BookType::Magazine => BookTypeDto::Magazine,
        }
    }
}

impl fmt::Display for BookTypeDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bt: BookType = (*self).into();
        write!(f, "{}", bt)
    }
}

impl From<BookTypeDto> for BookType {
    fn from(dto: BookTypeDto) -> Self {
        match dto {
            BookTypeDto::Comic => BookType::Comic,
            BookTypeDto::Manga => BookType::Manga,
            BookTypeDto::Novel => BookType::Novel,
            BookTypeDto::Novella => BookType::Novella,
            BookTypeDto::Anthology => BookType::Anthology,
            BookTypeDto::Artbook => BookType::Artbook,
            BookTypeDto::Oneshot => BookType::Oneshot,
            BookTypeDto::Omnibus => BookType::Omnibus,
            BookTypeDto::GraphicNovel => BookType::GraphicNovel,
            BookTypeDto::Magazine => BookType::Magazine,
        }
    }
}

// =============================================================================
// Book Author DTO (for structured author data)
// =============================================================================

/// Role of an author in a book
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BookAuthorRole {
    /// Primary author
    Author,
    /// Co-author
    CoAuthor,
    /// Editor
    Editor,
    /// Translator
    Translator,
    /// Illustrator
    Illustrator,
    /// Contributor (other role)
    Contributor,
    /// Writer (comics)
    Writer,
    /// Penciller (comics)
    Penciller,
    /// Inker (comics)
    Inker,
    /// Colorist (comics)
    Colorist,
    /// Letterer (comics)
    Letterer,
    /// Cover artist (comics)
    CoverArtist,
}

/// Structured author information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookAuthorDto {
    /// Author's name
    #[schema(example = "Andy Weir")]
    pub name: String,

    /// Role of this author
    #[schema(example = "author")]
    pub role: BookAuthorRole,

    /// Sort name for ordering (e.g., "Weir, Andy")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Weir, Andy")]
    pub sort_name: Option<String>,
}

// =============================================================================
// Book Award DTO (for awards data)
// =============================================================================

/// Award information for a book
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookAwardDto {
    /// Name of the award
    #[schema(example = "Hugo Award")]
    pub name: String,

    /// Year the award was given/nominated
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 2015)]
    pub year: Option<i32>,

    /// Award category
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Best Novel")]
    pub category: Option<String>,

    /// Whether the book won (true) or was just nominated (false)
    #[schema(example = true)]
    pub won: bool,
}

// =============================================================================
// Book External ID DTO (for external source tracking)
// =============================================================================

/// External ID from a metadata provider (plugin, epub, pdf, manual)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookExternalIdDto {
    /// External ID record ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440070")]
    pub id: uuid::Uuid,

    /// Book ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub book_id: uuid::Uuid,

    /// Source identifier (e.g., "plugin:openlibrary", "epub", "pdf", "manual")
    #[schema(example = "plugin:openlibrary")]
    pub source: String,

    /// External ID value from the source (ISBN, OLID, etc.)
    #[schema(example = "OL123456W")]
    pub external_id: String,

    /// URL to the external source (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "https://openlibrary.org/works/OL123456W")]
    pub external_url: Option<String>,

    /// Hash of the last fetched metadata (for change detection)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_hash: Option<String>,

    /// When the metadata was last synced from this source
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub last_synced_at: Option<DateTime<Utc>>,

    /// When the external ID was created
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the external ID was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

impl From<crate::db::entities::book_external_ids::Model> for BookExternalIdDto {
    fn from(model: crate::db::entities::book_external_ids::Model) -> Self {
        Self {
            id: model.id,
            book_id: model.book_id,
            source: model.source,
            external_id: model.external_id,
            external_url: model.external_url,
            metadata_hash: model.metadata_hash,
            last_synced_at: model.last_synced_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

// =============================================================================
// Book Cover DTO (for cover management)
// =============================================================================

/// Book cover data transfer object
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookCoverDto {
    /// Cover ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440080")]
    pub id: uuid::Uuid,

    /// Book ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub book_id: uuid::Uuid,

    /// Cover source (e.g., "extracted", "plugin:openlibrary", "custom", "url")
    #[schema(example = "extracted")]
    pub source: String,

    /// Path to the cover image
    #[schema(example = "uploads/covers/books/550e8400-e29b-41d4-a716-446655440001/cover.jpg")]
    pub path: String,

    /// Whether this cover is currently selected as the primary cover
    #[schema(example = true)]
    pub is_selected: bool,

    /// Image width in pixels (if known)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 300)]
    pub width: Option<i32>,

    /// Image height in pixels (if known)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 450)]
    pub height: Option<i32>,

    /// When the cover was created
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the cover was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

impl From<crate::db::entities::book_covers::Model> for BookCoverDto {
    fn from(model: crate::db::entities::book_covers::Model) -> Self {
        Self {
            id: model.id,
            book_id: model.book_id,
            source: model.source,
            path: model.path,
            is_selected: model.is_selected,
            width: model.width,
            height: model.height,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

// =============================================================================
// Book External ID List Response & Create Request
// =============================================================================

/// Response containing a list of book external IDs
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookExternalIdListResponse {
    /// List of external IDs for the book
    pub external_ids: Vec<BookExternalIdDto>,
}

/// Request to create or update a book external ID
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateBookExternalIdRequest {
    /// Source identifier (e.g., "plugin:openlibrary", "epub", "pdf", "manual")
    #[schema(example = "manual")]
    pub source: String,

    /// External ID value (ISBN, OLID, etc.)
    #[schema(example = "978-0553418026")]
    pub external_id: String,

    /// URL to the external source (optional)
    #[serde(default)]
    #[schema(example = "https://openlibrary.org/works/OL123456W")]
    pub external_url: Option<String>,
}

// =============================================================================
// Book External Link DTOs (mirrors series ExternalLinkDto)
// =============================================================================

/// External link to an external site for a book
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookExternalLinkDto {
    /// External link ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440060")]
    pub id: uuid::Uuid,

    /// Book ID this link belongs to
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub book_id: uuid::Uuid,

    /// Source name (e.g., "openlibrary", "goodreads", "amazon")
    #[schema(example = "openlibrary")]
    pub source_name: String,

    /// URL to the external site
    #[schema(example = "https://openlibrary.org/works/OL123W")]
    pub url: String,

    /// ID on the external source (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "OL123W")]
    pub external_id: Option<String>,

    /// When the link was created
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the link was last updated
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub updated_at: DateTime<Utc>,
}

impl From<crate::db::entities::book_external_links::Model> for BookExternalLinkDto {
    fn from(model: crate::db::entities::book_external_links::Model) -> Self {
        Self {
            id: model.id,
            book_id: model.book_id,
            source_name: model.source_name,
            url: model.url,
            external_id: model.external_id,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

/// Response containing a list of book external links
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookExternalLinkListResponse {
    /// List of external links for the book
    pub links: Vec<BookExternalLinkDto>,
}

/// Request to create or update a book external link
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateBookExternalLinkRequest {
    /// Source name (e.g., "openlibrary", "goodreads", "amazon")
    /// Will be normalized to lowercase
    #[schema(example = "openlibrary")]
    pub source_name: String,

    /// URL to the external site
    #[schema(example = "https://openlibrary.org/works/OL123W")]
    pub url: String,

    /// ID on the external source (if available)
    #[schema(example = "OL123W")]
    pub external_id: Option<String>,
}

// =============================================================================
// Book Cover List Response
// =============================================================================

/// Response containing a list of book covers
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookCoverListResponse {
    /// List of covers for the book
    pub covers: Vec<BookCoverDto>,
}

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
    /// Sort by last read date (requires user_id for filtering)
    LastRead,
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
            BookSortField::LastRead => write!(f, "last_read"),
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
            "last_read" | "read_date" => Ok(BookSortField::LastRead),
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

    /// Whether the book has been analyzed (page dimensions available)
    #[schema(example = true)]
    pub analyzed: bool,

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

    // ==========================================================================
    // New book metadata fields (Phase 6)
    // ==========================================================================
    /// Book type classification (comic, manga, novel, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "novel")]
    pub book_type: Option<BookTypeDto>,

    /// Book subtitle
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "A Novel")]
    pub subtitle: Option<String>,

    /// Structured author information as JSON array
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!([{"name": "Andy Weir", "role": "author", "sortName": "Weir, Andy"}]))]
    pub authors: Option<Vec<BookAuthorDto>>,

    /// Translator name
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "John Smith")]
    pub translator: Option<String>,

    /// Edition information (e.g., "First Edition", "Revised Edition")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "First Edition")]
    pub edition: Option<String>,

    /// Original title (for translated works)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "火星の人")]
    pub original_title: Option<String>,

    /// Original publication year (for re-releases or translations)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 2011)]
    pub original_year: Option<i32>,

    /// Position in a series (e.g., 1.0, 2.5 for .5 volumes)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 1.0)]
    pub series_position: Option<f64>,

    /// Total number of books in the series
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 3)]
    pub series_total: Option<i32>,

    /// Subject/topic tags
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["Science Fiction", "Space Exploration", "Survival"]))]
    pub subjects: Option<Vec<String>>,

    /// Awards information
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!([{"name": "Hugo Award", "year": 2015, "category": "Best Novel", "won": true}]))]
    pub awards: Option<Vec<BookAwardDto>>,

    /// Custom metadata JSON escape hatch
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>, example = json!({"customField": "value"}))]
    pub custom_metadata: Option<serde_json::Value>,

    // ==========================================================================
    // ComicInfo / raw metadata fields (needed by edit form)
    // ==========================================================================
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

    // ==========================================================================
    // New book metadata fields (Phase 6)
    // ==========================================================================
    /// Book type classification (comic, manga, novel, etc.)
    #[schema(example = "novel")]
    pub book_type: Option<BookTypeDto>,

    /// Book subtitle
    #[schema(example = "A Novel")]
    pub subtitle: Option<String>,

    /// Structured author information as JSON array
    #[schema(example = json!([{"name": "Andy Weir", "role": "author", "sortName": "Weir, Andy"}]))]
    pub authors: Option<Vec<BookAuthorDto>>,

    /// Translator name
    #[schema(example = "John Smith")]
    pub translator: Option<String>,

    /// Edition information (e.g., "First Edition", "Revised Edition")
    #[schema(example = "First Edition")]
    pub edition: Option<String>,

    /// Original title (for translated works)
    #[schema(example = "火星の人")]
    pub original_title: Option<String>,

    /// Original publication year (for re-releases or translations)
    #[schema(example = 2011)]
    pub original_year: Option<i32>,

    /// Position in a series (e.g., 1.0, 2.5 for .5 volumes)
    #[schema(example = 1.0)]
    pub series_position: Option<f64>,

    /// Total number of books in the series
    #[schema(example = 3)]
    pub series_total: Option<i32>,

    /// Subject/topic tags
    #[schema(example = json!(["Science Fiction", "Space Exploration", "Survival"]))]
    pub subjects: Option<Vec<String>>,

    /// Awards information
    #[schema(example = json!([{"name": "Hugo Award", "year": 2015, "category": "Best Novel", "won": true}]))]
    pub awards: Option<Vec<BookAwardDto>>,

    /// Custom metadata JSON escape hatch
    #[schema(value_type = Option<Object>, example = json!({"customField": "value"}))]
    pub custom_metadata: Option<serde_json::Value>,
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

    // ==========================================================================
    // New book metadata fields (Phase 6)
    // ==========================================================================
    /// Book type classification (comic, manga, novel, etc.)
    #[serde(default)]
    #[schema(value_type = Option<BookTypeDto>, example = "novel", nullable = true)]
    pub book_type: super::patch::PatchValue<BookTypeDto>,

    /// Book subtitle
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "A Novel", nullable = true)]
    pub subtitle: super::patch::PatchValue<String>,

    /// Structured author information as JSON array
    #[serde(default)]
    #[schema(value_type = Option<Vec<BookAuthorDto>>, nullable = true)]
    pub authors: super::patch::PatchValue<Vec<BookAuthorDto>>,

    /// Translator name
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "John Smith", nullable = true)]
    pub translator: super::patch::PatchValue<String>,

    /// Edition information (e.g., "First Edition", "Revised Edition")
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "First Edition", nullable = true)]
    pub edition: super::patch::PatchValue<String>,

    /// Original title (for translated works)
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "火星の人", nullable = true)]
    pub original_title: super::patch::PatchValue<String>,

    /// Original publication year (for re-releases or translations)
    #[serde(default)]
    #[schema(value_type = Option<i32>, example = 2011, nullable = true)]
    pub original_year: super::patch::PatchValue<i32>,

    /// Position in a series (e.g., 1.0, 2.5 for .5 volumes)
    #[serde(default)]
    #[schema(value_type = Option<f64>, example = 1.0, nullable = true)]
    pub series_position: super::patch::PatchValue<f64>,

    /// Total number of books in the series
    #[serde(default)]
    #[schema(value_type = Option<i32>, example = 3, nullable = true)]
    pub series_total: super::patch::PatchValue<i32>,

    /// Subject/topic tags
    #[serde(default)]
    #[schema(value_type = Option<Vec<String>>, nullable = true)]
    pub subjects: super::patch::PatchValue<Vec<String>>,

    /// Awards information
    #[serde(default)]
    #[schema(value_type = Option<Vec<BookAwardDto>>, nullable = true)]
    pub awards: super::patch::PatchValue<Vec<BookAwardDto>>,

    /// Custom metadata JSON escape hatch
    #[serde(default)]
    #[schema(value_type = Option<Object>, nullable = true)]
    pub custom_metadata: super::patch::PatchValue<serde_json::Value>,
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

    // ==========================================================================
    // New book metadata fields (Phase 6)
    // ==========================================================================
    /// Book type classification (comic, manga, novel, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "novel")]
    pub book_type: Option<BookTypeDto>,

    /// Book subtitle
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "A Novel")]
    pub subtitle: Option<String>,

    /// Structured author information as JSON array
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<BookAuthorDto>>,

    /// Translator name
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "John Smith")]
    pub translator: Option<String>,

    /// Edition information (e.g., "First Edition", "Revised Edition")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "First Edition")]
    pub edition: Option<String>,

    /// Original title (for translated works)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "火星の人")]
    pub original_title: Option<String>,

    /// Original publication year (for re-releases or translations)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 2011)]
    pub original_year: Option<i32>,

    /// Position in a series (e.g., 1.0, 2.5 for .5 volumes)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 1.0)]
    pub series_position: Option<f64>,

    /// Total number of books in the series
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 3)]
    pub series_total: Option<i32>,

    /// Subject/topic tags
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjects: Option<Vec<String>>,

    /// Awards information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub awards: Option<Vec<BookAwardDto>>,

    /// Custom metadata JSON escape hatch
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub custom_metadata: Option<serde_json::Value>,

    /// Metadata lock states
    pub locks: BookMetadataLocks,

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
    /// Whether title is locked
    #[schema(example = false)]
    pub title_lock: bool,

    /// Whether title_sort is locked
    #[schema(example = false)]
    pub title_sort_lock: bool,

    /// Whether number is locked
    #[schema(example = false)]
    pub number_lock: bool,

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

    // ==========================================================================
    // New lock fields (Phase 6)
    // ==========================================================================
    /// Whether book_type is locked
    #[schema(example = false)]
    pub book_type_lock: bool,

    /// Whether subtitle is locked
    #[schema(example = false)]
    pub subtitle_lock: bool,

    /// Whether authors_json is locked
    #[schema(example = false)]
    pub authors_json_lock: bool,

    /// Whether translator is locked
    #[schema(example = false)]
    pub translator_lock: bool,

    /// Whether edition is locked
    #[schema(example = false)]
    pub edition_lock: bool,

    /// Whether original_title is locked
    #[schema(example = false)]
    pub original_title_lock: bool,

    /// Whether original_year is locked
    #[schema(example = false)]
    pub original_year_lock: bool,

    /// Whether series_position is locked
    #[schema(example = false)]
    pub series_position_lock: bool,

    /// Whether series_total is locked
    #[schema(example = false)]
    pub series_total_lock: bool,

    /// Whether subjects is locked
    #[schema(example = false)]
    pub subjects_lock: bool,

    /// Whether awards_json is locked
    #[schema(example = false)]
    pub awards_json_lock: bool,

    /// Whether custom_metadata is locked
    #[schema(example = false)]
    pub custom_metadata_lock: bool,

    /// Whether cover is locked (prevents auto-updates)
    #[schema(example = false)]
    pub cover_lock: bool,
}

/// Request to update book metadata locks
///
/// All fields are optional. Only provided fields will be updated.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBookMetadataLocksRequest {
    /// Whether to lock title
    pub title_lock: Option<bool>,

    /// Whether to lock title_sort
    pub title_sort_lock: Option<bool>,

    /// Whether to lock number
    pub number_lock: Option<bool>,

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

    // ==========================================================================
    // New lock fields (Phase 6)
    // ==========================================================================
    /// Whether to lock book_type
    pub book_type_lock: Option<bool>,

    /// Whether to lock subtitle
    pub subtitle_lock: Option<bool>,

    /// Whether to lock authors_json
    pub authors_json_lock: Option<bool>,

    /// Whether to lock translator
    pub translator_lock: Option<bool>,

    /// Whether to lock edition
    pub edition_lock: Option<bool>,

    /// Whether to lock original_title
    pub original_title_lock: Option<bool>,

    /// Whether to lock original_year
    pub original_year_lock: Option<bool>,

    /// Whether to lock series_position
    pub series_position_lock: Option<bool>,

    /// Whether to lock series_total
    pub series_total_lock: Option<bool>,

    /// Whether to lock subjects
    pub subjects_lock: Option<bool>,

    /// Whether to lock awards_json
    pub awards_json_lock: Option<bool>,

    /// Whether to lock custom_metadata
    pub custom_metadata_lock: Option<bool>,

    /// Whether to lock cover (prevents auto-updates)
    pub cover_lock: Option<bool>,
}

// ==========================================================================
// Book genre/tag request DTOs
// ==========================================================================

/// Request to set all genres for a book (replaces existing)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetBookGenresRequest {
    /// List of genre names to assign to the book
    #[schema(example = json!(["Action", "Comedy", "Drama"]))]
    pub genres: Vec<String>,
}

/// Request to add a single genre to a book
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddBookGenreRequest {
    /// Genre name to add
    #[schema(example = "Action")]
    pub name: String,
}

/// Request to set all tags for a book (replaces existing)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetBookTagsRequest {
    /// List of tag names to assign to the book
    #[schema(example = json!(["completed", "favorite", "to-read"]))]
    pub tags: Vec<String>,
}

/// Request to add a single tag to a book
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddBookTagRequest {
    /// Tag name to add
    #[schema(example = "favorite")]
    pub name: String,
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
    /// Book was analyzed successfully but contains zero pages
    ZeroPages,
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
            BookErrorType::ZeroPages => BookErrorTypeDto::ZeroPages,
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
            BookErrorTypeDto::ZeroPages => BookErrorType::ZeroPages,
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

    // ==========================================================================
    // Phase 6 fields (book-specific rich metadata)
    // ==========================================================================
    /// Book type classification (comic, manga, novel, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "novel")]
    pub book_type: Option<BookTypeDto>,

    /// Book subtitle
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "A Novel")]
    pub subtitle: Option<String>,

    /// Structured author information as JSON array
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!([{"name": "Andy Weir", "role": "author", "sortName": "Weir, Andy"}]))]
    pub authors: Option<Vec<BookAuthorDto>>,

    /// Translator name
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "John Smith")]
    pub translator: Option<String>,

    /// Edition information (e.g., "First Edition", "Revised Edition")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "First Edition")]
    pub edition: Option<String>,

    /// Original title (for translated works)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "火星の人")]
    pub original_title: Option<String>,

    /// Original publication year (for re-releases or translations)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 2011)]
    pub original_year: Option<i32>,

    /// Position in a series (e.g., 1.0, 2.5 for .5 volumes)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 1.0)]
    pub series_position: Option<f64>,

    /// Total number of books in the series
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 3)]
    pub series_total: Option<i32>,

    /// Subject/topic tags
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["Science Fiction", "Space Exploration", "Survival"]))]
    pub subjects: Option<Vec<String>>,

    /// Awards information
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!([{"name": "Hugo Award", "year": 2015, "category": "Best Novel", "won": true}]))]
    pub awards: Option<Vec<BookAwardDto>>,

    /// Custom metadata JSON escape hatch
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>, example = json!({"customField": "value"}))]
    pub custom_metadata: Option<serde_json::Value>,

    /// Release/publication date
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "1987-02-01T00:00:00Z")]
    pub release_date: Option<DateTime<Utc>>,

    /// Writers as array
    pub writers: Vec<String>,

    /// Pencillers as array
    pub pencillers: Vec<String>,

    /// Inkers as array
    pub inkers: Vec<String>,

    /// Colorists as array
    pub colorists: Vec<String>,

    /// Letterers as array
    pub letterers: Vec<String>,

    /// Cover artists as array
    pub cover_artists: Vec<String>,

    /// Editors as array
    pub editors: Vec<String>,

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

    /// Whether the book has been analyzed (page dimensions available)
    #[schema(example = true)]
    pub analyzed: bool,

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

    /// Genres assigned to this book
    pub genres: Vec<super::series::GenreDto>,

    /// Tags assigned to this book
    pub tags: Vec<super::series::TagDto>,

    /// When the book was added to the library
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the book was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}
