use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::common::PaginatedResponse;

/// Series data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesDto {
    pub id: uuid::Uuid,
    pub library_id: uuid::Uuid,
    pub name: String,
    pub sort_name: Option<String>,
    pub description: Option<String>,
    pub publisher: Option<String>,
    pub year: Option<i32>,
    pub book_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_cover_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_custom_cover: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Series list response
pub type SeriesListResponse = PaginatedResponse<SeriesDto>;

/// Series filter for list queries
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SeriesFilter {
    /// Optional library filter
    pub library_id: Option<uuid::Uuid>,
}

/// Search series request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchSeriesRequest {
    /// Search query
    pub query: String,

    /// Optional library filter
    pub library_id: Option<uuid::Uuid>,
}
