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

    /// Scheduled per-job metadata refresh.
    ///
    /// Loads the [`library_jobs`] row by `job_id`, decodes its config (single
    /// provider + field groups + safety options), walks the library's series,
    /// and refreshes metadata via the existing `MetadataApplier`.
    RefreshLibraryMetadata {
        #[serde(rename = "jobId")]
        job_id: Uuid,
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

    /// Renumber books in a single series using the library's number strategy
    RenumberSeries {
        #[serde(rename = "seriesId")]
        series_id: Uuid,
    },

    /// Renumber books in multiple series (fan-out task)
    /// This is a fan-out task that enqueues individual RenumberSeries tasks
    RenumberSeriesBatch {
        #[serde(rename = "seriesIds", default)]
        series_ids: Option<Vec<Uuid>>,
    },

    /// Clean up expired plugin storage data across all user plugins
    CleanupPluginData,

    /// Clean up expired series exports (files + DB records)
    CleanupSeriesExports,

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

    /// Export series data to a JSON or CSV file
    ExportSeries {
        #[serde(rename = "exportId")]
        export_id: Uuid,
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

    /// Backfill release-tracking aliases from existing series metadata.
    ///
    /// Walks series in scope, harvests the canonical title plus alternate titles
    /// from `series_metadata` and `series_alternate_titles`, and seeds them as
    /// `metadata`-source aliases in `series_aliases`. Idempotent — re-runs do
    /// not create duplicates. Does NOT enable tracking; that stays explicit.
    BackfillTrackingFromMetadata {
        /// If set, scope to this library; otherwise all series.
        #[serde(rename = "libraryId", default)]
        library_id: Option<Uuid>,
        /// If set, scope to these specific series (takes precedence over library_id).
        #[serde(rename = "seriesIds", default)]
        series_ids: Option<Vec<Uuid>>,
    },

    /// Poll a single `release_sources` row for new releases.
    ///
    /// Resolves the source's owning plugin, calls `releases/poll` over the
    /// existing plugin host, runs returned candidates through the matcher +
    /// threshold, and writes accepted candidates to the ledger. On success
    /// updates `last_polled_at` (and optionally `etag`); on failure records
    /// `last_error`. Idempotent: ledger writes dedup on
    /// `(source_id, external_release_id)` and `info_hash`.
    PollReleaseSource {
        #[serde(rename = "sourceId")]
        source_id: Uuid,
    },
}

fn default_mode() -> String {
    "normal".to_string()
}

impl TaskType {
    /// Returns the default priority for this task type.
    ///
    /// Higher values = more urgent. Uses large gaps for future insertions.
    /// Categories:
    ///   1000-900: Scanning (library discovery, post-scan cleanup)
    ///    800-750: Analysis (book/series analysis, title reprocessing)
    ///    600-570: Thumbnails (single and batch generation)
    ///    400-380: Metadata (deduplication, external lookups, plugin matching)
    ///    200-180: Plugins (user-facing plugin operations)
    ///        100: Cleanup (low-priority maintenance)
    pub fn default_priority(&self) -> i32 {
        match self {
            // Scanning
            TaskType::ScanLibrary { .. } => 1000,
            TaskType::PurgeDeleted { .. } => 900,
            // Analysis
            TaskType::AnalyzeBook { .. } => 800,
            TaskType::AnalyzeSeries { .. } => 790,
            TaskType::ReprocessSeriesTitle { .. } => 780,
            TaskType::ReprocessSeriesTitles { .. } => 770,
            TaskType::RenumberSeries { .. } => 760,
            TaskType::RenumberSeriesBatch { .. } => 750,
            // Thumbnails
            TaskType::GenerateThumbnail { .. } => 600,
            TaskType::GenerateSeriesThumbnail { .. } => 590,
            TaskType::GenerateThumbnails { .. } => 580,
            TaskType::GenerateSeriesThumbnails { .. } => 570,
            // Metadata
            TaskType::FindDuplicates => 400,
            TaskType::RefreshMetadata { .. } => 390,
            TaskType::RefreshLibraryMetadata { .. } => 385,
            TaskType::PluginAutoMatch { .. } => 380,
            // Export
            TaskType::ExportSeries { .. } => 450,
            // Plugins
            TaskType::UserPluginRecommendationDismiss { .. } => 200,
            TaskType::UserPluginSync { .. } => 190,
            TaskType::UserPluginRecommendations { .. } => 180,
            // Release tracking maintenance
            TaskType::BackfillTrackingFromMetadata { .. } => 150,
            // Release polling: scheduled background discovery
            TaskType::PollReleaseSource { .. } => 170,
            // Cleanup
            TaskType::CleanupBookFiles { .. }
            | TaskType::CleanupSeriesFiles { .. }
            | TaskType::CleanupOrphanedFiles
            | TaskType::CleanupPdfCache
            | TaskType::CleanupPluginData
            | TaskType::CleanupSeriesExports => 100,
        }
    }

    /// Extract task type string for database storage
    pub fn type_string(&self) -> &'static str {
        match self {
            TaskType::ScanLibrary { .. } => "scan_library",
            TaskType::AnalyzeBook { .. } => "analyze_book",
            TaskType::AnalyzeSeries { .. } => "analyze_series",
            TaskType::PurgeDeleted { .. } => "purge_deleted",
            TaskType::RefreshMetadata { .. } => "refresh_metadata",
            TaskType::RefreshLibraryMetadata { .. } => "refresh_library_metadata",
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
            TaskType::RenumberSeries { .. } => "renumber_series",
            TaskType::RenumberSeriesBatch { .. } => "renumber_series_batch",
            TaskType::CleanupPluginData => "cleanup_plugin_data",
            TaskType::CleanupSeriesExports => "cleanup_series_exports",
            TaskType::ExportSeries { .. } => "export_series",
            TaskType::UserPluginSync { .. } => "user_plugin_sync",
            TaskType::UserPluginRecommendations { .. } => "user_plugin_recommendations",
            TaskType::UserPluginRecommendationDismiss { .. } => {
                "user_plugin_recommendation_dismiss"
            }
            TaskType::BackfillTrackingFromMetadata { .. } => "backfill_tracking_from_metadata",
            TaskType::PollReleaseSource { .. } => "poll_release_source",
        }
    }

    /// Extract library_id if present.
    ///
    /// `RefreshLibraryMetadata` carries `job_id` rather than `library_id`; the
    /// library is resolved at run time from the job row. The library scope is
    /// reflected by `enqueue_filter_library_id` on enqueue; this helper
    /// returns `None` for that variant.
    pub fn library_id(&self) -> Option<Uuid> {
        match self {
            TaskType::ScanLibrary { library_id, .. } => Some(*library_id),
            TaskType::PurgeDeleted { library_id } => Some(*library_id),
            TaskType::GenerateThumbnails { library_id, .. } => *library_id,
            TaskType::GenerateSeriesThumbnails { library_id, .. } => *library_id,
            TaskType::ReprocessSeriesTitles { library_id, .. } => *library_id,
            TaskType::BackfillTrackingFromMetadata { library_id, .. } => *library_id,
            _ => None,
        }
    }

    /// Extract the library job ID for tasks scoped to a single
    /// [`library_jobs`] row, if any.
    pub fn job_id(&self) -> Option<Uuid> {
        match self {
            TaskType::RefreshLibraryMetadata { job_id } => Some(*job_id),
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
            TaskType::RefreshLibraryMetadata { job_id } => {
                // job_id is stored in params (no FK column on tasks).
                // The handler resolves the library from the job row at run time.
                serde_json::json!({ "job_id": job_id })
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
            TaskType::RenumberSeriesBatch { series_ids } => {
                serde_json::json!({ "series_ids": series_ids })
            }
            TaskType::UserPluginSync { plugin_id, user_id } => {
                serde_json::json!({ "plugin_id": plugin_id, "user_id": user_id })
            }
            TaskType::UserPluginRecommendations { plugin_id, user_id } => {
                serde_json::json!({ "plugin_id": plugin_id, "user_id": user_id })
            }
            TaskType::ExportSeries { export_id, user_id } => {
                serde_json::json!({ "export_id": export_id, "user_id": user_id })
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
            TaskType::BackfillTrackingFromMetadata { series_ids, .. } => {
                serde_json::json!({ "series_ids": series_ids })
            }
            TaskType::PollReleaseSource { source_id } => {
                serde_json::json!({ "source_id": source_id })
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
            TaskType::RenumberSeries { series_id } => Some(*series_id),
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

    /// JSON-param key/value pair to use as a dedup discriminator for task
    /// types whose identity lives in `params` rather than in FK columns.
    ///
    /// Returning `Some((key, value))` tells the dedup path in
    /// `TaskRepository::find_existing_task` to additionally filter by
    /// `params->>key = value`. Without this, two `poll_release_source` tasks
    /// for *different* `source_id`s would falsely collide because they share
    /// the same `task_type` and have no FK columns set, causing the second
    /// "Poll now" click to be silently coalesced onto the first source's
    /// in-flight poll.
    ///
    /// `key` must be a simple identifier (alphanumeric + underscore) since
    /// SQLite splices it into a JSON path string.
    pub fn dedup_params(&self) -> Option<(&'static str, String)> {
        match self {
            TaskType::PollReleaseSource { source_id } => Some(("source_id", source_id.to_string())),
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

    #[test]
    fn test_renumber_series_extraction() {
        let series_id = Uuid::new_v4();

        let task = TaskType::RenumberSeries { series_id };

        assert_eq!(task.type_string(), "renumber_series");
        assert_eq!(task.library_id(), None);
        assert_eq!(task.series_id(), Some(series_id));
        assert_eq!(task.book_id(), None);

        let (type_str, lib_id, extracted_series_id, book_id, params) = task.extract_fields();
        assert_eq!(type_str, "renumber_series");
        assert_eq!(lib_id, None);
        assert_eq!(extracted_series_id, Some(series_id));
        assert_eq!(book_id, None);
        // RenumberSeries has no special params, so params should be None (empty object)
        assert!(params.is_none());
    }

    #[test]
    fn test_renumber_series_serialization() {
        let series_id = Uuid::new_v4();

        let task = TaskType::RenumberSeries { series_id };
        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("renumber_series"));
        assert!(json.contains(&series_id.to_string()));

        let deserialized: TaskType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.type_string(), "renumber_series");
        assert_eq!(deserialized.series_id(), Some(series_id));
    }

    #[test]
    fn test_renumber_series_batch_extraction() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let task = TaskType::RenumberSeriesBatch {
            series_ids: Some(vec![id1, id2]),
        };

        assert_eq!(task.type_string(), "renumber_series_batch");
        assert_eq!(task.library_id(), None);
        assert_eq!(task.series_id(), None);
        assert_eq!(task.book_id(), None);

        let (type_str, lib_id, series_id, book_id, params) = task.extract_fields();
        assert_eq!(type_str, "renumber_series_batch");
        assert_eq!(lib_id, None);
        assert_eq!(series_id, None);
        assert_eq!(book_id, None);
        assert!(params.is_some());
        let params = params.unwrap();
        let ids = params["series_ids"].as_array().unwrap();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_renumber_series_batch_empty() {
        // Batch with None series_ids
        let task = TaskType::RenumberSeriesBatch { series_ids: None };

        assert_eq!(task.type_string(), "renumber_series_batch");
        let params = task.params();
        assert!(params["series_ids"].is_null());
    }

    #[test]
    fn test_refresh_library_metadata_extraction() {
        let job_id = Uuid::new_v4();
        let task = TaskType::RefreshLibraryMetadata { job_id };

        assert_eq!(task.type_string(), "refresh_library_metadata");
        // RefreshLibraryMetadata is scoped by job_id; library is resolved at runtime.
        assert_eq!(task.library_id(), None);
        assert_eq!(task.job_id(), Some(job_id));
        assert_eq!(task.series_id(), None);
        assert_eq!(task.book_id(), None);
        assert_eq!(task.default_priority(), 385);

        let (type_str, lib_id, series_id, book_id, params) = task.extract_fields();
        assert_eq!(type_str, "refresh_library_metadata");
        assert!(lib_id.is_none());
        assert!(series_id.is_none());
        assert!(book_id.is_none());
        // job_id is part of the params payload (no dedicated FK column on tasks)
        let params = params.expect("expected job_id params");
        assert_eq!(params["job_id"], serde_json::json!(job_id));
    }

    #[test]
    fn test_refresh_library_metadata_serialization() {
        let job_id = Uuid::new_v4();
        let task = TaskType::RefreshLibraryMetadata { job_id };

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("refresh_library_metadata"));
        assert!(json.contains(&job_id.to_string()));
        // jobId is the camelCase rename for the new variant.
        assert!(json.contains("jobId"));

        let deserialized: TaskType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.type_string(), "refresh_library_metadata");
        assert_eq!(deserialized.job_id(), Some(job_id));
    }

    #[test]
    fn test_poll_release_source_extraction() {
        let source_id = Uuid::new_v4();
        let task = TaskType::PollReleaseSource { source_id };

        assert_eq!(task.type_string(), "poll_release_source");
        assert_eq!(task.library_id(), None);
        assert_eq!(task.series_id(), None);
        assert_eq!(task.book_id(), None);
        assert_eq!(task.default_priority(), 170);

        let (type_str, lib_id, series_id, book_id, params) = task.extract_fields();
        assert_eq!(type_str, "poll_release_source");
        assert_eq!(lib_id, None);
        assert_eq!(series_id, None);
        assert_eq!(book_id, None);
        let params = params.expect("expected source_id params");
        assert_eq!(params["source_id"], serde_json::json!(source_id));
    }

    #[test]
    fn test_poll_release_source_serialization() {
        let source_id = Uuid::new_v4();
        let task = TaskType::PollReleaseSource { source_id };

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("poll_release_source"));
        assert!(json.contains(&source_id.to_string()));
        // sourceId is the camelCase rename.
        assert!(json.contains("sourceId"));

        let deserialized: TaskType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.type_string(), "poll_release_source");
        match deserialized {
            TaskType::PollReleaseSource { source_id: id } => {
                assert_eq!(id, source_id);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_default_priority_values() {
        let library_id = Uuid::new_v4();
        let series_id = Uuid::new_v4();
        let book_id = Uuid::new_v4();
        let plugin_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // Scanning: highest priority
        assert_eq!(
            TaskType::ScanLibrary {
                library_id,
                mode: "normal".to_string()
            }
            .default_priority(),
            1000
        );
        assert_eq!(
            TaskType::PurgeDeleted { library_id }.default_priority(),
            900
        );

        // Analysis
        assert_eq!(
            TaskType::AnalyzeBook {
                book_id,
                force: false
            }
            .default_priority(),
            800
        );
        assert_eq!(
            TaskType::AnalyzeSeries { series_id }.default_priority(),
            790
        );
        assert_eq!(
            TaskType::ReprocessSeriesTitle { series_id }.default_priority(),
            780
        );
        assert_eq!(
            TaskType::ReprocessSeriesTitles {
                library_id: Some(library_id),
                series_ids: None
            }
            .default_priority(),
            770
        );
        assert_eq!(
            TaskType::RenumberSeries { series_id }.default_priority(),
            760
        );
        assert_eq!(
            TaskType::RenumberSeriesBatch {
                series_ids: Some(vec![series_id])
            }
            .default_priority(),
            750
        );

        // Thumbnails
        assert_eq!(
            TaskType::GenerateThumbnail {
                book_id,
                force: false
            }
            .default_priority(),
            600
        );
        assert_eq!(
            TaskType::GenerateSeriesThumbnail {
                series_id,
                force: false
            }
            .default_priority(),
            590
        );
        assert_eq!(
            TaskType::GenerateThumbnails {
                library_id: Some(library_id),
                series_id: None,
                series_ids: None,
                book_ids: None,
                force: false
            }
            .default_priority(),
            580
        );
        assert_eq!(
            TaskType::GenerateSeriesThumbnails {
                library_id: Some(library_id),
                series_ids: None,
                force: false
            }
            .default_priority(),
            570
        );

        // Metadata
        assert_eq!(TaskType::FindDuplicates.default_priority(), 400);
        assert_eq!(
            TaskType::RefreshMetadata {
                book_id,
                source: "test".to_string()
            }
            .default_priority(),
            390
        );
        assert_eq!(
            TaskType::PluginAutoMatch {
                series_id,
                plugin_id,
                source_scope: None
            }
            .default_priority(),
            380
        );

        // Plugins
        assert_eq!(
            TaskType::UserPluginRecommendationDismiss {
                plugin_id,
                user_id,
                external_id: "test".to_string(),
                reason: None
            }
            .default_priority(),
            200
        );
        assert_eq!(
            TaskType::UserPluginSync { plugin_id, user_id }.default_priority(),
            190
        );
        assert_eq!(
            TaskType::UserPluginRecommendations { plugin_id, user_id }.default_priority(),
            180
        );

        // Cleanup: lowest priority
        assert_eq!(
            TaskType::CleanupBookFiles {
                book_id,
                thumbnail_path: None,
                series_id: None
            }
            .default_priority(),
            100
        );
        assert_eq!(
            TaskType::CleanupSeriesFiles { series_id }.default_priority(),
            100
        );
        assert_eq!(TaskType::CleanupOrphanedFiles.default_priority(), 100);
        assert_eq!(TaskType::CleanupPdfCache.default_priority(), 100);
        assert_eq!(TaskType::CleanupPluginData.default_priority(), 100);
    }

    #[test]
    fn test_default_priority_ordering_invariants() {
        let library_id = Uuid::new_v4();
        let _series_id = Uuid::new_v4();
        let book_id = Uuid::new_v4();

        // Scanning > Analysis > Thumbnails > Metadata > Plugins > Cleanup
        let scan = TaskType::ScanLibrary {
            library_id,
            mode: "normal".to_string(),
        }
        .default_priority();
        let analyze = TaskType::AnalyzeBook {
            book_id,
            force: false,
        }
        .default_priority();
        let thumbnail = TaskType::GenerateThumbnail {
            book_id,
            force: false,
        }
        .default_priority();
        let metadata = TaskType::FindDuplicates.default_priority();
        let plugin = TaskType::UserPluginSync {
            plugin_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
        }
        .default_priority();
        let cleanup = TaskType::CleanupOrphanedFiles.default_priority();

        assert!(
            scan > analyze,
            "Scanning should have higher priority than analysis"
        );
        assert!(
            analyze > thumbnail,
            "Analysis should have higher priority than thumbnails"
        );
        assert!(
            thumbnail > metadata,
            "Thumbnails should have higher priority than metadata"
        );
        assert!(
            metadata > plugin,
            "Metadata should have higher priority than plugins"
        );
        assert!(
            plugin > cleanup,
            "Plugins should have higher priority than cleanup"
        );
    }

    #[test]
    fn test_renumber_series_batch_serialization() {
        let id1 = Uuid::new_v4();

        let task = TaskType::RenumberSeriesBatch {
            series_ids: Some(vec![id1]),
        };

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("renumber_series_batch"));
        assert!(json.contains(&id1.to_string()));

        let deserialized: TaskType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.type_string(), "renumber_series_batch");
    }
}
