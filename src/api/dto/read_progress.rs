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
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ReadProgressResponse {
    /// Progress record ID
    pub id: Uuid,

    /// User ID
    pub user_id: Uuid,

    /// Book ID
    pub book_id: Uuid,

    /// Current page (0-indexed)
    #[schema(example = 42)]
    pub current_page: i32,

    /// Whether the book is completed
    #[schema(example = false)]
    pub completed: bool,

    /// When reading started
    pub started_at: DateTime<Utc>,

    /// When progress was last updated
    pub updated_at: DateTime<Utc>,

    /// When the book was completed (if completed)
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
    pub total: usize,
}
