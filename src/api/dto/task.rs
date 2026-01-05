use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Task information for tracking background operations
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskDto {
    /// Unique task identifier (library ID for scan tasks)
    pub task_id: String,
    /// Type of task (e.g., "scan")
    pub task_type: String,
    /// Current status of the task
    pub status: String,
    /// Description of what the task is doing
    pub description: String,
    /// When the task started
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// When the task completed (if finished)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Progress information (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<TaskProgressDto>,
}

/// Progress information for a task
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskProgressDto {
    /// Current progress value
    pub current: i64,
    /// Total expected value
    pub total: i64,
    /// Progress percentage (0-100)
    pub percentage: f64,
}
