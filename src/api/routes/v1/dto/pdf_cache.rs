//! DTOs for PDF cache management endpoints

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Statistics about the on-disk rendered-page cache.
///
/// This is the cache that stores already-rendered JPEG images of PDF pages.
/// Backed by `PdfPageCache` in the service layer.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PdfPageCacheStatsDto {
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

impl From<crate::services::CacheStats> for PdfPageCacheStatsDto {
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

/// Per-entry view of the in-memory PDFium handle cache.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PdfHandleCacheEntryDto {
    /// Book ID for the cached document.
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub book_id: Uuid,

    /// File path of the opened PDF.
    #[schema(example = "/library/books/manual.pdf")]
    pub path: String,

    /// Seconds since the handle was opened.
    #[schema(example = 312)]
    pub age_seconds: u64,

    /// Seconds since the last render against this handle.
    #[schema(example = 14)]
    pub idle_seconds: u64,

    /// Number of renders served from this handle.
    #[schema(example = 27)]
    pub render_count: u64,
}

impl From<crate::services::HandleCacheEntrySnapshot> for PdfHandleCacheEntryDto {
    fn from(entry: crate::services::HandleCacheEntrySnapshot) -> Self {
        Self {
            book_id: entry.book_id,
            path: entry.path,
            age_seconds: entry.age_seconds,
            idle_seconds: entry.idle_seconds,
            render_count: entry.render_count,
        }
    }
}

/// Statistics about the in-memory open-document handle cache.
///
/// Backed by `PdfHandleCache` in the service layer. Avoids re-opening the
/// underlying PDF file via PDFium on every page request.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PdfHandleCacheStatsDto {
    /// Whether the handle cache is enabled.
    #[schema(example = true)]
    pub enabled: bool,

    /// Maximum number of handles the cache will retain.
    #[schema(example = 256)]
    pub capacity: u64,

    /// Idle TTL in seconds before a handle is evicted by the background sweeper.
    #[schema(example = 900)]
    pub idle_ttl_seconds: u64,

    /// Number of handles currently cached.
    #[schema(example = 12)]
    pub current_size: u64,

    /// Cumulative cache hits (handle reused without re-opening).
    #[schema(example = 4321)]
    pub hits: u64,

    /// Cumulative cache misses (no entry on lookup).
    #[schema(example = 87)]
    pub misses: u64,

    /// Cumulative PDFium opens performed by the cache.
    #[schema(example = 87)]
    pub opens: u64,

    /// Cumulative evictions (capacity + manual).
    #[schema(example = 5)]
    pub evictions: u64,

    /// Cumulative idle-TTL evictions performed by the sweeper.
    #[schema(example = 3)]
    pub idle_evictions: u64,

    /// Per-entry detail for the admin UI.
    pub entries: Vec<PdfHandleCacheEntryDto>,
}

impl From<crate::services::HandleCacheSnapshot> for PdfHandleCacheStatsDto {
    fn from(snap: crate::services::HandleCacheSnapshot) -> Self {
        Self {
            enabled: snap.enabled,
            capacity: snap.capacity as u64,
            idle_ttl_seconds: snap.idle_ttl_seconds,
            current_size: snap.current_size as u64,
            hits: snap.hits,
            misses: snap.misses,
            opens: snap.opens,
            evictions: snap.evictions,
            idle_evictions: snap.idle_evictions,
            entries: snap.entries.into_iter().map(Into::into).collect(),
        }
    }
}

/// Combined PDF cache statistics.
///
/// Exposes both the on-disk rendered-page cache and the in-memory open-document
/// handle cache in a single payload.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PdfCacheStatsDto {
    /// Disk-backed rendered-page cache (JPEGs).
    pub pages: PdfPageCacheStatsDto,
    /// In-memory open-document handle cache (PDFium).
    pub handles: PdfHandleCacheStatsDto,
}

/// Result of a PDF cache cleanup operation
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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

/// Response when clearing the handle cache (close-all or single).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PdfHandleCacheClearResultDto {
    /// Number of handles closed by the operation.
    #[schema(example = 12)]
    pub handles_closed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_page_stats() -> PdfPageCacheStatsDto {
        PdfPageCacheStatsDto {
            total_files: 100,
            total_size_bytes: 1024 * 1024 * 50,
            total_size_human: "50.0 MB".to_string(),
            book_count: 10,
            oldest_file_age_days: Some(5),
            cache_dir: "/data/cache".to_string(),
            cache_enabled: true,
        }
    }

    fn sample_handle_stats() -> PdfHandleCacheStatsDto {
        PdfHandleCacheStatsDto {
            enabled: true,
            capacity: 256,
            idle_ttl_seconds: 900,
            current_size: 2,
            hits: 12,
            misses: 3,
            opens: 3,
            evictions: 0,
            idle_evictions: 0,
            entries: vec![],
        }
    }

    #[test]
    fn page_stats_dto_serialization() {
        let stats = sample_page_stats();
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"totalFiles\":100"));
        assert!(json.contains("\"bookCount\":10"));
        assert!(json.contains("\"cacheEnabled\":true"));
    }

    #[test]
    fn page_stats_dto_skips_none_oldest() {
        let stats = PdfPageCacheStatsDto {
            total_files: 0,
            total_size_bytes: 0,
            total_size_human: "0 B".to_string(),
            book_count: 0,
            oldest_file_age_days: None,
            cache_dir: "/data/cache".to_string(),
            cache_enabled: true,
        };
        let json = serde_json::to_string(&stats).unwrap();
        assert!(!json.contains("\"oldestFileAgeDays\""));
    }

    #[test]
    fn handle_stats_dto_serialization() {
        let stats = sample_handle_stats();
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"capacity\":256"));
        assert!(json.contains("\"idleTtlSeconds\":900"));
        assert!(json.contains("\"currentSize\":2"));
        assert!(json.contains("\"enabled\":true"));
    }

    #[test]
    fn combined_stats_dto_serialization() {
        let combined = PdfCacheStatsDto {
            pages: sample_page_stats(),
            handles: sample_handle_stats(),
        };
        let json = serde_json::to_string(&combined).unwrap();
        assert!(json.contains("\"pages\":{"));
        assert!(json.contains("\"handles\":{"));
        assert!(json.contains("\"totalFiles\":100"));
        assert!(json.contains("\"capacity\":256"));
    }

    #[test]
    fn cleanup_result_dto_serialization() {
        let result = PdfCacheCleanupResultDto {
            files_deleted: 50,
            bytes_reclaimed: 1024 * 1024 * 25,
            bytes_reclaimed_human: "25.0 MB".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"filesDeleted\":50"));
        assert!(json.contains("\"bytesReclaimedHuman\":\"25.0 MB\""));
    }

    #[test]
    fn trigger_response_serialization() {
        let response = TriggerPdfCacheCleanupResponse {
            task_id: uuid::Uuid::nil(),
            message: "Task queued".to_string(),
            max_age_days: 30,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"taskId\""));
        assert!(json.contains("\"maxAgeDays\":30"));
    }

    #[test]
    fn handle_clear_result_serialization() {
        let result = PdfHandleCacheClearResultDto { handles_closed: 5 };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"handlesClosed\":5"));
    }
}
