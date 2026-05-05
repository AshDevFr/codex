//! Event types for entity change notifications and task progress
//!
//! TODO: Remove allow(dead_code) once event features are fully integrated

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Type of entity that was changed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    Book,
    Series,
    Library,
}

/// Task status for progress tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    /// Task is pending and waiting to be processed
    Pending,
    /// Task is currently being processed
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed with an error
    Failed,
}

/// Progress information for a running task
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskProgress {
    /// Current progress value
    #[schema(example = 5)]
    pub current: usize,
    /// Total work to be done
    #[schema(example = 10)]
    pub total: usize,
    /// Optional progress message
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Processing book 5 of 10")]
    pub message: Option<String>,
}

/// Specific event types for entity changes
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EntityEvent {
    /// A book was created
    BookCreated {
        #[serde(rename = "bookId")]
        book_id: Uuid,
        #[serde(rename = "seriesId")]
        series_id: Uuid,
        #[serde(rename = "libraryId")]
        library_id: Uuid,
    },
    /// A book was updated
    BookUpdated {
        #[serde(rename = "bookId")]
        book_id: Uuid,
        #[serde(rename = "seriesId")]
        series_id: Uuid,
        #[serde(rename = "libraryId")]
        library_id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        fields: Option<Vec<String>>,
    },
    /// A book was deleted
    BookDeleted {
        #[serde(rename = "bookId")]
        book_id: Uuid,
        #[serde(rename = "seriesId")]
        series_id: Uuid,
        #[serde(rename = "libraryId")]
        library_id: Uuid,
    },
    /// A series was created
    SeriesCreated {
        #[serde(rename = "seriesId")]
        series_id: Uuid,
        #[serde(rename = "libraryId")]
        library_id: Uuid,
    },
    /// A series was updated
    SeriesUpdated {
        #[serde(rename = "seriesId")]
        series_id: Uuid,
        #[serde(rename = "libraryId")]
        library_id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        fields: Option<Vec<String>>,
    },
    /// A series was deleted
    SeriesDeleted {
        #[serde(rename = "seriesId")]
        series_id: Uuid,
        #[serde(rename = "libraryId")]
        library_id: Uuid,
    },
    /// Series metadata was updated by a plugin
    SeriesMetadataUpdated {
        #[serde(rename = "seriesId")]
        series_id: Uuid,
        #[serde(rename = "libraryId")]
        library_id: Uuid,
        /// Plugin that updated the metadata
        #[serde(rename = "pluginId")]
        plugin_id: Uuid,
        /// Fields that were updated
        #[serde(rename = "fieldsUpdated")]
        fields_updated: Vec<String>,
    },
    /// Deleted books were purged from a series
    SeriesBulkPurged {
        #[serde(rename = "seriesId")]
        series_id: Uuid,
        #[serde(rename = "libraryId")]
        library_id: Uuid,
        count: u64,
    },
    /// A cover image was updated
    CoverUpdated {
        #[serde(rename = "entityType")]
        entity_type: EntityType,
        #[serde(rename = "entityId")]
        entity_id: Uuid,
        #[serde(rename = "libraryId", skip_serializing_if = "Option::is_none")]
        library_id: Option<Uuid>,
    },
    /// A library was updated
    LibraryUpdated {
        #[serde(rename = "libraryId")]
        library_id: Uuid,
    },
    /// A library was deleted
    LibraryDeleted {
        #[serde(rename = "libraryId")]
        library_id: Uuid,
    },
    /// A plugin was created
    PluginCreated {
        #[serde(rename = "pluginId")]
        plugin_id: Uuid,
    },
    /// A plugin was updated (configuration changed)
    PluginUpdated {
        #[serde(rename = "pluginId")]
        plugin_id: Uuid,
    },
    /// A plugin was enabled
    PluginEnabled {
        #[serde(rename = "pluginId")]
        plugin_id: Uuid,
    },
    /// A plugin was disabled
    PluginDisabled {
        #[serde(rename = "pluginId")]
        plugin_id: Uuid,
    },
    /// A plugin was deleted
    PluginDeleted {
        #[serde(rename = "pluginId")]
        plugin_id: Uuid,
    },
    /// A new release was recorded in the ledger.
    ///
    /// Emitted once per accepted, non-deduped ledger insert by the polling
    /// task and the `releases/record` reverse-RPC handler. The frontend uses
    /// this to bump the Releases nav badge, surface a toast on the inbox
    /// page, and refresh the per-series Releases tab.
    ReleaseAnnounced {
        #[serde(rename = "ledgerId")]
        ledger_id: Uuid,
        #[serde(rename = "seriesId")]
        series_id: Uuid,
        #[serde(rename = "sourceId")]
        source_id: Uuid,
        /// Plugin name that owns the source (`release_sources.plugin_id`).
        /// Helps the frontend filter without an extra lookup.
        #[serde(rename = "pluginId")]
        plugin_id: String,
        /// Chapter announced (if the source emits chapters).
        #[serde(skip_serializing_if = "Option::is_none")]
        chapter: Option<f64>,
        /// Volume announced (if the source emits volumes).
        #[serde(skip_serializing_if = "Option::is_none")]
        volume: Option<i32>,
        /// Language code (e.g. `"en"`); used by client-side notification
        /// preference filters.
        language: String,
    },
    /// A release source's poll task completed.
    ///
    /// Emitted at the end of every `poll_release_source` task run, after
    /// `release_sources.last_summary` / `last_polled_at` / `etag` have been
    /// persisted. The frontend uses this to refresh the Release tracking
    /// settings page in real time so users don't have to reload to see a
    /// "Poll now" finish. Carries no diff details — receivers should
    /// invalidate the source query and re-read the row.
    ReleaseSourcePolled {
        #[serde(rename = "sourceId")]
        source_id: Uuid,
        /// Plugin that owns the source (`release_sources.plugin_id`).
        /// Cheap filter for clients only watching certain plugins.
        #[serde(rename = "pluginId")]
        plugin_id: String,
        /// `true` if the poll wrote a `last_error`. Cheap "did it fail"
        /// hint without forcing the client to refetch.
        #[serde(rename = "hadError")]
        had_error: bool,
    },
    /// Internal signal to indicate shutdown (not sent to clients)
    #[serde(skip)]
    Shutdown,
}

