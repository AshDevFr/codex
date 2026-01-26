use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::common::PaginatedResponse;

/// Sort direction for list queries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

impl fmt::Display for SortDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SortDirection::Asc => write!(f, "asc"),
            SortDirection::Desc => write!(f, "desc"),
        }
    }
}

impl FromStr for SortDirection {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "asc" => Ok(SortDirection::Asc),
            "desc" => Ok(SortDirection::Desc),
            _ => Err(format!("Invalid sort direction: {}", s)),
        }
    }
}

/// Sort field options for series list queries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SeriesSortField {
    /// Sort by series name (uses title_sort if available, otherwise title)
    #[default]
    Name,
    /// Sort by date added to library
    DateAdded,
    /// Sort by last update time
    DateUpdated,
    /// Sort by release year
    ReleaseDate,
    /// Sort by last read time (user-specific)
    DateRead,
    /// Sort by number of books in the series
    BookCount,
}

impl fmt::Display for SeriesSortField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SeriesSortField::Name => write!(f, "name"),
            SeriesSortField::DateAdded => write!(f, "date_added"),
            SeriesSortField::DateUpdated => write!(f, "date_updated"),
            SeriesSortField::ReleaseDate => write!(f, "release_date"),
            SeriesSortField::DateRead => write!(f, "date_read"),
            SeriesSortField::BookCount => write!(f, "book_count"),
        }
    }
}

impl FromStr for SeriesSortField {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "name" => Ok(SeriesSortField::Name),
            "date_added" | "created_at" => Ok(SeriesSortField::DateAdded),
            "date_updated" | "updated_at" => Ok(SeriesSortField::DateUpdated),
            "release_date" | "year" => Ok(SeriesSortField::ReleaseDate),
            "date_read" => Ok(SeriesSortField::DateRead),
            "book_count" => Ok(SeriesSortField::BookCount),
            _ => Err(format!("Invalid sort field: {}", s)),
        }
    }
}

/// Parsed sort parameter for series queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeriesSortParam {
    pub field: SeriesSortField,
    pub direction: SortDirection,
}

impl Default for SeriesSortParam {
    fn default() -> Self {
        Self {
            field: SeriesSortField::Name,
            direction: SortDirection::Asc,
        }
    }
}

#[allow(dead_code)] // Public API for series sorting - used in query parsing
impl SeriesSortParam {
    pub fn new(field: SeriesSortField, direction: SortDirection) -> Self {
        Self { field, direction }
    }

    /// Parse from "field,direction" format (e.g., "name,asc")
    pub fn parse(s: &str) -> Self {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 2 {
            return Self::default();
        }

        let field = SeriesSortField::from_str(parts[0]).unwrap_or_default();
        let direction = SortDirection::from_str(parts[1]).unwrap_or_default();

        Self { field, direction }
    }

    /// Check if this sort requires user-specific data (e.g., read progress)
    pub fn requires_user_context(&self) -> bool {
        matches!(self.field, SeriesSortField::DateRead)
    }

    /// Check if this sort requires aggregation from books table
    pub fn requires_aggregation(&self) -> bool {
        false
    }
}

impl fmt::Display for SeriesSortParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{}", self.field, self.direction)
    }
}

/// Series data transfer object
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesDto {
    /// Series unique identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub id: uuid::Uuid,

    /// Library unique identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub library_id: uuid::Uuid,

    /// Name of the library this series belongs to
    #[schema(example = "Comics")]
    pub library_name: String,

    /// Series title from series_metadata
    #[schema(example = "Batman: Year One")]
    pub title: String,

    /// Sort title from series_metadata (for ordering)
    #[schema(example = "batman year one")]
    pub title_sort: Option<String>,

    /// Summary/description from series_metadata
    #[schema(
        example = "The definitive origin story of Batman, following Bruce Wayne's first year as a vigilante."
    )]
    pub summary: Option<String>,

    /// Publisher name
    #[schema(example = "DC Comics")]
    pub publisher: Option<String>,

    /// Release year
    #[schema(example = 1987)]
    pub year: Option<i32>,

    /// Total number of books in this series
    #[schema(example = 4)]
    pub book_count: i64,

    /// Filesystem path to the series directory
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "/media/comics/Batman - Year One")]
    pub path: Option<String>,

    /// Selected cover source (e.g., "first_book", "custom")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "first_book")]
    pub selected_cover_source: Option<String>,

    /// Whether the series has a custom cover uploaded
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub has_custom_cover: Option<bool>,

    /// Number of unread books in this series (user-specific)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 2)]
    pub unread_count: Option<i64>,

    /// When the series was created
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the series was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Series list response
pub type SeriesListResponse = PaginatedResponse<SeriesDto>;

