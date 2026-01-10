use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::common::PaginatedResponse;
use super::read_progress::ReadProgressResponse;

/// Book data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookDto {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub id: uuid::Uuid,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub series_id: uuid::Uuid,
    #[schema(example = "Batman: Year One")]
    pub series_name: String,
    #[schema(example = "Batman: Year One #1")]
    pub title: String,
    #[schema(example = "batman year one 001")]
    pub sort_title: Option<String>,
    #[schema(example = "/media/comics/Batman/Batman - Year One 001.cbz")]
    pub file_path: String,
    #[schema(example = "cbz")]
    pub file_format: String,
    #[schema(example = 52428800)]
    pub file_size: i64,
    #[schema(example = "a1b2c3d4e5f6g7h8i9j0")]
    pub file_hash: String,
    #[schema(example = 32)]
    pub page_count: i32,
    #[schema(example = 1)]
    pub number: Option<i32>,
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_progress: Option<ReadProgressResponse>,
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
    #[schema(example = "550e8400-e29b-41d4-a716-446655440003")]
    pub id: uuid::Uuid,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub book_id: uuid::Uuid,
    #[schema(example = "Batman: Year One #1")]
    pub title: Option<String>,
    #[schema(example = "Batman: Year One")]
    pub series: Option<String>,
    #[schema(example = "1")]
    pub number: Option<String>,
    #[schema(
        example = "Bruce Wayne returns to Gotham City after years abroad to begin his war on crime."
    )]
    pub summary: Option<String>,
    #[schema(example = "DC Comics")]
    pub publisher: Option<String>,
    #[schema(example = "DC Black Label")]
    pub imprint: Option<String>,
    #[schema(example = "Superhero")]
    pub genre: Option<String>,
    #[schema(example = 32)]
    pub page_count: Option<i32>,
    #[schema(example = "en")]
    pub language_iso: Option<String>,
    #[schema(example = "1987-02-01T00:00:00Z")]
    pub release_date: Option<DateTime<Utc>>,
    #[schema(example = json!(["Frank Miller"]))]
    pub writers: Vec<String>,
    #[schema(example = json!(["David Mazzucchelli"]))]
    pub pencillers: Vec<String>,
    #[schema(example = json!(["David Mazzucchelli"]))]
    pub inkers: Vec<String>,
    #[schema(example = json!(["Richmond Lewis"]))]
    pub colorists: Vec<String>,
    #[schema(example = json!(["Todd Klein"]))]
    pub letterers: Vec<String>,
    #[schema(example = json!(["David Mazzucchelli"]))]
    pub cover_artists: Vec<String>,
    #[schema(example = json!(["Dennis O'Neil"]))]
    pub editors: Vec<String>,
}
