use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// A group of duplicate books
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DuplicateGroup {
    /// Unique identifier for the duplicate group
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// SHA-256 hash of the file content
    #[schema(example = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")]
    pub file_hash: String,

    /// List of book IDs that share this hash
    pub book_ids: Vec<Uuid>,

    /// Number of duplicate copies found
    #[schema(example = 3)]
    pub duplicate_count: i32,

    /// When the duplicate was first detected
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub created_at: String,

    /// When the group was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: String,
}

/// Response for listing duplicates
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListDuplicatesResponse {
    /// List of duplicate groups
    pub duplicates: Vec<DuplicateGroup>,

    /// Total number of duplicate groups
    #[schema(example = 5)]
    pub total_groups: usize,

    /// Total number of books that are duplicates
    #[schema(example = 15)]
    pub total_duplicate_books: usize,
}

/// Response for triggering a duplicate scan
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TriggerDuplicateScanResponse {
    /// Task ID for tracking the scan progress
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub task_id: Uuid,

    /// Status message
    #[schema(example = "Duplicate scan started")]
    pub message: String,
}