/// Full series list response (with metadata, locks, genres, tags, etc.)
pub type FullSeriesListResponse = PaginatedResponse<FullSeriesResponse>;

/// Alphabetical group with count
///
/// Represents a group of series starting with a specific letter/character
/// along with the count of series in that group.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AlphabeticalGroupDto {
    /// The first character (lowercase letter, digit, or special character)
    #[schema(example = "a")]
    pub group: String,

    /// Number of series starting with this character
    #[schema(example = 20)]
    pub count: i64,
}

/// Search series request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchSeriesRequest {
    /// Search query
    #[schema(example = "batman")]
    pub query: String,

    /// Optional library filter
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub library_id: Option<uuid::Uuid>,

    /// Return full series data including metadata, locks, genres, tags, etc.
    #[serde(default)]
    pub full: bool,
}

/// PUT request for full replacement of series metadata
///
/// All metadata fields will be replaced with the values in this request.
/// Omitting a field (or setting it to null) will clear that field.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceSeriesMetadataRequest {
    /// Series title/name
    #[schema(example = "Batman: Year One")]
    pub title: Option<String>,

    /// Custom sort name for ordering (e.g., "Batman Year One" instead of "The Batman Year One")
    #[schema(example = "Batman Year One")]
    pub title_sort: Option<String>,

    /// Series description/summary
    #[schema(example = "The definitive origin story of Batman.")]
    pub summary: Option<String>,

    /// Publisher name
    #[schema(example = "DC Comics")]
    pub publisher: Option<String>,

    /// Imprint (sub-publisher)
    #[schema(example = "Vertigo")]
    pub imprint: Option<String>,

    /// Series status (ongoing, ended, hiatus, abandoned, unknown)
    #[schema(example = "ended")]
    pub status: Option<String>,

    /// Age rating (e.g., 13, 16, 18)
    #[schema(example = 16)]
    pub age_rating: Option<i32>,

    /// Language (BCP47 format: "en", "ja", "ko")
    #[schema(example = "en")]
    pub language: Option<String>,

    /// Reading direction (ltr, rtl, ttb or webtoon)
    #[schema(example = "ltr")]
    pub reading_direction: Option<String>,

    /// Release year
    #[schema(example = 1987)]
    pub year: Option<i32>,

    /// Expected total book count (for ongoing series)
    #[schema(example = 4)]
    pub total_book_count: Option<i32>,

    /// Custom JSON metadata for extensions
    #[schema(value_type = Option<Object>, example = json!({"myField": "value", "nested": {"key": "data"}}))]
    pub custom_metadata: Option<serde_json::Value>,
}

