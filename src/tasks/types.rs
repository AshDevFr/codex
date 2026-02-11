//! Task types supported by the distributed task queue
//!
//! TODO: Remove allow(dead_code) once all task features are fully integrated

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Task types supported by the distributed task queue
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskType {
    /// Scan a library for new/changed books
    ScanLibrary {
        #[serde(rename = "libraryId")]
        library_id: Uuid,
        #[serde(default = "default_mode")]
        mode: String, // "normal" or "deep"
    },

    /// Analyze a single book's metadata
    AnalyzeBook {
        #[serde(rename = "bookId")]
        book_id: Uuid,
        #[serde(default)]
        force: bool,
    },

    /// Analyze all books in a series (always forces re-analysis)
    AnalyzeSeries {
        #[serde(rename = "seriesId")]
        series_id: Uuid,
    },

    /// Purge soft-deleted books from a library
    PurgeDeleted {
        #[serde(rename = "libraryId")]
        library_id: Uuid,
    },

    /// Refresh metadata from external source
    RefreshMetadata {
        #[serde(rename = "bookId")]
        book_id: Uuid,
        source: String, // "comicvine", "openlibrary", etc.
    },

    /// Generate thumbnails for books in a scope (library, series, specific books, or all)
    /// This is a fan-out task that enqueues individual GenerateThumbnail tasks
    GenerateThumbnails {
        #[serde(rename = "libraryId")]
        library_id: Option<Uuid>, // If set, only books in this library
        #[serde(rename = "seriesId")]
        series_id: Option<Uuid>, // If set, only books in this series (takes precedence over library_id)
        #[serde(rename = "seriesIds", default)]
        series_ids: Option<Vec<Uuid>>, // If set, only books in these specific series (takes precedence over series_id and library_id)
        #[serde(rename = "bookIds", default)]
        book_ids: Option<Vec<Uuid>>, // If set, only these specific books (takes precedence over all other scopes)
        #[serde(default)]
        force: bool, // If true, regenerate all thumbnails; if false, only missing ones
    },

    /// Generate thumbnail for a single book
    GenerateThumbnail {
        #[serde(rename = "bookId")]
        book_id: Uuid,
        #[serde(default)]
        force: bool, // If true, regenerate even if thumbnail exists
    },

    /// Generate thumbnail for a series (from first book's cover)
    GenerateSeriesThumbnail {
        #[serde(rename = "seriesId")]
        series_id: Uuid,
        #[serde(default)]
        force: bool, // If true, regenerate even if thumbnail exists
    },

    /// Generate thumbnails for series in a scope (library, specific series, or all)
    /// This is a fan-out task that enqueues individual GenerateSeriesThumbnail tasks
    GenerateSeriesThumbnails {
        #[serde(rename = "libraryId")]
        library_id: Option<Uuid>, // If set, only series in this library
        #[serde(rename = "seriesIds", default)]
        series_ids: Option<Vec<Uuid>>, // If set, only these specific series (takes precedence over library_id)
        #[serde(default)]
        force: bool, // If true, regenerate all thumbnails; if false, only missing ones
    },

    /// Find and catalog duplicate books across all libraries
    FindDuplicates,

    /// Clean up files for a deleted book (thumbnail + cover references)
    CleanupBookFiles {
        #[serde(rename = "bookId")]
        book_id: Uuid,
        /// Optional thumbnail path (if known at deletion time)
        #[serde(rename = "thumbnailPath", default)]
        thumbnail_path: Option<String>,
        /// Optional series_id to invalidate series thumbnail cache
        #[serde(rename = "seriesId", default)]
        series_id: Option<Uuid>,
    },

    /// Clean up files for a deleted series (cover files)
    CleanupSeriesFiles {
        #[serde(rename = "seriesId")]
        series_id: Uuid,
    },

    /// Scan filesystem for orphaned files and delete them
    CleanupOrphanedFiles,

    /// Clean up old pages from the PDF page cache
    CleanupPdfCache,

    /// Auto-match metadata for a series using a plugin
    PluginAutoMatch {
        #[serde(rename = "seriesId")]
        series_id: Uuid,
        #[serde(rename = "pluginId")]
        plugin_id: Uuid,
        /// Source scope that triggered this task (for tracking)
        #[serde(rename = "sourceScope", default)]
        source_scope: Option<String>, // "series:detail", "series:bulk", "library:detail", "library:scan"
    },

    /// Reprocess a single series title using library preprocessing rules
    ReprocessSeriesTitle {
        #[serde(rename = "seriesId")]
        series_id: Uuid,
    },

    /// Reprocess series titles in a scope (library, bulk selection, or specific series)
    /// This is a fan-out task that enqueues individual ReprocessSeriesTitle tasks
    ReprocessSeriesTitles {
        #[serde(rename = "libraryId")]
        library_id: Option<Uuid>, // If set, process all series in this library
        #[serde(rename = "seriesIds", default)]
        series_ids: Option<Vec<Uuid>>, // If set, process only these specific series (bulk selection)
    },

    /// Clean up expired plugin storage data across all user plugins
    CleanupPluginData,

    /// Sync user plugin data with external service
    UserPluginSync {
        #[serde(rename = "pluginId")]
        plugin_id: Uuid,
        #[serde(rename = "userId")]
        user_id: Uuid,
    },

    /// Refresh recommendations from a user plugin
    UserPluginRecommendations {
        #[serde(rename = "pluginId")]
        plugin_id: Uuid,
        #[serde(rename = "userId")]
        user_id: Uuid,
    },

    /// Notify a plugin that a recommendation was dismissed
    UserPluginRecommendationDismiss {
        #[serde(rename = "pluginId")]
        plugin_id: Uuid,
        #[serde(rename = "userId")]
        user_id: Uuid,
        #[serde(rename = "externalId")]
        external_id: String,
        #[serde(default)]
        reason: Option<String>,
    },
}