/// Complete entity change event with metadata
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntityChangeEvent {
    /// The specific event that occurred
    #[serde(flatten)]
    pub event: EntityEvent,
    /// When the event occurred
    pub timestamp: DateTime<Utc>,
    /// User who triggered the change (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<Uuid>,
}

impl EntityChangeEvent {
    /// Create a new entity change event
    pub fn new(event: EntityEvent, user_id: Option<Uuid>) -> Self {
        Self {
            event,
            timestamp: Utc::now(),
            user_id,
        }
    }

    /// Get the library ID associated with this event (if any)
    pub fn library_id(&self) -> Option<Uuid> {
        match &self.event {
            EntityEvent::BookCreated { library_id, .. }
            | EntityEvent::BookUpdated { library_id, .. }
            | EntityEvent::BookDeleted { library_id, .. }
            | EntityEvent::SeriesCreated { library_id, .. }
            | EntityEvent::SeriesUpdated { library_id, .. }
            | EntityEvent::SeriesDeleted { library_id, .. }
            | EntityEvent::SeriesMetadataUpdated { library_id, .. }
            | EntityEvent::SeriesBulkPurged { library_id, .. }
            | EntityEvent::LibraryUpdated { library_id }
            | EntityEvent::LibraryDeleted { library_id } => Some(*library_id),
            EntityEvent::CoverUpdated { library_id, .. } => *library_id,
            EntityEvent::PluginCreated { .. }
            | EntityEvent::PluginUpdated { .. }
            | EntityEvent::PluginEnabled { .. }
            | EntityEvent::PluginDisabled { .. }
            | EntityEvent::PluginDeleted { .. }
            | EntityEvent::ReleaseAnnounced { .. }
            | EntityEvent::ReleaseSourcePolled { .. }
            | EntityEvent::Shutdown => None,
        }
    }

    /// Create a shutdown signal event (internal use only)
    pub fn shutdown_signal() -> Self {
        Self {
            event: EntityEvent::Shutdown,
            timestamp: Utc::now(),
            user_id: None,
        }
    }

    /// Check if this is a shutdown signal
    pub fn is_shutdown(&self) -> bool {
        matches!(self.event, EntityEvent::Shutdown)
    }

    /// Build a `ReleaseAnnounced` event from a freshly-inserted ledger row.
    ///
    /// Wraps the variant construction so callers in the polling task and the
    /// reverse-RPC handler share one source of truth for the event shape.
    pub fn release_announced(
        row: &crate::db::entities::release_ledger::Model,
        plugin_id: &str,
    ) -> Self {
        Self::new(
            EntityEvent::ReleaseAnnounced {
                ledger_id: row.id,
                series_id: row.series_id,
                source_id: row.source_id,
                plugin_id: plugin_id.to_string(),
                chapter: row.chapter,
                volume: row.volume,
                language: row.language.clone().unwrap_or_default(),
            },
            None,
        )
    }