/// PATCH request for partial update of series metadata
///
/// Only provided fields will be updated. Absent fields are unchanged.
/// Explicitly null fields will be cleared.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatchSeriesMetadataRequest {
    /// Series title/name
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "Batman: Year One", nullable = true)]
    pub title: super::patch::PatchValue<String>,

    /// Custom sort name for ordering
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "Batman Year One", nullable = true)]
    pub title_sort: super::patch::PatchValue<String>,

    /// Series description/summary
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "The definitive origin story of Batman.", nullable = true)]
    pub summary: super::patch::PatchValue<String>,

    /// Publisher name
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "DC Comics", nullable = true)]
    pub publisher: super::patch::PatchValue<String>,

    /// Imprint (sub-publisher)
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "Vertigo", nullable = true)]
    pub imprint: super::patch::PatchValue<String>,

    /// Series status (ongoing, ended, hiatus, abandoned, unknown)
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "ended", nullable = true)]
    pub status: super::patch::PatchValue<String>,

    /// Age rating (e.g., 13, 16, 18)
    #[serde(default)]
    #[schema(value_type = Option<i32>, example = 16, nullable = true)]
    pub age_rating: super::patch::PatchValue<i32>,

    /// Language (BCP47 format: "en", "ja", "ko")
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "en", nullable = true)]
    pub language: super::patch::PatchValue<String>,

    /// Reading direction (ltr, rtl, ttb or webtoon)
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "ltr", nullable = true)]
    pub reading_direction: super::patch::PatchValue<String>,

    /// Release year
    #[serde(default)]
    #[schema(value_type = Option<i32>, example = 1987, nullable = true)]
    pub year: super::patch::PatchValue<i32>,

    /// Expected total book count (for ongoing series)
    #[serde(default)]
    #[schema(value_type = Option<i32>, example = 4, nullable = true)]
    pub total_book_count: super::patch::PatchValue<i32>,

    /// Custom JSON metadata for extensions
    #[serde(default)]
    #[schema(value_type = Option<Object>, example = json!({"myField": "value"}), nullable = true)]
    pub custom_metadata: super::patch::PatchValue<serde_json::Value>,
}

/// Response containing series metadata
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesMetadataResponse {
    /// Series ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub id: uuid::Uuid,

    /// Series title/name
    #[schema(example = "Batman: Year One")]
    pub title: String,

    /// Custom sort name for ordering
    #[schema(example = "Batman Year One")]
    pub title_sort: Option<String>,

    /// Series description/summary
    #[schema(example = "The definitive origin story of Batman.")]
    pub summary: Option<String>,

    /// Publisher name
    #[schema(example = "DC Comics")]
    pub publisher: Option<String>,

    /// Imprint (sub-publisher)
    #[schema(example = "Vertigo")]
    pub imprint: Option<String>,

    /// Series status (ongoing, ended, hiatus, abandoned, unknown)
    #[schema(example = "ended")]
    pub status: Option<String>,

    /// Age rating (e.g., 13, 16, 18)
    #[schema(example = 16)]
    pub age_rating: Option<i32>,

    /// Language (BCP47 format: "en", "ja", "ko")
    #[schema(example = "en")]
    pub language: Option<String>,

    /// Reading direction (ltr, rtl, ttb or webtoon)
    #[schema(example = "ltr")]
    pub reading_direction: Option<String>,

    /// Release year
    #[schema(example = 1987)]
    pub year: Option<i32>,

    /// Expected total book count (for ongoing series)
    #[schema(example = 4)]
    pub total_book_count: Option<i32>,

    /// Custom JSON metadata for extensions
    #[schema(value_type = Option<Object>, example = json!({"myField": "value"}))]
    pub custom_metadata: Option<serde_json::Value>,

    /// Last update timestamp
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// Genre DTOs
// ============================================================================

/// Genre data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GenreDto {
    /// Genre ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440010")]
    pub id: uuid::Uuid,

    /// Genre name
    #[schema(example = "Action")]
    pub name: String,

    /// Number of series with this genre
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 42)]
    pub series_count: Option<u64>,

    /// When the genre was created
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,
}

/// Response containing a list of genres
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GenreListResponse {
    /// List of genres
    pub genres: Vec<GenreDto>,
}

/// Request to set genres for a series
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetSeriesGenresRequest {
    /// List of genre names to set for the series
    /// Genres that don't exist will be created automatically
    #[schema(example = json!(["Action", "Comedy", "Drama"]))]
    pub genres: Vec<String>,
}

/// Request to add a single genre to a series
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddSeriesGenreRequest {
    /// Name of the genre to add
    /// The genre will be created if it doesn't exist
    #[schema(example = "Action")]
    pub name: String,
}

