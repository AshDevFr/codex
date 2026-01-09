use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// A group of duplicate books
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DuplicateGroup {
    pub id: Uuid,
    pub file_hash: String,
    pub book_ids: Vec<Uuid>,
    pub duplicate_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// Response for listing duplicates
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListDuplicatesResponse {
    pub duplicates: Vec<DuplicateGroup>,
    pub total_groups: usize,
    pub total_duplicate_books: usize,
}

/// Response for triggering a duplicate scan
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TriggerDuplicateScanResponse {
    pub task_id: Uuid,
    pub message: String,
}
