use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::common::PaginatedResponse;

/// Book data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookDto {
    pub id: uuid::Uuid,
    pub series_id: uuid::Uuid,
    pub series_name: String,
    pub title: String,
    pub sort_title: Option<String>,
    pub file_path: String,
    pub file_format: String,
    pub file_size: i64,
    pub file_hash: String,
    pub page_count: i32,
    pub number: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Book list response
pub type BookListResponse = PaginatedResponse<BookDto>;

/// Detailed book response with metadata
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookDetailResponse {
    pub book: BookDto,
    pub metadata: Option<BookMetadataDto>,
}

/// Book metadata DTO
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookMetadataDto {
    pub id: uuid::Uuid,
    pub book_id: uuid::Uuid,
    pub title: Option<String>,
    pub series: Option<String>,
    pub number: Option<String>,
    pub summary: Option<String>,
    pub publisher: Option<String>,
    pub imprint: Option<String>,
    pub genre: Option<String>,
    pub page_count: Option<i32>,
    pub language_iso: Option<String>,
    pub release_date: Option<DateTime<Utc>>,
    pub writers: Vec<String>,
    pub pencillers: Vec<String>,
    pub inkers: Vec<String>,
    pub colorists: Vec<String>,
    pub letterers: Vec<String>,
    pub cover_artists: Vec<String>,
    pub editors: Vec<String>,
}