/// Response for taxonomy cleanup operations (genres/tags)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaxonomyCleanupResponse {
    /// Number of unused items deleted
    #[schema(example = 5)]
    pub deleted_count: u64,

    /// Names of deleted items
    #[schema(example = json!(["OldGenre", "UnusedGenre"]))]
    pub deleted_names: Vec<String>,
}

// ============================================================================
// Tag DTOs
// ============================================================================

/// Tag data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TagDto {
    /// Tag ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440020")]
    pub id: uuid::Uuid,

    /// Tag name
    #[schema(example = "Completed")]
    pub name: String,

    /// Number of series with this tag
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 15)]
    pub series_count: Option<u64>,

    /// When the tag was created
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,
}

/// Response containing a list of tags
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TagListResponse {
    /// List of tags
    pub tags: Vec<TagDto>,
}

/// Request to set tags for a series
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetSeriesTagsRequest {
    /// List of tag names to set for the series
    /// Tags that don't exist will be created automatically
    #[schema(example = json!(["Completed", "Favorite", "Reading"]))]
    pub tags: Vec<String>,
}

/// Request to add a single tag to a series
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddSeriesTagRequest {
    /// Name of the tag to add
    /// The tag will be created if it doesn't exist
    #[schema(example = "Favorite")]
    pub name: String,
}

// ============================================================================
// User Rating DTOs
// ============================================================================

/// User series rating data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserSeriesRatingDto {
    /// Rating ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440030")]
    pub id: uuid::Uuid,

    /// Series ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub series_id: uuid::Uuid,

    /// Rating value (1-100, displayed as 1-10 in UI with 0.1 precision)
    #[schema(example = 85)]
    pub rating: i32,

    /// Optional notes/review
    #[schema(example = "Great series, loved the art style!")]
    pub notes: Option<String>,

    /// When the rating was created
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the rating was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Response containing a list of user ratings
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserRatingsListResponse {
    /// List of user ratings
    pub ratings: Vec<UserSeriesRatingDto>,
}

/// Request to create or update a user's rating for a series
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetUserRatingRequest {
    /// Rating value (1-100, typically set via 1-10 slider multiplied by 10)
    ///
    /// In the UI, display as 1-10 with 0.5 step increments.
    /// Multiply UI value by 10 before sending (e.g., 7.5 → 75).
    #[schema(example = 85, minimum = 1, maximum = 100)]
    pub rating: i32,

    /// Optional notes/review for this series
    #[schema(example = "Great series, loved the art style!")]
    pub notes: Option<String>,
}

// ============================================================================
// Alternate Title DTOs
// ============================================================================

/// Alternate title data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AlternateTitleDto {
    /// Alternate title ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440040")]
    pub id: uuid::Uuid,

    /// Series ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub series_id: uuid::Uuid,

    /// Label for this title (e.g., "Japanese", "Romaji", "English", "Korean")
    #[schema(example = "Japanese")]
    pub label: String,

    /// The alternate title
    #[schema(example = "進撃の巨人")]
    pub title: String,

    /// When the title was created
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the title was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Response containing a list of alternate titles
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AlternateTitleListResponse {
    /// List of alternate titles
    pub titles: Vec<AlternateTitleDto>,
}

/// Request to create an alternate title for a series
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateAlternateTitleRequest {
    /// Label for this title (e.g., "Japanese", "Romaji", "English")
    #[schema(example = "Japanese")]
    pub label: String,

    /// The alternate title
    #[schema(example = "進撃の巨人")]
    pub title: String,
}

/// Request to update an alternate title
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAlternateTitleRequest {
    /// New label for this title
    #[schema(example = "Romaji")]
    pub label: Option<String>,

    /// New title text
    #[schema(example = "Shingeki no Kyojin")]
    pub title: Option<String>,
}

// ============================================================================
// External Rating DTOs
// ============================================================================