    /// Build a `ReleaseSourcePolled` event for the end of a poll task run.
    ///
    /// Carries only IDs and a single boolean error hint; receivers should
    /// invalidate any cached `release_sources` query and re-read the row
    /// for fresh `last_summary` / `last_polled_at` / etc.
    pub fn release_source_polled(source_id: Uuid, plugin_id: &str, had_error: bool) -> Self {
        Self::new(
            EntityEvent::ReleaseSourcePolled {
                source_id,
                plugin_id: plugin_id.to_string(),
                had_error,
            },
            None,
        )
    }
}

/// Task progress event for background operations
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgressEvent {
    /// Unique identifier for this task instance
    pub task_id: Uuid,
    /// Type of task being executed
    pub task_type: String,
    /// Current status of the task
    pub status: TaskStatus,
    /// Progress information (for running tasks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<TaskProgress>,
    /// Error message (for failed tasks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// When the task started
    pub started_at: DateTime<Utc>,
    /// When the task completed (success or failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    /// Library ID if this task is related to a library
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_id: Option<Uuid>,
    /// Series ID if this task is related to a series
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_id: Option<Uuid>,
    /// Book ID if this task is related to a book
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_id: Option<Uuid>,
}

impl TaskProgressEvent {
    /// Create a new task started event
    pub fn started(
        task_id: Uuid,
        task_type: impl Into<String>,
        library_id: Option<Uuid>,
        series_id: Option<Uuid>,
        book_id: Option<Uuid>,
    ) -> Self {
        Self {
            task_id,
            task_type: task_type.into(),
            status: TaskStatus::Running,
            progress: None,
            error: None,
            started_at: Utc::now(),
            completed_at: None,
            library_id,
            series_id,
            book_id,
        }
    }

    /// Create a task progress update event
    #[allow(clippy::too_many_arguments)] // All fields are needed to construct a complete progress event
    pub fn progress(
        task_id: Uuid,
        task_type: impl Into<String>,
        current: usize,
        total: usize,
        message: Option<String>,
        library_id: Option<Uuid>,
        series_id: Option<Uuid>,
        book_id: Option<Uuid>,
    ) -> Self {
        Self {
            task_id,
            task_type: task_type.into(),
            status: TaskStatus::Running,
            progress: Some(TaskProgress {
                current,
                total,
                message,
            }),
            error: None,
            started_at: Utc::now(),
            completed_at: None,
            library_id,
            series_id,
            book_id,
        }
    }

    /// Create a task completed event
    pub fn completed(
        task_id: Uuid,
        task_type: impl Into<String>,
        started_at: DateTime<Utc>,
        library_id: Option<Uuid>,
        series_id: Option<Uuid>,
        book_id: Option<Uuid>,
    ) -> Self {
        Self {
            task_id,
            task_type: task_type.into(),
            status: TaskStatus::Completed,
            progress: None,
            error: None,
            started_at,
            completed_at: Some(Utc::now()),
            library_id,
            series_id,
            book_id,
        }
    }

    /// Create a task failed event
    pub fn failed(
        task_id: Uuid,
        task_type: impl Into<String>,
        error: impl Into<String>,
        started_at: DateTime<Utc>,
        library_id: Option<Uuid>,
        series_id: Option<Uuid>,
        book_id: Option<Uuid>,
    ) -> Self {
        Self {
            task_id,
            task_type: task_type.into(),
            status: TaskStatus::Failed,
            progress: None,
            error: Some(error.into()),
            started_at,
            completed_at: Some(Utc::now()),
            library_id,
            series_id,
            book_id,
        }
    }

    /// Create a shutdown signal event (internal use only)
    pub fn shutdown_signal() -> Self {
        Self {
            task_id: Uuid::nil(),
            task_type: "__shutdown__".to_string(),
            status: TaskStatus::Completed,
            progress: None,
            error: None,
            started_at: Utc::now(),
            completed_at: None,
            library_id: None,
            series_id: None,
            book_id: None,
        }
    }

    /// Check if this is a shutdown signal
    pub fn is_shutdown(&self) -> bool {
        self.task_type == "__shutdown__"
    }
}
