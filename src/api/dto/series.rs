use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::common::PaginatedResponse;

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