/// External rating data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExternalRatingDto {
    /// External rating ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440050")]
    pub id: uuid::Uuid,

    /// Series ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub series_id: uuid::Uuid,

    /// Source name (e.g., "myanimelist", "anilist", "mangabaka")
    #[schema(example = "myanimelist")]
    pub source_name: String,

    /// Rating value (0-100)
    #[schema(example = 85.5)]
    pub rating: f64,

    /// Number of votes (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 12500)]
    pub vote_count: Option<i32>,

    /// When the rating was last fetched from the source
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub fetched_at: DateTime<Utc>,

    /// When the rating record was created
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the rating record was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Response containing a list of external ratings
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExternalRatingListResponse {
    /// List of external ratings
    pub ratings: Vec<ExternalRatingDto>,
}

/// Request to create or update an external rating for a series
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateExternalRatingRequest {
    /// Source name (e.g., "myanimelist", "anilist", "mangabaka")
    /// Will be normalized to lowercase
    #[schema(example = "myanimelist")]
    pub source_name: String,

    /// Rating value (0-100)
    #[schema(example = 85.5)]
    pub rating: f64,

    /// Number of votes (if available)
    #[schema(example = 12500)]
    pub vote_count: Option<i32>,
}

// ============================================================================
// External Link DTOs
// ============================================================================

/// External link data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExternalLinkDto {
    /// External link ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440060")]
    pub id: uuid::Uuid,

    /// Series ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub series_id: uuid::Uuid,

    /// Source name (e.g., "myanimelist", "anilist", "mangadex")
    #[schema(example = "myanimelist")]
    pub source_name: String,

    /// URL to the external source
    #[schema(example = "https://myanimelist.net/manga/12345")]
    pub url: String,

    /// ID on the external source (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "12345")]
    pub external_id: Option<String>,

    /// When the link was created
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the link was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Response containing a list of external links
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExternalLinkListResponse {
    /// List of external links
    pub links: Vec<ExternalLinkDto>,
}

/// Request to create or update an external link for a series
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateExternalLinkRequest {
    /// Source name (e.g., "myanimelist", "anilist", "mangadex")
    /// Will be normalized to lowercase
    #[schema(example = "myanimelist")]
    pub source_name: String,

    /// URL to the external source
    #[schema(example = "https://myanimelist.net/manga/12345")]
    pub url: String,

    /// ID on the external source (if available)
    #[schema(example = "12345")]
    pub external_id: Option<String>,
}

// ============================================================================
// Full Metadata DTOs (with locks and related data)
// ============================================================================

/// Lock states for all lockable metadata fields
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetadataLocks {
    /// Whether the title field is locked
    #[schema(example = false)]
    pub title: bool,

    /// Whether the title_sort field is locked
    #[schema(example = false)]
    pub title_sort: bool,

    /// Whether the summary field is locked
    #[schema(example = true)]
    pub summary: bool,

    /// Whether the publisher field is locked
    #[schema(example = false)]
    pub publisher: bool,

    /// Whether the imprint field is locked
    #[schema(example = false)]
    pub imprint: bool,

    /// Whether the status field is locked
    #[schema(example = false)]
    pub status: bool,

    /// Whether the age_rating field is locked
    #[schema(example = false)]
    pub age_rating: bool,

    /// Whether the language field is locked
    #[schema(example = false)]
    pub language: bool,

    /// Whether the reading_direction field is locked
    #[schema(example = false)]
    pub reading_direction: bool,

    /// Whether the year field is locked
    #[schema(example = false)]
    pub year: bool,

    /// Whether the total_book_count field is locked
    #[schema(example = false)]
    pub total_book_count: bool,

    /// Whether the genres are locked
    #[schema(example = false)]
    pub genres: bool,

    /// Whether the tags are locked
    #[schema(example = false)]
    pub tags: bool,

    /// Whether the custom_metadata field is locked
    #[schema(example = false)]
    pub custom_metadata: bool,
}

