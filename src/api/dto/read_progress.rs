use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Request to update reading progress for a book
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateProgressRequest {
    /// Current page number (0-indexed)
    #[schema(example = 42)]
    pub current_page: i32,

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

    /// Current page (0-indexed)
    #[schema(example = 42)]
    pub current_page: i32,

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
