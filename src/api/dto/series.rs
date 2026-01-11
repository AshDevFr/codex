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
    /// Sort by series name (uses sort_name if available, otherwise name)
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
    /// Sort by total file size of all books in series
    FileSize,
    /// Sort by series path/filename
    Filename,
    /// Sort by total page count of all books in series
    PageCount,
}

impl fmt::Display for SeriesSortField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SeriesSortField::Name => write!(f, "name"),
            SeriesSortField::DateAdded => write!(f, "date_added"),
            SeriesSortField::DateUpdated => write!(f, "date_updated"),
            SeriesSortField::ReleaseDate => write!(f, "release_date"),
            SeriesSortField::DateRead => write!(f, "date_read"),
            SeriesSortField::FileSize => write!(f, "file_size"),
            SeriesSortField::Filename => write!(f, "filename"),
            SeriesSortField::PageCount => write!(f, "page_count"),
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
            "file_size" => Ok(SeriesSortField::FileSize),
            "filename" => Ok(SeriesSortField::Filename),
            "page_count" => Ok(SeriesSortField::PageCount),
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
        matches!(
            self.field,
            SeriesSortField::FileSize | SeriesSortField::PageCount
        )
    }
}

impl fmt::Display for SeriesSortParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{}", self.field, self.direction)
    }
}

/// Series data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesDto {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub id: uuid::Uuid,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub library_id: uuid::Uuid,
    #[schema(example = "Batman: Year One")]
    pub name: String,
    #[schema(example = "batman year one")]
    pub sort_name: Option<String>,
    #[schema(
        example = "The definitive origin story of Batman, following Bruce Wayne's first year as a vigilante."
    )]
    pub description: Option<String>,
    #[schema(example = "DC Comics")]
    pub publisher: Option<String>,
    #[schema(example = 1987)]
    pub year: Option<i32>,
    #[schema(example = 4)]
    pub book_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "/media/comics/Batman - Year One")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "first_book")]
    pub selected_cover_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub has_custom_cover: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 2)]
    pub unread_count: Option<i64>,
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Series list response
pub type SeriesListResponse = PaginatedResponse<SeriesDto>;

/// Series filter for list queries
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SeriesFilter {
    /// Optional library filter
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub library_id: Option<uuid::Uuid>,
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
}
