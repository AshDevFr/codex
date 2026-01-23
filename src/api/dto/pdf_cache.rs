//! DTOs for PDF cache management endpoints

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Statistics about the PDF page cache
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PdfCacheStatsDto {
    /// Total number of cached page files
    #[schema(example = 1500)]
    pub total_files: u64,

    /// Total size of cache in bytes
    #[schema(example = 157286400)]
    pub total_size_bytes: u64,

    /// Human-readable total size (e.g., "150.0 MB")
    #[schema(example = "150.0 MB")]
    pub total_size_human: String,

    /// Number of unique books with cached pages
    #[schema(example = 45)]
    pub book_count: u64,

    /// Age of the oldest cached file in days (if any files exist)
    #[schema(example = 15)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest_file_age_days: Option<u32>,

    /// Path to the cache directory
    #[schema(example = "/data/cache")]
    pub cache_dir: String,

    /// Whether the PDF page cache is enabled
    #[schema(example = true)]
    pub cache_enabled: bool,
}

impl From<crate::services::CacheStats> for PdfCacheStatsDto {
    fn from(stats: crate::services::CacheStats) -> Self {
        Self {
            total_files: stats.total_files,
            total_size_bytes: stats.total_size_bytes,
            total_size_human: stats.total_size_human(),
            book_count: stats.book_count,
            oldest_file_age_days: stats.oldest_file_age_days,
            cache_dir: stats.cache_dir,
            cache_enabled: true,
        }
    }
}

/// Result of a PDF cache cleanup operation
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PdfCacheCleanupResultDto {
    /// Number of cached page files deleted
    #[schema(example = 250)]
    pub files_deleted: u64,

    /// Bytes freed by the cleanup
    #[schema(example = 26214400)]
    pub bytes_reclaimed: u64,

    /// Human-readable size reclaimed (e.g., "25.0 MB")
    #[schema(example = "25.0 MB")]
    pub bytes_reclaimed_human: String,
}

impl From<crate::services::CleanupResult> for PdfCacheCleanupResultDto {
    fn from(result: crate::services::CleanupResult) -> Self {
        Self {
            files_deleted: result.files_deleted,
            bytes_reclaimed: result.bytes_reclaimed,
            bytes_reclaimed_human: result.bytes_reclaimed_human(),
        }
    }
}

/// Response when triggering a PDF cache cleanup task
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TriggerPdfCacheCleanupResponse {
    /// ID of the queued cleanup task
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub task_id: Uuid,

    /// Message describing the action taken
    #[schema(example = "PDF cache cleanup task queued successfully")]
    pub message: String,

    /// Max age setting being used for cleanup (in days)
    #[schema(example = 30)]
    pub max_age_days: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_dto_serialization() {
        let stats = PdfCacheStatsDto {
            total_files: 100,
            total_size_bytes: 1024 * 1024 * 50,
            total_size_human: "50.0 MB".to_string(),
            book_count: 10,
            oldest_file_age_days: Some(5),
            cache_dir: "/data/cache".to_string(),
            cache_enabled: true,
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"total_files\":100"));
        assert!(json.contains("\"book_count\":10"));
        assert!(json.contains("\"cache_enabled\":true"));
    }

    #[test]
    fn test_stats_dto_skips_none_oldest() {
        let stats = PdfCacheStatsDto {
            total_files: 0,
            total_size_bytes: 0,
            total_size_human: "0 B".to_string(),
            book_count: 0,
            oldest_file_age_days: None,
            cache_dir: "/data/cache".to_string(),
            cache_enabled: true,
        };

        let json = serde_json::to_string(&stats).unwrap();
        // oldest_file_age_days should be skipped when None
        assert!(!json.contains("\"oldest_file_age_days\""));
    }

    #[test]
    fn test_cleanup_result_dto_serialization() {
        let result = PdfCacheCleanupResultDto {
            files_deleted: 50,
            bytes_reclaimed: 1024 * 1024 * 25,
            bytes_reclaimed_human: "25.0 MB".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"files_deleted\":50"));
        assert!(json.contains("\"bytes_reclaimed_human\":\"25.0 MB\""));
    }

    #[test]
    fn test_trigger_response_serialization() {
        let response = TriggerPdfCacheCleanupResponse {
            task_id: uuid::Uuid::nil(),
            message: "Task queued".to_string(),
            max_age_days: 30,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"task_id\""));
        assert!(json.contains("\"max_age_days\":30"));
    }
}
