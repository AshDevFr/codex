use serde::Serialize;
use utoipa::ToSchema;

/// Page data transfer object
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PageDto {
    /// Unique page identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: uuid::Uuid,

    /// Book this page belongs to
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub book_id: uuid::Uuid,

    /// Page number within the book (1-indexed)
    #[schema(example = 1)]
    pub page_number: i32,

    /// Original filename within the archive
    #[schema(example = "page_001.jpg")]
    pub file_name: String,

    /// Image format (jpg, png, webp, etc.)
    #[schema(example = "jpg")]
    pub file_format: String,

    /// File size in bytes
    #[schema(example = 524288)]
    pub file_size: i64,

    /// Image width in pixels
    #[schema(example = 1200)]
    pub width: Option<i32>,

    /// Image height in pixels
    #[schema(example = 1800)]
    pub height: Option<i32>,
}