/// Full series metadata response including all related data
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FullSeriesMetadataResponse {
    /// Series ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub series_id: uuid::Uuid,

    // Core metadata fields
    /// Series title (usually same as series name)
    #[schema(example = "Batman: Year One")]
    pub title: String,

    /// Custom sort name for ordering
    #[schema(example = "Batman Year One")]
    pub title_sort: Option<String>,

    /// Series description/summary
    #[schema(example = "The definitive origin story of Batman.")]
    pub summary: Option<String>,

    /// Publisher name
    #[schema(example = "DC Comics")]
    pub publisher: Option<String>,

    /// Imprint (sub-publisher)
    #[schema(example = "Vertigo")]
    pub imprint: Option<String>,

    /// Series status (ongoing, ended, hiatus, abandoned, unknown)
    #[schema(example = "ended")]
    pub status: Option<String>,

    /// Age rating (e.g., 13, 16, 18)
    #[schema(example = 16)]
    pub age_rating: Option<i32>,

    /// Language (BCP47 format: "en", "ja", "ko")
    #[schema(example = "en")]
    pub language: Option<String>,

    /// Reading direction (ltr, rtl, ttb or webtoon)
    #[schema(example = "ltr")]
    pub reading_direction: Option<String>,

    /// Release year
    #[schema(example = 1987)]
    pub year: Option<i32>,

    /// Expected total book count (for ongoing series)
    #[schema(example = 4)]
    pub total_book_count: Option<i32>,

    /// Custom JSON metadata
    #[schema(value_type = Option<Object>, example = json!({"myField": "value"}))]
    pub custom_metadata: Option<serde_json::Value>,

    // Lock states
    /// Lock states for all metadata fields
    pub locks: MetadataLocks,

    // Related data
    /// Genres assigned to this series
    pub genres: Vec<GenreDto>,

    /// Tags assigned to this series
    pub tags: Vec<TagDto>,

    /// Alternate titles for this series
    pub alternate_titles: Vec<AlternateTitleDto>,

    /// External ratings from various sources
    pub external_ratings: Vec<ExternalRatingDto>,

    /// External links to other sites
    pub external_links: Vec<ExternalLinkDto>,

    /// Timestamps
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Nested metadata object for FullSeriesResponse
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesFullMetadata {
    /// Series title
    #[schema(example = "Batman: Year One")]
    pub title: String,

    /// Custom sort name for ordering
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Batman Year One")]
    pub title_sort: Option<String>,

    /// Series description/summary
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "The definitive origin story of Batman.")]
    pub summary: Option<String>,

    /// Publisher name
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "DC Comics")]
    pub publisher: Option<String>,

    /// Imprint (sub-publisher)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Vertigo")]
    pub imprint: Option<String>,

    /// Series status (ongoing, ended, hiatus, abandoned, unknown)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "ended")]
    pub status: Option<String>,

    /// Age rating (e.g., 13, 16, 18)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 16)]
    pub age_rating: Option<i32>,

    /// Language (BCP47 format: "en", "ja", "ko")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "en")]
    pub language: Option<String>,

    /// Reading direction (ltr, rtl, ttb or webtoon)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "ltr")]
    pub reading_direction: Option<String>,

    /// Release year
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 1987)]
    pub year: Option<i32>,

    /// Expected total book count (for ongoing series)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 4)]
    pub total_book_count: Option<i32>,

    /// Custom JSON metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>, example = json!({"myField": "value"}))]
    pub custom_metadata: Option<serde_json::Value>,

    /// Lock states for all metadata fields
    pub locks: MetadataLocks,

    /// When the metadata was created
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the metadata was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Full series response including series data and complete metadata
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FullSeriesResponse {
    /// Series unique identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub id: uuid::Uuid,

    /// Library unique identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub library_id: uuid::Uuid,

    /// Name of the library this series belongs to
    #[schema(example = "Comics")]
    pub library_name: String,

    /// Total number of books in this series
    #[schema(example = 4)]
    pub book_count: i64,

    /// Number of unread books in this series (user-specific)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 2)]
    pub unread_count: Option<i64>,

    /// Filesystem path to the series directory
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "/media/comics/Batman - Year One")]
    pub path: Option<String>,

    /// Selected cover source (e.g., "first_book", "custom")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "first_book")]
    pub selected_cover_source: Option<String>,

    /// Whether the series has a custom cover uploaded
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub has_custom_cover: Option<bool>,

    /// Complete series metadata
    pub metadata: SeriesFullMetadata,

    /// Genres assigned to this series
    pub genres: Vec<GenreDto>,

    /// Tags assigned to this series
    pub tags: Vec<TagDto>,

    /// Alternate titles for this series
    pub alternate_titles: Vec<AlternateTitleDto>,

    /// External ratings from various sources
    pub external_ratings: Vec<ExternalRatingDto>,

    /// External links to other sites
    pub external_links: Vec<ExternalLinkDto>,

    /// When the series was created
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the series was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Request to update metadata lock states
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMetadataLocksRequest {
    /// Whether to lock the title field
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub title: Option<bool>,

    /// Whether to lock the title_sort field
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub title_sort: Option<bool>,

    /// Whether to lock the summary field
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = true)]
    pub summary: Option<bool>,

    /// Whether to lock the publisher field
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub publisher: Option<bool>,

    /// Whether to lock the imprint field
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub imprint: Option<bool>,

    /// Whether to lock the status field
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub status: Option<bool>,

    /// Whether to lock the age_rating field
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub age_rating: Option<bool>,

    /// Whether to lock the language field
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub language: Option<bool>,

    /// Whether to lock the reading_direction field
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub reading_direction: Option<bool>,

    /// Whether to lock the year field
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub year: Option<bool>,

    /// Whether to lock the total_book_count field
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub total_book_count: Option<bool>,

    /// Whether to lock the genres
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub genres: Option<bool>,

    /// Whether to lock the tags
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub tags: Option<bool>,

    /// Whether to lock the custom_metadata field
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub custom_metadata: Option<bool>,
}

