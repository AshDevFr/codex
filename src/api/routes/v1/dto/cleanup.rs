//! DTOs for file cleanup endpoints

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Statistics about orphaned files in the system
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrphanStatsDto {
    /// Number of orphaned thumbnail files (no matching book in database)
    #[schema(example = 42)]
    pub orphaned_thumbnails: u32,

    /// Number of orphaned cover files (no matching series in database)
    #[schema(example = 5)]
    pub orphaned_covers: u32,

    /// Total size of all orphaned files in bytes
    #[schema(example = 1073741824)]
    pub total_size_bytes: u64,

    /// List of orphaned files with details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<OrphanedFileDto>>,
}

/// Information about a single orphaned file
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrphanedFileDto {
    /// Path to the orphaned file (relative to data directory)
    #[schema(example = "thumbnails/books/55/550e8400-e29b-41d4-a716-446655440000.jpg")]
    pub path: String,

    /// The entity UUID extracted from the filename
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub entity_id: Option<Uuid>,

    /// Size of the file in bytes
    #[schema(example = 25600)]
    pub size_bytes: u64,

    /// Type of file: "thumbnail" or "cover"
    #[schema(example = "thumbnail")]
    pub file_type: String,
}

/// Result of a cleanup operation
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CleanupResultDto {
    /// Number of thumbnail files deleted
    #[schema(example = 42)]
    pub thumbnails_deleted: u32,

    /// Number of cover files deleted
    #[schema(example = 5)]
    pub covers_deleted: u32,

    /// Total bytes freed by deletion
    #[schema(example = 1073741824)]
    pub bytes_freed: u64,

    /// Number of files that failed to delete
    #[schema(example = 0)]
    pub failures: u32,

    /// Error messages for any failed deletions
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

impl From<crate::services::file_cleanup::CleanupStats> for CleanupResultDto {
    fn from(stats: crate::services::file_cleanup::CleanupStats) -> Self {
        Self {
            thumbnails_deleted: stats.thumbnails_deleted,
            covers_deleted: stats.covers_deleted,
            bytes_freed: stats.bytes_freed,
            failures: stats.failures,
            errors: stats.errors,
        }
    }
}

/// Response when triggering a cleanup task
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TriggerCleanupResponse {
    /// ID of the queued cleanup task
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub task_id: Uuid,

    /// Message describing the action taken
    #[schema(example = "Cleanup task queued successfully")]
    pub message: String,
}

/// Query parameters for orphan stats endpoint
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct OrphanStatsQuery {
    /// If true, include the full list of orphaned files in the response
    #[serde(default)]
    pub include_files: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orphan_stats_dto_serialization() {
        let stats = OrphanStatsDto {
            orphaned_thumbnails: 42,
            orphaned_covers: 5,
            total_size_bytes: 1024 * 1024,
            files: None,
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"orphaned_thumbnails\":42"));
        assert!(json.contains("\"orphaned_covers\":5"));
        // files field should be skipped when None
        assert!(!json.contains("\"files\""));
    }

    #[test]
    fn test_orphan_stats_dto_with_files() {
        let file = OrphanedFileDto {
            path: "thumbnails/books/55/550e8400.jpg".to_string(),
            entity_id: Some(uuid::Uuid::nil()),
            size_bytes: 1024,
            file_type: "thumbnail".to_string(),
        };

        let stats = OrphanStatsDto {
            orphaned_thumbnails: 1,
            orphaned_covers: 0,
            total_size_bytes: 1024,
            files: Some(vec![file]),
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"files\""));
        assert!(json.contains("\"file_type\":\"thumbnail\""));
    }

    #[test]
    fn test_cleanup_result_dto_from_stats() {
        let stats = crate::services::file_cleanup::CleanupStats {
            thumbnails_deleted: 10,
            covers_deleted: 2,
            bytes_freed: 500_000,
            failures: 1,
            errors: vec!["Test error".to_string()],
        };

        let dto: CleanupResultDto = stats.into();
        assert_eq!(dto.thumbnails_deleted, 10);
        assert_eq!(dto.covers_deleted, 2);
        assert_eq!(dto.bytes_freed, 500_000);
        assert_eq!(dto.failures, 1);
        assert_eq!(dto.errors.len(), 1);
    }

    #[test]
    fn test_cleanup_result_dto_empty_errors_skipped() {
        let dto = CleanupResultDto {
            thumbnails_deleted: 5,
            covers_deleted: 0,
            bytes_freed: 1000,
            failures: 0,
            errors: vec![],
        };

        let json = serde_json::to_string(&dto).unwrap();
        // errors field should be skipped when empty
        assert!(!json.contains("\"errors\""));
    }

    #[test]
    fn test_trigger_cleanup_response_serialization() {
        let response = TriggerCleanupResponse {
            task_id: uuid::Uuid::nil(),
            message: "Task queued".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"task_id\""));
        assert!(json.contains("\"message\":\"Task queued\""));
    }
}
