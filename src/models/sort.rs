//! Sort parameters for list queries.
//!
//! Lives in `models` so db repositories can take typed sort parameters
//! without depending on the api layer where the public DTO names also live.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
    /// Sort by user rating (user-specific)
    Rating,
    /// Sort by community average rating
    CommunityRating,
    /// Sort by external rating (highest external source rating)
    ExternalRating,
    /// Sort by fuzzy-search relevance score. Only meaningful when a
    /// `fullTextSearch` query is present and `search.fuzzy.enabled` is on;
    /// otherwise handlers fall back to the natural default (`Name`).
    Relevance,
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
            SeriesSortField::Rating => write!(f, "rating"),
            SeriesSortField::CommunityRating => write!(f, "community_rating"),
            SeriesSortField::ExternalRating => write!(f, "external_rating"),
            SeriesSortField::Relevance => write!(f, "relevance"),
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
            "rating" | "user_rating" => Ok(SeriesSortField::Rating),
            "community_rating" | "avg_rating" => Ok(SeriesSortField::CommunityRating),
            "external_rating" => Ok(SeriesSortField::ExternalRating),
            "relevance" | "score" => Ok(SeriesSortField::Relevance),
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

    /// Parse from "field,direction" format (e.g., "name,asc").
    ///
    /// "relevance" (with or without a direction) is accepted as a shorthand
    /// that pairs with a `fullTextSearch` query.
    pub fn parse(s: &str) -> Self {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("relevance") || trimmed.eq_ignore_ascii_case("score") {
            return Self {
                field: SeriesSortField::Relevance,
                direction: SortDirection::Desc,
            };
        }

        let parts: Vec<&str> = trimmed.split(',').collect();
        if parts.len() != 2 {
            return Self::default();
        }

        let field = SeriesSortField::from_str(parts[0]).unwrap_or_default();
        let direction = SortDirection::from_str(parts[1]).unwrap_or_default();

        Self { field, direction }
    }

    /// Check if this sort requires user-specific data (e.g., read progress)
    pub fn requires_user_context(&self) -> bool {
        matches!(
            self.field,
            SeriesSortField::DateRead | SeriesSortField::Rating
        )
    }

    /// Check if this sort requires aggregation
    pub fn requires_aggregation(&self) -> bool {
        matches!(
            self.field,
            SeriesSortField::BookCount
                | SeriesSortField::Rating
                | SeriesSortField::CommunityRating
                | SeriesSortField::ExternalRating
        )
    }
}

impl fmt::Display for SeriesSortParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{}", self.field, self.direction)
    }
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
    /// Sort by fuzzy-search relevance score. Only meaningful when a
    /// `fullTextSearch` query is present and `search.fuzzy.enabled` is on;
    /// otherwise handlers fall back to the natural default (`Title`).
    Relevance,
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
            BookSortField::Relevance => write!(f, "relevance"),
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
            "relevance" | "score" => Ok(BookSortField::Relevance),
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
    /// Parse from "field,direction" format (e.g., "title,asc").
    ///
    /// "relevance" (with or without a direction) is accepted as a shorthand
    /// that pairs with a `fullTextSearch` query.
    pub fn parse(s: &str) -> Self {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("relevance") || trimmed.eq_ignore_ascii_case("score") {
            return Self {
                field: BookSortField::Relevance,
                direction: SortDirection::Desc,
            };
        }

        let parts: Vec<&str> = trimmed.split(',').collect();
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