// ============================================================================
// Cover DTOs
// ============================================================================

/// Series cover data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesCoverDto {
    /// Cover ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440070")]
    pub id: uuid::Uuid,

    /// Series ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub series_id: uuid::Uuid,

    /// Cover source (e.g., "custom", "book:uuid", "mangabaka")
    #[schema(example = "custom")]
    pub source: String,

    /// Path to the cover image
    #[schema(example = "data/covers/550e8400-e29b-41d4-a716-446655440002.jpg")]
    pub path: String,

    /// Whether this cover is currently selected as the primary cover
    #[schema(example = true)]
    pub is_selected: bool,

    /// Image width in pixels (if known)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 800)]
    pub width: Option<i32>,

    /// Image height in pixels (if known)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 1200)]
    pub height: Option<i32>,

    /// When the cover was created
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the cover was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Response containing a list of series covers
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesCoverListResponse {
    /// List of covers
    pub covers: Vec<SeriesCoverDto>,
}

// ============================================================================
// Average Rating DTOs
// ============================================================================

/// Response containing the average community rating for a series
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesAverageRatingResponse {
    /// Average rating from all users (0-100 scale), null if no ratings exist
    #[schema(example = 78.5)]
    pub average: Option<f64>,

    /// Total number of user ratings for this series
    #[schema(example = 15)]
    pub count: u64,
}

// ============================================================================
// Series Update DTOs
// ============================================================================

/// PATCH request for updating series title
///
/// Only provided fields will be updated. Absent fields are unchanged.
/// Explicitly null fields will be cleared (where applicable).
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatchSeriesRequest {
    /// Series title (stored in series_metadata.title)
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "Batman: Year One", nullable = true)]
    pub title: super::patch::PatchValue<String>,
}

/// Response for series update
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesUpdateResponse {
    /// Series ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub id: uuid::Uuid,

    /// Updated title
    #[schema(example = "Batman: Year One")]
    pub title: String,

    /// Last update timestamp
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}
