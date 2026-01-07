use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Task types supported by the distributed task queue
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskType {
    /// Scan a library for new/changed books
    ScanLibrary {
        library_id: Uuid,
        #[serde(default = "default_mode")]
        mode: String, // "normal" or "deep"
    },

    /// Analyze a single book's metadata
    AnalyzeBook { book_id: Uuid },

    /// Analyze all books in a series
    AnalyzeSeries {
        series_id: Uuid,
        #[serde(default = "default_concurrency")]
        concurrency: usize,
    },

    /// Purge soft-deleted books from a library
    PurgeDeleted { library_id: Uuid },

    /// Refresh metadata from external source
    RefreshMetadata {
        book_id: Uuid,
        source: String, // "comicvine", "openlibrary", etc.
    },

    /// Generate missing thumbnails
    GenerateThumbnails {
        library_id: Option<Uuid>, // None = all libraries
    },
}

fn default_mode() -> String {
    "normal".to_string()
}

fn default_concurrency() -> usize {
    4
}

impl TaskType {
    /// Extract task type string for database storage
    pub fn type_string(&self) -> &'static str {
        match self {
            TaskType::ScanLibrary { .. } => "scan_library",
            TaskType::AnalyzeBook { .. } => "analyze_book",
            TaskType::AnalyzeSeries { .. } => "analyze_series",
            TaskType::PurgeDeleted { .. } => "purge_deleted",
            TaskType::RefreshMetadata { .. } => "refresh_metadata",
            TaskType::GenerateThumbnails { .. } => "generate_thumbnails",
        }
    }

    /// Extract library_id if present
    pub fn library_id(&self) -> Option<Uuid> {
        match self {
            TaskType::ScanLibrary { library_id, .. } => Some(*library_id),
            TaskType::PurgeDeleted { library_id } => Some(*library_id),
            TaskType::GenerateThumbnails { library_id } => *library_id,
            _ => None,
        }
    }

    /// Extract series_id if present
    pub fn series_id(&self) -> Option<Uuid> {
        match self {
            TaskType::AnalyzeSeries { series_id, .. } => Some(*series_id),
            _ => None,
        }
    }

    /// Extract book_id if present
    pub fn book_id(&self) -> Option<Uuid> {
        match self {
            TaskType::AnalyzeBook { book_id } => Some(*book_id),
            TaskType::RefreshMetadata { book_id, .. } => Some(*book_id),
            _ => None,
        }
    }

    /// Get task-specific parameters as JSON
    pub fn params(&self) -> serde_json::Value {
        match self {
            TaskType::ScanLibrary { mode, .. } => {
                serde_json::json!({ "mode": mode })
            }
            TaskType::AnalyzeSeries { concurrency, .. } => {
                serde_json::json!({ "concurrency": concurrency })
            }
            TaskType::RefreshMetadata { source, .. } => {
                serde_json::json!({ "source": source })
            }
            _ => serde_json::json!({}),
        }
    }

    /// Extract all fields needed for database insertion
    /// Returns: (type_string, library_id, series_id, book_id, params)
    pub fn extract_fields(
        &self,
    ) -> (
        &'static str,
        Option<Uuid>,
        Option<Uuid>,
        Option<Uuid>,
        Option<serde_json::Value>,
    ) {
        let type_str = self.type_string();
        let library_id = self.library_id();
        let series_id = self.series_id();
        let book_id = self.book_id();
        let params = self.params();

        let params_value = if params.is_null() || params.as_object().map_or(false, |o| o.is_empty())
        {
            None
        } else {
            Some(params)
        };

        (type_str, library_id, series_id, book_id, params_value)
    }
}

/// Task execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub success: bool,
    pub message: Option<String>,
    pub data: Option<serde_json::Value>,
}

impl TaskResult {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: Some(message.into()),
            data: None,
        }
    }

    pub fn success_with_data(message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            success: true,
            message: Some(message.into()),
            data: Some(data),
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: Some(message.into()),
            data: None,
        }
    }
}

/// Task queue statistics
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskStats {
    pub pending: u64,
    pub processing: u64,
    pub completed: u64,
    pub failed: u64,
    pub stale: u64,
    pub total: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_type_extraction() {
        let library_id = Uuid::new_v4();
        let task = TaskType::ScanLibrary {
            library_id,
            mode: "deep".to_string(),
        };

        assert_eq!(task.type_string(), "scan_library");

        let (type_str, lib_id, series_id, book_id, params) = task.extract_fields();
        assert_eq!(type_str, "scan_library");
        assert_eq!(lib_id, Some(library_id));
        assert_eq!(series_id, None);
        assert_eq!(book_id, None);
        assert!(params.is_some());
    }

    #[test]
    fn test_analyze_book_extraction() {
        let book_id = Uuid::new_v4();
        let task = TaskType::AnalyzeBook { book_id };

        assert_eq!(task.type_string(), "analyze_book");

        let (_, lib_id, series_id, extracted_book_id, _) = task.extract_fields();
        assert_eq!(lib_id, None);
        assert_eq!(series_id, None);
        assert_eq!(extracted_book_id, Some(book_id));
    }

    #[test]
    fn test_task_result_success() {
        let result = TaskResult::success("Task completed");
        assert!(result.success);
        assert_eq!(result.message, Some("Task completed".to_string()));
        assert!(result.data.is_none());
    }

    #[test]
    fn test_task_result_failure() {
        let result = TaskResult::failure("Task failed");
        assert!(!result.success);
        assert_eq!(result.message, Some("Task failed".to_string()));
    }

    #[test]
    fn test_task_result_with_data() {
        use serde_json::json;
        let data = json!({"count": 42});
        let result = TaskResult::success_with_data("Done", data.clone());
        assert!(result.success);
        assert_eq!(result.data, Some(data));
    }

    #[test]
    fn test_task_stats_total() {
        let stats = TaskStats {
            pending: 5,
            processing: 3,
            completed: 10,
            failed: 2,
            stale: 1,
            total: 21,
        };
        assert_eq!(stats.total, 21);
        assert_eq!(
            stats.pending + stats.processing + stats.completed + stats.failed,
            20
        );
    }
}
