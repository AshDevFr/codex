use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Request to update reading progress for a book
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateProgressRequest {
    /// Current page number (1-indexed)
    #[schema(example = 42)]
    pub current_page: i32,

    /// Progress as a percentage (0.0-1.0), used for EPUB books with reflowable content
    #[schema(example = 0.45)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_percentage: Option<f64>,

    /// Whether the book is marked as completed
    #[schema(example = false)]
    #[serde(default)]
    pub completed: bool,
}

/// Response containing reading progress for a book
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReadProgressResponse {
    /// Progress record ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// User ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub user_id: Uuid,

    /// Book ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub book_id: Uuid,

    /// Current page (1-indexed)
    #[schema(example = 42)]
    pub current_page: i32,

    /// Progress as a percentage (0.0-1.0)
    /// For EPUBs, this is the stored percentage from reflowable content
    /// For other formats, this is calculated from current_page / total_pages
    #[schema(example = 0.45)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_percentage: Option<f64>,

    /// Whether the book is completed
    #[schema(example = false)]
    pub completed: bool,

    /// When reading started
    #[schema(example = "2024-01-10T14:30:00Z")]
    pub started_at: DateTime<Utc>,

    /// When progress was last updated
    #[schema(example = "2024-01-15T18:45:00Z")]
    pub updated_at: DateTime<Utc>,

    /// When the book was completed (if completed)
    #[schema(example = "2024-01-20T20:00:00Z")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
}

impl From<crate::db::entities::read_progress::Model> for ReadProgressResponse {
    fn from(model: crate::db::entities::read_progress::Model) -> Self {
        Self {
            id: model.id,
            user_id: model.user_id,
            book_id: model.book_id,
            current_page: model.current_page,
            progress_percentage: model.progress_percentage,
            completed: model.completed,
            started_at: model.started_at,
            updated_at: model.updated_at,
            completed_at: model.completed_at,
        }
    }
}

/// Response containing a list of reading progress records
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ReadProgressListResponse {
    /// List of progress records
    pub progress: Vec<ReadProgressResponse>,

    /// Total count
    #[schema(example = 25)]
    pub total: usize,
}

/// Response for bulk mark as read/unread operations
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MarkReadResponse {
    /// Number of books affected
    #[schema(example = 5)]
    pub count: usize,

    /// Message describing the operation
    #[schema(example = "Marked 5 books as read")]
    pub message: String,
}

// ============================================================================
// Bulk Operations DTOs
// ============================================================================

/// Request to perform bulk operations on multiple books
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkBooksRequest {
    /// List of book IDs to operate on
    #[schema(example = json!(["550e8400-e29b-41d4-a716-446655440001", "550e8400-e29b-41d4-a716-446655440002"]))]
    pub book_ids: Vec<Uuid>,
}

/// Request to perform bulk analyze operations on multiple books
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkAnalyzeBooksRequest {
    /// List of book IDs to analyze
    #[schema(example = json!(["550e8400-e29b-41d4-a716-446655440001", "550e8400-e29b-41d4-a716-446655440002"]))]
    pub book_ids: Vec<Uuid>,

    /// Whether to force re-analysis of already analyzed books
    #[serde(default)]
    #[schema(example = false)]
    pub force: bool,
}

/// Request to perform bulk operations on multiple series
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkSeriesRequest {
    /// List of series IDs to operate on
    #[schema(example = json!(["550e8400-e29b-41d4-a716-446655440001", "550e8400-e29b-41d4-a716-446655440002"]))]
    pub series_ids: Vec<Uuid>,
}

/// Request to perform bulk analyze operations on multiple series
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkAnalyzeSeriesRequest {
    /// List of series IDs to analyze
    #[schema(example = json!(["550e8400-e29b-41d4-a716-446655440001", "550e8400-e29b-41d4-a716-446655440002"]))]
    pub series_ids: Vec<Uuid>,

    /// Whether to force re-analysis of already analyzed books
    #[serde(default)]
    #[schema(example = false)]
    pub force: bool,
}

/// Response for bulk analyze operations
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkAnalyzeResponse {
    /// Number of analysis tasks enqueued
    #[schema(example = 5)]
    pub tasks_enqueued: usize,

    /// Message describing the operation
    #[schema(example = "Enqueued 5 analysis tasks")]
    pub message: String,
}

/// Request to generate thumbnails for books in bulk (by book IDs)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkGenerateBookThumbnailsRequest {
    /// List of book IDs to generate thumbnails for
    #[schema(example = json!(["550e8400-e29b-41d4-a716-446655440001", "550e8400-e29b-41d4-a716-446655440002"]))]
    pub book_ids: Vec<Uuid>,

    /// If true, regenerate thumbnails even if they exist
    #[serde(default)]
    #[schema(example = false)]
    pub force: bool,
}

/// Request to generate book thumbnails for multiple series in bulk
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkGenerateSeriesBookThumbnailsRequest {
    /// List of series IDs to generate book thumbnails for
    #[schema(example = json!(["550e8400-e29b-41d4-a716-446655440001", "550e8400-e29b-41d4-a716-446655440002"]))]
    pub series_ids: Vec<Uuid>,

    /// If true, regenerate thumbnails even if they exist
    #[serde(default)]
    #[schema(example = false)]
    pub force: bool,
}

/// Request to generate series thumbnails in bulk
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkGenerateSeriesThumbnailsRequest {
    /// List of series IDs to generate thumbnails for
    #[schema(example = json!(["550e8400-e29b-41d4-a716-446655440001", "550e8400-e29b-41d4-a716-446655440002"]))]
    pub series_ids: Vec<Uuid>,

    /// If true, regenerate thumbnails even if they exist
    #[serde(default)]
    #[schema(example = false)]
    pub force: bool,
}

/// Request to reprocess series titles in bulk
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkReprocessSeriesTitlesRequest {
    /// List of series IDs to reprocess titles for
    #[schema(example = json!(["550e8400-e29b-41d4-a716-446655440001", "550e8400-e29b-41d4-a716-446655440002"]))]
    pub series_ids: Vec<Uuid>,
}

/// Response for bulk task operations
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkTaskResponse {
    /// ID of the fan-out task that was created
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub task_id: Uuid,

    /// Message describing the operation
    #[schema(example = "Thumbnail generation task queued for 5 series")]
    pub message: String,
}