fn default_mode() -> String {
    "normal".to_string()
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
            TaskType::GenerateThumbnail { .. } => "generate_thumbnail",
            TaskType::GenerateSeriesThumbnail { .. } => "generate_series_thumbnail",
            TaskType::GenerateSeriesThumbnails { .. } => "generate_series_thumbnails",
            TaskType::FindDuplicates => "find_duplicates",
            TaskType::CleanupBookFiles { .. } => "cleanup_book_files",
            TaskType::CleanupSeriesFiles { .. } => "cleanup_series_files",
            TaskType::CleanupOrphanedFiles => "cleanup_orphaned_files",
            TaskType::CleanupPdfCache => "cleanup_pdf_cache",
            TaskType::PluginAutoMatch { .. } => "plugin_auto_match",
            TaskType::ReprocessSeriesTitle { .. } => "reprocess_series_title",
            TaskType::ReprocessSeriesTitles { .. } => "reprocess_series_titles",
            TaskType::CleanupPluginData => "cleanup_plugin_data",
            TaskType::UserPluginSync { .. } => "user_plugin_sync",
            TaskType::UserPluginRecommendations { .. } => "user_plugin_recommendations",
            TaskType::UserPluginRecommendationDismiss { .. } => {
                "user_plugin_recommendation_dismiss"
            }
        }
    }

    /// Extract library_id if present
    pub fn library_id(&self) -> Option<Uuid> {
        match self {
            TaskType::ScanLibrary { library_id, .. } => Some(*library_id),
            TaskType::PurgeDeleted { library_id } => Some(*library_id),
            TaskType::GenerateThumbnails { library_id, .. } => *library_id,
            TaskType::GenerateSeriesThumbnails { library_id, .. } => *library_id,
            TaskType::ReprocessSeriesTitles { library_id, .. } => *library_id,
            _ => None,
        }
    }

    /// Get task-specific parameters as JSON
    pub fn params(&self) -> serde_json::Value {
        match self {
            TaskType::ScanLibrary { mode, .. } => {
                serde_json::json!({ "mode": mode })
            }
            TaskType::AnalyzeBook { force, .. } => {
                serde_json::json!({ "force": force })
            }
            TaskType::AnalyzeSeries { .. } => {
                serde_json::json!({})
            }
            TaskType::RefreshMetadata { source, .. } => {
                serde_json::json!({ "source": source })
            }
            TaskType::GenerateThumbnails {
                force,
                book_ids,
                series_ids,
                ..
            } => {
                serde_json::json!({ "force": force, "book_ids": book_ids, "series_ids": series_ids })
            }
            TaskType::GenerateThumbnail { force, .. } => {
                serde_json::json!({ "force": force })
            }
            TaskType::GenerateSeriesThumbnail { force, .. } => {
                serde_json::json!({ "force": force })
            }
            TaskType::GenerateSeriesThumbnails {
                force, series_ids, ..
            } => {
                serde_json::json!({ "force": force, "series_ids": series_ids })
            }
            TaskType::CleanupBookFiles {
                book_id,
                thumbnail_path,
                series_id,
            } => {
                // Store book_id in params since the FK column can't reference deleted books
                serde_json::json!({ "book_id": book_id, "thumbnail_path": thumbnail_path, "series_id": series_id })
            }
            TaskType::CleanupSeriesFiles { series_id } => {
                // Store series_id in params since the FK column can't reference deleted series
                serde_json::json!({ "series_id": series_id })
            }
            TaskType::PluginAutoMatch {
                plugin_id,
                source_scope,
                ..
            } => {
                serde_json::json!({ "plugin_id": plugin_id, "source_scope": source_scope })
            }
            TaskType::ReprocessSeriesTitles { series_ids, .. } => {
                serde_json::json!({ "series_ids": series_ids })
            }
            TaskType::UserPluginSync { plugin_id, user_id } => {
                serde_json::json!({ "plugin_id": plugin_id, "user_id": user_id })
            }
            TaskType::UserPluginRecommendations { plugin_id, user_id } => {
                serde_json::json!({ "plugin_id": plugin_id, "user_id": user_id })
            }
            TaskType::UserPluginRecommendationDismiss {
                plugin_id,
                user_id,
                external_id,
                reason,
            } => {
                serde_json::json!({
                    "plugin_id": plugin_id,
                    "user_id": user_id,
                    "external_id": external_id,
                    "reason": reason,
                })
            }
            _ => serde_json::json!({}),
        }
    }

    /// Extract series_id if present
    /// Note: CleanupSeriesFiles stores series_id in params, not as FK (entity may be deleted)
    pub fn series_id(&self) -> Option<Uuid> {
        match self {
            TaskType::AnalyzeSeries { series_id, .. } => Some(*series_id),
            TaskType::GenerateThumbnails { series_id, .. } => *series_id,
            TaskType::GenerateSeriesThumbnail { series_id, .. } => Some(*series_id),
            TaskType::PluginAutoMatch { series_id, .. } => Some(*series_id),
            TaskType::ReprocessSeriesTitle { series_id } => Some(*series_id),
            // CleanupSeriesFiles intentionally NOT included - series_id is stored in params
            // because the series may already be deleted when the task runs
            _ => None,
        }
    }

    /// Extract book_id if present
    /// Note: CleanupBookFiles stores book_id in params, not as FK (entity may be deleted)
    pub fn book_id(&self) -> Option<Uuid> {
        match self {
            TaskType::AnalyzeBook { book_id, .. } => Some(*book_id),
            TaskType::RefreshMetadata { book_id, .. } => Some(*book_id),
            TaskType::GenerateThumbnail { book_id, .. } => Some(*book_id),
            // CleanupBookFiles intentionally NOT included - book_id is stored in params
            // because the book is already deleted when the cleanup task runs
            _ => None,
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

        let params_value = if params.is_null() || params.as_object().is_some_and(|o| o.is_empty()) {
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
#[serde(rename_all = "camelCase")]
pub struct TaskStats {
    /// Total counts across all task types
    pub pending: u64,
    pub processing: u64,
    pub completed: u64,
    pub failed: u64,
    pub stale: u64,
    pub total: u64,
    /// Breakdown by task type and status
    pub by_type: std::collections::HashMap<String, TaskTypeStats>,
}

/// Statistics for a specific task type
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskTypeStats {
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
        let task = TaskType::AnalyzeBook {
            book_id,
            force: false,
        };

        assert_eq!(task.type_string(), "analyze_book");

        let (_, lib_id, series_id, extracted_book_id, params) = task.extract_fields();
        assert_eq!(lib_id, None);
        assert_eq!(series_id, None);
        assert_eq!(extracted_book_id, Some(book_id));
        assert!(params.is_some());
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
        use std::collections::HashMap;

        let stats = TaskStats {
            pending: 5,
            processing: 3,
            completed: 10,
            failed: 2,
            stale: 1,
            total: 21,
            by_type: HashMap::new(),
        };
        assert_eq!(stats.total, 21);
        assert_eq!(
            stats.pending + stats.processing + stats.completed + stats.failed,
            20
        );
    }

    #[test]
    fn test_generate_thumbnails_extraction() {
        let library_id = Uuid::new_v4();
        let series_id = Uuid::new_v4();

        // Library scope
        let task = TaskType::GenerateThumbnails {
            library_id: Some(library_id),
            series_id: None,
            series_ids: None,
            book_ids: None,
            force: false,
        };
        assert_eq!(task.type_string(), "generate_thumbnails");
        assert_eq!(task.library_id(), Some(library_id));
        assert_eq!(task.series_id(), None);
        assert_eq!(task.book_id(), None);

        let params = task.params();
        assert_eq!(params["force"], false);

        // Series scope
        let task = TaskType::GenerateThumbnails {
            library_id: None,
            series_id: Some(series_id),
            series_ids: None,
            book_ids: None,
            force: true,
        };
        assert_eq!(task.library_id(), None);
        assert_eq!(task.series_id(), Some(series_id));

        let params = task.params();
        assert_eq!(params["force"], true);

        // All scope
        let task = TaskType::GenerateThumbnails {
            library_id: None,
            series_id: None,
            series_ids: None,
            book_ids: None,
            force: false,
        };
        assert_eq!(task.library_id(), None);
        assert_eq!(task.series_id(), None);
    }

    #[test]
    fn test_generate_thumbnail_extraction() {
        let book_id = Uuid::new_v4();

        let task = TaskType::GenerateThumbnail {
            book_id,
            force: true,
        };

        assert_eq!(task.type_string(), "generate_thumbnail");
        assert_eq!(task.library_id(), None);
        assert_eq!(task.series_id(), None);
        assert_eq!(task.book_id(), Some(book_id));

        let params = task.params();
        assert_eq!(params["force"], true);

        // Test with force=false
        let task = TaskType::GenerateThumbnail {
            book_id,
            force: false,
        };
        let params = task.params();
        assert_eq!(params["force"], false);
    }

    #[test]
    fn test_generate_thumbnail_extract_fields() {
        let book_id = Uuid::new_v4();

        let task = TaskType::GenerateThumbnail {
            book_id,
            force: true,
        };

        let (type_str, lib_id, series_id, extracted_book_id, params) = task.extract_fields();
        assert_eq!(type_str, "generate_thumbnail");
        assert_eq!(lib_id, None);
        assert_eq!(series_id, None);
        assert_eq!(extracted_book_id, Some(book_id));
        assert!(params.is_some());
        assert_eq!(params.unwrap()["force"], true);
    }

    #[test]
    fn test_generate_thumbnails_extract_fields() {
        let library_id = Uuid::new_v4();
        let series_id = Uuid::new_v4();

        // With series_id (takes precedence)
        let task = TaskType::GenerateThumbnails {
            library_id: Some(library_id),
            series_id: Some(series_id),
            series_ids: None,
            book_ids: None,
            force: true,
        };

        let (type_str, lib_id, extracted_series_id, book_id, params) = task.extract_fields();
        assert_eq!(type_str, "generate_thumbnails");
        assert_eq!(lib_id, Some(library_id));
        assert_eq!(extracted_series_id, Some(series_id));
        assert_eq!(book_id, None);
        assert!(params.is_some());
        assert_eq!(params.unwrap()["force"], true);
    }

    #[test]
    fn test_generate_series_thumbnails_extraction() {
        let library_id = Uuid::new_v4();

        // Library scope
        let task = TaskType::GenerateSeriesThumbnails {
            library_id: Some(library_id),
            series_ids: None,
            force: false,
        };
        assert_eq!(task.type_string(), "generate_series_thumbnails");
        assert_eq!(task.library_id(), Some(library_id));
        assert_eq!(task.series_id(), None);
        assert_eq!(task.book_id(), None);

        let params = task.params();
        assert_eq!(params["force"], false);

        // All scope
        let task = TaskType::GenerateSeriesThumbnails {
            library_id: None,
            series_ids: None,
            force: true,
        };
        assert_eq!(task.library_id(), None);
        assert_eq!(task.series_id(), None);

        let params = task.params();
        assert_eq!(params["force"], true);
    }

    #[test]
    fn test_generate_series_thumbnails_extract_fields() {
        let library_id = Uuid::new_v4();

        let task = TaskType::GenerateSeriesThumbnails {
            library_id: Some(library_id),
            series_ids: None,
            force: true,
        };

        let (type_str, lib_id, series_id, book_id, params) = task.extract_fields();
        assert_eq!(type_str, "generate_series_thumbnails");
        assert_eq!(lib_id, Some(library_id));
        assert_eq!(series_id, None);
        assert_eq!(book_id, None);
        assert!(params.is_some());
        assert_eq!(params.unwrap()["force"], true);
    }

    #[test]
    fn test_cleanup_book_files_extraction() {
        let book_id = Uuid::new_v4();
        let series_id = Uuid::new_v4();

        // Without thumbnail path or series_id
        let task = TaskType::CleanupBookFiles {
            book_id,
            thumbnail_path: None,
            series_id: None,
        };

        assert_eq!(task.type_string(), "cleanup_book_files");
        // book_id is NOT returned from book_id() - it's stored in params because
        // cleanup tasks reference deleted books that can't have FK constraints
        assert_eq!(task.book_id(), None);
        assert_eq!(task.library_id(), None);
        assert_eq!(task.series_id(), None);

        let (type_str, lib_id, extracted_series_id, extracted_book_id, params) =
            task.extract_fields();
        assert_eq!(type_str, "cleanup_book_files");
        assert_eq!(lib_id, None);
        assert_eq!(extracted_series_id, None);
        assert_eq!(extracted_book_id, None); // Not using FK column
        // params should contain book_id, thumbnail_path, and series_id
        assert!(params.is_some());
        let params = params.unwrap();
        assert_eq!(params["book_id"], book_id.to_string());
        assert!(params["thumbnail_path"].is_null());
        assert!(params["series_id"].is_null());

        // With thumbnail path and series_id
        let task = TaskType::CleanupBookFiles {
            book_id,
            thumbnail_path: Some("/data/thumbnails/books/ab/abc123.jpg".to_string()),
            series_id: Some(series_id),
        };

        let params = task.params();
        assert_eq!(params["book_id"], book_id.to_string());
        assert_eq!(
            params["thumbnail_path"],
            "/data/thumbnails/books/ab/abc123.jpg"
        );
        assert_eq!(params["series_id"], series_id.to_string());
    }

    #[test]
    fn test_cleanup_series_files_extraction() {
        let series_id = Uuid::new_v4();

        let task = TaskType::CleanupSeriesFiles { series_id };

        assert_eq!(task.type_string(), "cleanup_series_files");
        // series_id is NOT returned from series_id() - it's stored in params because
        // cleanup tasks reference deleted series that can't have FK constraints
        assert_eq!(task.series_id(), None);
        assert_eq!(task.book_id(), None);
        assert_eq!(task.library_id(), None);

        let (type_str, lib_id, extracted_series_id, book_id, params) = task.extract_fields();
        assert_eq!(type_str, "cleanup_series_files");
        assert_eq!(lib_id, None);
        assert_eq!(extracted_series_id, None); // Not using FK column
        assert_eq!(book_id, None);
        // params should contain series_id
        assert!(params.is_some());
        let params = params.unwrap();
        assert_eq!(params["series_id"], series_id.to_string());
    }

    #[test]
    fn test_cleanup_orphaned_files_extraction() {
        let task = TaskType::CleanupOrphanedFiles;

        assert_eq!(task.type_string(), "cleanup_orphaned_files");
        assert_eq!(task.library_id(), None);
        assert_eq!(task.series_id(), None);
        assert_eq!(task.book_id(), None);

        let (type_str, lib_id, series_id, book_id, params) = task.extract_fields();
        assert_eq!(type_str, "cleanup_orphaned_files");
        assert_eq!(lib_id, None);
        assert_eq!(series_id, None);
        assert_eq!(book_id, None);
        assert!(params.is_none());
    }

    #[test]
    fn test_cleanup_task_serialization() {
        let book_id = Uuid::new_v4();
        let series_id = Uuid::new_v4();

        // Test CleanupBookFiles serialization
        let task = TaskType::CleanupBookFiles {
            book_id,
            thumbnail_path: Some("/path/to/thumb.jpg".to_string()),
            series_id: Some(series_id),
        };
        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("cleanup_book_files"));
        assert!(json.contains(&book_id.to_string()));

        // Test deserialization
        let deserialized: TaskType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.type_string(), "cleanup_book_files");

        // Test CleanupSeriesFiles serialization
        let task = TaskType::CleanupSeriesFiles { series_id };
        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("cleanup_series_files"));

        // Test CleanupOrphanedFiles serialization
        let task = TaskType::CleanupOrphanedFiles;
        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("cleanup_orphaned_files"));

        let deserialized: TaskType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.type_string(), "cleanup_orphaned_files");
    }

    #[test]
    fn test_user_plugin_recommendation_dismiss_extraction() {
        let plugin_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let task = TaskType::UserPluginRecommendationDismiss {
            plugin_id,
            user_id,
            external_id: "12345".to_string(),
            reason: Some("not_interested".to_string()),
        };

        assert_eq!(task.type_string(), "user_plugin_recommendation_dismiss");
        assert_eq!(task.library_id(), None);
        assert_eq!(task.series_id(), None);
        assert_eq!(task.book_id(), None);

        let params = task.params();
        assert_eq!(params["plugin_id"], plugin_id.to_string());
        assert_eq!(params["user_id"], user_id.to_string());
        assert_eq!(params["external_id"], "12345");
        assert_eq!(params["reason"], "not_interested");
    }

    #[test]
    fn test_user_plugin_recommendation_dismiss_extract_fields() {
        let plugin_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let task = TaskType::UserPluginRecommendationDismiss {
            plugin_id,
            user_id,
            external_id: "99".to_string(),
            reason: None,
        };

        let (type_str, lib_id, series_id, book_id, params) = task.extract_fields();
        assert_eq!(type_str, "user_plugin_recommendation_dismiss");
        assert_eq!(lib_id, None);
        assert_eq!(series_id, None);
        assert_eq!(book_id, None);
        assert!(params.is_some());
        let params = params.unwrap();
        assert_eq!(params["external_id"], "99");
        assert!(params["reason"].is_null());
    }

    #[test]
    fn test_user_plugin_recommendation_dismiss_serialization() {
        let plugin_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let task = TaskType::UserPluginRecommendationDismiss {
            plugin_id,
            user_id,
            external_id: "12345".to_string(),
            reason: Some("already_read".to_string()),
        };

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("user_plugin_recommendation_dismiss"));
        assert!(json.contains(&plugin_id.to_string()));
        assert!(json.contains("12345"));

        let deserialized: TaskType = serde_json::from_str(&json).unwrap();
        assert_eq!(
            deserialized.type_string(),
            "user_plugin_recommendation_dismiss"
        );
    }
}
