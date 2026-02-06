//! PDF page cache service for caching rendered PDF pages
//!
//! This service provides disk-based caching for rendered PDF pages to improve
//! performance when accessing PDF pages multiple times. Pages are cached as JPEG
//! files organized by book ID with bucket-based subdirectories to avoid having
//! too many files in a single directory.
//!
//! Cache structure:
//! ```text
//! {cache_dir}/pdf_pages/{book_id_prefix}/{book_id}/page_{number}_{dpi}.jpg
//! ```
//!
//! Example:
//! ```text
//! data/cache/pdf_pages/55/550e8400-e29b-41d4-a716-446655440000/page_1_150.jpg
//! ```

use anyhow::{Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio_util::io::ReaderStream;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Metadata for a cached page file
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CachedPageMeta {
    /// Path to the cached file
    pub path: PathBuf,
    /// File size in bytes
    pub size: u64,
    /// Last modified time as Unix timestamp (seconds)
    pub modified_unix: u64,
    /// ETag based on file path, size, and modified time
    pub etag: String,
}

/// Statistics for the PDF page cache
#[derive(Debug, Clone, Default, Serialize)]
pub struct CacheStats {
    /// Total number of cached files
    pub total_files: u64,
    /// Total size of cache in bytes
    pub total_size_bytes: u64,
    /// Number of unique books with cached pages
    pub book_count: u64,
    /// Age of the oldest file in days (if any files exist)
    pub oldest_file_age_days: Option<u32>,
    /// Path to the cache directory
    pub cache_dir: String,
}

impl CacheStats {
    /// Get human-readable size string (e.g., "50 MB")
    pub fn total_size_human(&self) -> String {
        humanize_bytes(self.total_size_bytes)
    }
}

/// Result of a cache cleanup operation
#[derive(Debug, Clone, Default, Serialize)]
pub struct CleanupResult {
    /// Number of files deleted
    pub files_deleted: u64,
    /// Number of bytes reclaimed
    pub bytes_reclaimed: u64,
}

impl CleanupResult {
    /// Get human-readable size string
    pub fn bytes_reclaimed_human(&self) -> String {
        humanize_bytes(self.bytes_reclaimed)
    }
}

/// Convert bytes to human-readable string
fn humanize_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Service for caching rendered PDF pages
pub struct PdfPageCache {
    cache_dir: PathBuf,
    enabled: bool,
}

impl PdfPageCache {
    /// Create a new PDF page cache service
    ///
    /// # Arguments
    /// * `cache_dir` - Base directory for the cache
    /// * `enabled` - Whether caching is enabled
    pub fn new(cache_dir: impl AsRef<Path>, enabled: bool) -> Self {
        Self {
            cache_dir: cache_dir.as_ref().to_path_buf(),
            enabled,
        }
    }

    /// Check if caching is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the base cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Get the cache path for a specific page
    ///
    /// # Arguments
    /// * `book_id` - The book's UUID
    /// * `page` - Page number (1-indexed)
    /// * `dpi` - DPI the page was rendered at
    fn cache_path(&self, book_id: Uuid, page: i32, dpi: u16) -> PathBuf {
        let id_str = book_id.to_string();
        let prefix = &id_str[..2]; // First 2 characters for bucketing

        self.cache_dir
            .join("pdf_pages")
            .join(prefix)
            .join(&id_str)
            .join(format!("page_{}_{}.jpg", page, dpi))
    }

    /// Get the cache directory for a book
    #[allow(dead_code)]
    fn book_cache_dir(&self, book_id: Uuid) -> PathBuf {
        let id_str = book_id.to_string();
        let prefix = &id_str[..2];

        self.cache_dir.join("pdf_pages").join(prefix).join(&id_str)
    }

    /// Get a cached page if it exists
    ///
    /// # Arguments
    /// * `book_id` - The book's UUID
    /// * `page` - Page number (1-indexed)
    /// * `dpi` - DPI the page was rendered at
    ///
    /// # Returns
    /// * `Some(Vec<u8>)` - The cached page data
    /// * `None` - If caching is disabled or the page is not cached
    pub async fn get(&self, book_id: Uuid, page: i32, dpi: u16) -> Option<Vec<u8>> {
        if !self.enabled {
            return None;
        }

        let path = self.cache_path(book_id, page, dpi);
        match fs::read(&path).await {
            Ok(data) => {
                debug!(
                    "PDF cache hit: book={}, page={}, dpi={}",
                    book_id, page, dpi
                );
                Some(data)
            }
            Err(_) => {
                debug!(
                    "PDF cache miss: book={}, page={}, dpi={}",
                    book_id, page, dpi
                );
                None
            }
        }
    }

    /// Store a rendered page in the cache
    ///
    /// # Arguments
    /// * `book_id` - The book's UUID
    /// * `page` - Page number (1-indexed)
    /// * `dpi` - DPI the page was rendered at
    /// * `data` - The rendered page image data
    ///
    /// # Returns
    /// * `Ok(())` - If the page was cached successfully
    /// * `Err` - If caching is disabled or writing failed
    pub async fn set(&self, book_id: Uuid, page: i32, dpi: u16, data: &[u8]) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let path = self.cache_path(book_id, page, dpi);

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create cache directory: {:?}", parent))?;
        }

        // Write the cached page
        fs::write(&path, data)
            .await
            .with_context(|| format!("Failed to write cached page to {:?}", path))?;

        debug!(
            "PDF cache stored: book={}, page={}, dpi={}, size={}",
            book_id,
            page,
            dpi,
            data.len()
        );

        Ok(())
    }

    /// Invalidate (delete) all cached pages for a book
    ///
    /// This should be called when a book is deleted or its file changes.
    ///
    /// # Arguments
    /// * `book_id` - The book's UUID
    ///
    /// # Returns
    /// * `Ok(())` - If the cache was invalidated (or didn't exist)
    /// * `Err` - If deletion failed
    #[allow(dead_code)]
    pub async fn invalidate_book(&self, book_id: Uuid) -> Result<()> {
        let dir = self.book_cache_dir(book_id);

        // Check if directory exists before trying to delete
        if fs::metadata(&dir).await.is_ok() {
            fs::remove_dir_all(&dir)
                .await
                .with_context(|| format!("Failed to invalidate PDF cache for book {}", book_id))?;

            debug!("PDF cache invalidated for book {}", book_id);
        }

        Ok(())
    }

    /// Check if a specific page is cached
    ///
    /// # Arguments
    /// * `book_id` - The book's UUID
    /// * `page` - Page number (1-indexed)
    /// * `dpi` - DPI the page was rendered at
    #[allow(dead_code)]
    pub async fn is_cached(&self, book_id: Uuid, page: i32, dpi: u16) -> bool {
        if !self.enabled {
            return false;
        }

        let path = self.cache_path(book_id, page, dpi);
        fs::metadata(&path).await.is_ok()
    }

    /// Get metadata for a cached page (for HTTP conditional requests)
    ///
    /// Returns file metadata including size, modified time, and ETag for use
    /// with HTTP caching headers (ETag, Last-Modified, If-None-Match, etc.)
    ///
    /// # Arguments
    /// * `book_id` - The book's UUID
    /// * `page` - Page number (1-indexed)
    /// * `dpi` - DPI the page was rendered at
    ///
    /// # Returns
    /// * `Some(CachedPageMeta)` - Metadata for the cached page
    /// * `None` - If caching is disabled or the page is not cached
    pub async fn get_metadata(&self, book_id: Uuid, page: i32, dpi: u16) -> Option<CachedPageMeta> {
        if !self.enabled {
            return None;
        }

        let path = self.cache_path(book_id, page, dpi);
        let metadata = fs::metadata(&path).await.ok()?;

        let size = metadata.len();
        let modified_unix = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Generate ETag from path + size + modified time for uniqueness
        let etag = format!(
            "\"{:x}-{:x}-{:x}\"",
            book_id.as_u128() ^ (page as u128) ^ (dpi as u128),
            size,
            modified_unix
        );

        Some(CachedPageMeta {
            path,
            size,
            modified_unix,
            etag,
        })
    }

    /// Open a cached page file for streaming
    ///
    /// Returns a tokio file handle that can be converted to a stream for
    /// efficient serving without loading the entire file into memory.
    ///
    /// # Arguments
    /// * `book_id` - The book's UUID
    /// * `page` - Page number (1-indexed)
    /// * `dpi` - DPI the page was rendered at
    ///
    /// # Returns
    /// * `Some(ReaderStream)` - A stream for reading the cached file
    /// * `None` - If caching is disabled or the page is not cached
    pub async fn get_stream(
        &self,
        book_id: Uuid,
        page: i32,
        dpi: u16,
    ) -> Option<ReaderStream<tokio::fs::File>> {
        if !self.enabled {
            return None;
        }

        let path = self.cache_path(book_id, page, dpi);
        let file = tokio::fs::File::open(&path).await.ok()?;

        debug!(
            "PDF cache streaming: book={}, page={}, dpi={}",
            book_id, page, dpi
        );

        Some(ReaderStream::new(file))
    }

    /// Get cache statistics for a book
    ///
    /// # Returns
    /// * Number of cached pages and total size in bytes
    #[allow(dead_code)]
    pub async fn get_book_stats(&self, book_id: Uuid) -> Result<(usize, u64)> {
        let dir = self.book_cache_dir(book_id);

        if fs::metadata(&dir).await.is_err() {
            return Ok((0, 0));
        }

        let mut count = 0;
        let mut total_size: u64 = 0;

        let mut entries = fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if let Ok(metadata) = entry.metadata().await
                && metadata.is_file()
            {
                count += 1;
                total_size += metadata.len();
            }
        }

        Ok((count, total_size))
    }

    /// Clear the entire PDF page cache
    ///
    /// Use with caution - this removes all cached PDF pages.
    pub async fn clear_all(&self) -> Result<CleanupResult> {
        let pdf_pages_dir = self.cache_dir.join("pdf_pages");

        // Get stats before clearing
        let stats = self.get_total_stats().await?;

        if fs::metadata(&pdf_pages_dir).await.is_ok() {
            fs::remove_dir_all(&pdf_pages_dir).await.with_context(|| {
                format!("Failed to clear PDF cache directory: {:?}", pdf_pages_dir)
            })?;

            warn!("Cleared entire PDF page cache");
        }

        Ok(CleanupResult {
            files_deleted: stats.total_files,
            bytes_reclaimed: stats.total_size_bytes,
        })
    }

    /// Get total cache statistics (file count, total size, book count)
    ///
    /// Walks the entire cache directory to gather statistics.
    pub async fn get_total_stats(&self) -> Result<CacheStats> {
        let pdf_pages_dir = self.cache_dir.join("pdf_pages");

        let mut stats = CacheStats {
            cache_dir: pdf_pages_dir.to_string_lossy().to_string(),
            ..Default::default()
        };

        if fs::metadata(&pdf_pages_dir).await.is_err() {
            return Ok(stats);
        }

        let now = SystemTime::now();
        let mut oldest_modified: Option<SystemTime> = None;

        // Walk through prefix buckets (e.g., "55", "aa", etc.)
        let mut bucket_entries = fs::read_dir(&pdf_pages_dir).await?;
        while let Some(bucket_entry) = bucket_entries.next_entry().await? {
            if !bucket_entry.file_type().await?.is_dir() {
                continue;
            }

            // Walk through book directories
            let mut book_entries = fs::read_dir(bucket_entry.path()).await?;
            while let Some(book_entry) = book_entries.next_entry().await? {
                if !book_entry.file_type().await?.is_dir() {
                    continue;
                }

                stats.book_count += 1;

                // Walk through page files
                let mut page_entries = fs::read_dir(book_entry.path()).await?;
                while let Some(page_entry) = page_entries.next_entry().await? {
                    let metadata = page_entry.metadata().await?;
                    if !metadata.is_file() {
                        continue;
                    }

                    stats.total_files += 1;
                    stats.total_size_bytes += metadata.len();

                    // Track oldest file
                    if let Ok(modified) = metadata.modified() {
                        match oldest_modified {
                            None => oldest_modified = Some(modified),
                            Some(oldest) if modified < oldest => oldest_modified = Some(modified),
                            _ => {}
                        }
                    }
                }
            }
        }

        // Calculate age of oldest file
        if let Some(oldest) = oldest_modified
            && let Ok(age) = now.duration_since(oldest)
        {
            stats.oldest_file_age_days = Some((age.as_secs() / 86400) as u32);
        }

        Ok(stats)
    }

    /// Delete cached pages older than the specified number of days
    ///
    /// # Arguments
    /// * `max_age_days` - Maximum age in days. Pages older than this will be deleted.
    ///   If 0, no pages are deleted (no-op).
    ///
    /// # Returns
    /// * `CleanupResult` with files deleted and bytes reclaimed
    pub async fn cleanup_old_pages(&self, max_age_days: u32) -> Result<CleanupResult> {
        if max_age_days == 0 {
            debug!("PDF cache cleanup skipped: max_age_days is 0");
            return Ok(CleanupResult::default());
        }

        let pdf_pages_dir = self.cache_dir.join("pdf_pages");

        if fs::metadata(&pdf_pages_dir).await.is_err() {
            return Ok(CleanupResult::default());
        }

        let cutoff = SystemTime::now() - Duration::from_secs(max_age_days as u64 * 86400);
        let mut result = CleanupResult::default();
        let mut empty_dirs: Vec<PathBuf> = Vec::new();

        // Walk through prefix buckets
        let mut bucket_entries = fs::read_dir(&pdf_pages_dir).await?;
        while let Some(bucket_entry) = bucket_entries.next_entry().await? {
            if !bucket_entry.file_type().await?.is_dir() {
                continue;
            }

            let bucket_path = bucket_entry.path();

            // Walk through book directories
            let mut book_entries = fs::read_dir(&bucket_path).await?;
            while let Some(book_entry) = book_entries.next_entry().await? {
                if !book_entry.file_type().await?.is_dir() {
                    continue;
                }

                let book_path = book_entry.path();
                let mut book_has_remaining_files = false;

                // Walk through page files
                let mut page_entries = fs::read_dir(&book_path).await?;
                while let Some(page_entry) = page_entries.next_entry().await? {
                    let metadata = page_entry.metadata().await?;
                    if !metadata.is_file() {
                        continue;
                    }

                    // Check if file is older than cutoff
                    let should_delete = metadata.modified().map(|m| m < cutoff).unwrap_or(false);

                    if should_delete {
                        let file_size = metadata.len();
                        let file_path = page_entry.path();

                        if let Err(e) = fs::remove_file(&file_path).await {
                            warn!("Failed to delete cached page {:?}: {}", file_path, e);
                        } else {
                            result.files_deleted += 1;
                            result.bytes_reclaimed += file_size;
                        }
                    } else {
                        book_has_remaining_files = true;
                    }
                }

                // Mark empty book directories for cleanup
                if !book_has_remaining_files {
                    empty_dirs.push(book_path);
                }
            }
        }

        // Clean up empty book directories
        for dir in empty_dirs {
            if let Err(e) = fs::remove_dir(&dir).await {
                debug!("Could not remove empty book directory {:?}: {}", dir, e);
            }
        }

        // Clean up empty bucket directories
        let mut bucket_entries = fs::read_dir(&pdf_pages_dir).await?;
        while let Some(bucket_entry) = bucket_entries.next_entry().await? {
            if bucket_entry.file_type().await?.is_dir() {
                // Check if bucket is empty
                let mut entries = fs::read_dir(bucket_entry.path()).await?;
                if entries.next_entry().await?.is_none() {
                    let _ = fs::remove_dir(bucket_entry.path()).await;
                }
            }
        }

        if result.files_deleted > 0 {
            info!(
                "PDF cache cleanup: deleted {} files, reclaimed {}",
                result.files_deleted,
                result.bytes_reclaimed_human()
            );
        } else {
            debug!("PDF cache cleanup: no old files to delete");
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_cache() -> (PdfPageCache, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let cache = PdfPageCache::new(temp_dir.path(), true);
        (cache, temp_dir)
    }

    #[test]
    fn test_new_cache() {
        let (cache, _temp_dir) = create_test_cache();
        assert!(cache.is_enabled());
    }

    #[test]
    fn test_new_cache_disabled() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let cache = PdfPageCache::new(temp_dir.path(), false);
        assert!(!cache.is_enabled());
    }

    #[test]
    fn test_cache_path_generation() {
        let (cache, _temp_dir) = create_test_cache();
        let book_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();

        let path = cache.cache_path(book_id, 1, 150);
        let path_str = path.to_string_lossy();

        assert!(path_str.contains("pdf_pages"));
        assert!(path_str.contains("55")); // Bucket prefix
        assert!(path_str.contains("550e8400-e29b-41d4-a716-446655440000"));
        assert!(path_str.ends_with("page_1_150.jpg"));
    }

    #[test]
    fn test_cache_path_different_pages() {
        let (cache, _temp_dir) = create_test_cache();
        let book_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();

        let path1 = cache.cache_path(book_id, 1, 150);
        let path2 = cache.cache_path(book_id, 2, 150);
        let path3 = cache.cache_path(book_id, 1, 300);

        // Different pages should have different paths
        assert_ne!(path1, path2);
        // Same page, different DPI should have different paths
        assert_ne!(path1, path3);
    }

    #[test]
    fn test_book_cache_dir() {
        let (cache, _temp_dir) = create_test_cache();
        let book_id = Uuid::parse_str("aa0e8400-e29b-41d4-a716-446655440000").unwrap();

        let dir = cache.book_cache_dir(book_id);
        let dir_str = dir.to_string_lossy();

        assert!(dir_str.contains("pdf_pages"));
        assert!(dir_str.contains("aa")); // Bucket prefix
        assert!(dir_str.ends_with("aa0e8400-e29b-41d4-a716-446655440000"));
    }

    #[tokio::test]
    async fn test_get_nonexistent_page() {
        let (cache, _temp_dir) = create_test_cache();
        let book_id = Uuid::new_v4();

        let result = cache.get(book_id, 1, 150).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_disabled_cache() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let cache = PdfPageCache::new(temp_dir.path(), false);
        let book_id = Uuid::new_v4();

        let result = cache.get(book_id, 1, 150).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_set_and_get() {
        let (cache, _temp_dir) = create_test_cache();
        let book_id = Uuid::new_v4();
        let test_data = b"fake jpeg data for testing";

        // Store the page
        cache.set(book_id, 1, 150, test_data).await.unwrap();

        // Retrieve it
        let result = cache.get(book_id, 1, 150).await;
        assert!(result.is_some());
        assert_eq!(result.unwrap(), test_data);
    }

    #[tokio::test]
    async fn test_set_disabled_cache() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let cache = PdfPageCache::new(temp_dir.path(), false);
        let book_id = Uuid::new_v4();
        let test_data = b"fake jpeg data";

        // Should succeed but not actually store
        cache.set(book_id, 1, 150, test_data).await.unwrap();

        // Verify nothing was stored
        let path = cache.cache_path(book_id, 1, 150);
        assert!(fs::metadata(&path).await.is_err());
    }

    #[tokio::test]
    async fn test_is_cached() {
        let (cache, _temp_dir) = create_test_cache();
        let book_id = Uuid::new_v4();

        // Initially not cached
        assert!(!cache.is_cached(book_id, 1, 150).await);

        // Store a page
        cache.set(book_id, 1, 150, b"test data").await.unwrap();

        // Now it should be cached
        assert!(cache.is_cached(book_id, 1, 150).await);

        // Different page should not be cached
        assert!(!cache.is_cached(book_id, 2, 150).await);
    }

    #[tokio::test]
    async fn test_invalidate_book() {
        let (cache, _temp_dir) = create_test_cache();
        let book_id = Uuid::new_v4();

        // Store multiple pages
        cache.set(book_id, 1, 150, b"page 1").await.unwrap();
        cache.set(book_id, 2, 150, b"page 2").await.unwrap();
        cache.set(book_id, 3, 150, b"page 3").await.unwrap();

        // Verify they're cached
        assert!(cache.is_cached(book_id, 1, 150).await);
        assert!(cache.is_cached(book_id, 2, 150).await);
        assert!(cache.is_cached(book_id, 3, 150).await);

        // Invalidate the book
        cache.invalidate_book(book_id).await.unwrap();

        // All pages should be gone
        assert!(!cache.is_cached(book_id, 1, 150).await);
        assert!(!cache.is_cached(book_id, 2, 150).await);
        assert!(!cache.is_cached(book_id, 3, 150).await);
    }

    #[tokio::test]
    async fn test_invalidate_nonexistent_book() {
        let (cache, _temp_dir) = create_test_cache();
        let book_id = Uuid::new_v4();

        // Should not error when invalidating non-existent book
        let result = cache.invalidate_book(book_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_book_stats() {
        let (cache, _temp_dir) = create_test_cache();
        let book_id = Uuid::new_v4();

        // Initially empty
        let (count, size) = cache.get_book_stats(book_id).await.unwrap();
        assert_eq!(count, 0);
        assert_eq!(size, 0);

        // Store some pages
        cache.set(book_id, 1, 150, b"page 1 data").await.unwrap();
        cache
            .set(book_id, 2, 150, b"page 2 longer data")
            .await
            .unwrap();

        // Check stats
        let (count, size) = cache.get_book_stats(book_id).await.unwrap();
        assert_eq!(count, 2);
        assert_eq!(size, 11 + 18); // "page 1 data" + "page 2 longer data"
    }

    #[tokio::test]
    async fn test_clear_all() {
        let (cache, _temp_dir) = create_test_cache();
        let book_id1 = Uuid::new_v4();
        let book_id2 = Uuid::new_v4();

        // Store pages for multiple books
        cache.set(book_id1, 1, 150, b"book 1 page 1").await.unwrap();
        cache.set(book_id2, 1, 150, b"book 2 page 1").await.unwrap();

        // Clear all
        let result = cache.clear_all().await.unwrap();

        // Verify result
        assert_eq!(result.files_deleted, 2);
        assert_eq!(result.bytes_reclaimed, 13 + 13); // "book 1 page 1" + "book 2 page 1"

        // Everything should be gone
        assert!(!cache.is_cached(book_id1, 1, 150).await);
        assert!(!cache.is_cached(book_id2, 1, 150).await);
    }

    #[tokio::test]
    async fn test_clear_all_empty_cache() {
        let (cache, _temp_dir) = create_test_cache();

        // Should not error when clearing empty cache
        let result = cache.clear_all().await.unwrap();
        assert_eq!(result.files_deleted, 0);
        assert_eq!(result.bytes_reclaimed, 0);
    }

    #[tokio::test]
    async fn test_get_total_stats_empty() {
        let (cache, _temp_dir) = create_test_cache();

        let stats = cache.get_total_stats().await.unwrap();
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_size_bytes, 0);
        assert_eq!(stats.book_count, 0);
        assert!(stats.oldest_file_age_days.is_none());
    }

    #[tokio::test]
    async fn test_get_total_stats_with_data() {
        let (cache, _temp_dir) = create_test_cache();
        let book_id1 = Uuid::new_v4();
        let book_id2 = Uuid::new_v4();

        // Store pages for multiple books
        cache.set(book_id1, 1, 150, b"page 1 data").await.unwrap();
        cache.set(book_id1, 2, 150, b"page 2 data").await.unwrap();
        cache.set(book_id2, 1, 150, b"page 1 data").await.unwrap();

        let stats = cache.get_total_stats().await.unwrap();
        assert_eq!(stats.total_files, 3);
        assert_eq!(stats.total_size_bytes, 11 * 3); // "page X data" = 11 bytes each
        assert_eq!(stats.book_count, 2);
        // Age should be 0 days since we just created the files
        assert!(stats.oldest_file_age_days == Some(0) || stats.oldest_file_age_days.is_none());
    }

    #[tokio::test]
    async fn test_get_total_stats_human_size() {
        let stats = CacheStats {
            total_size_bytes: 1024 * 1024 * 50, // 50 MB
            ..Default::default()
        };
        assert_eq!(stats.total_size_human(), "50.0 MB");

        let stats2 = CacheStats {
            total_size_bytes: 1024 * 1024 * 1024 * 2, // 2 GB
            ..Default::default()
        };
        assert_eq!(stats2.total_size_human(), "2.0 GB");

        let stats3 = CacheStats {
            total_size_bytes: 512 * 1024, // 512 KB
            ..Default::default()
        };
        assert_eq!(stats3.total_size_human(), "512.0 KB");

        let stats4 = CacheStats {
            total_size_bytes: 500, // 500 B
            ..Default::default()
        };
        assert_eq!(stats4.total_size_human(), "500 B");
    }

    #[tokio::test]
    async fn test_cleanup_old_pages_zero_days() {
        let (cache, _temp_dir) = create_test_cache();
        let book_id = Uuid::new_v4();

        // Store a page
        cache.set(book_id, 1, 150, b"test data").await.unwrap();

        // Cleanup with 0 days should be a no-op
        let result = cache.cleanup_old_pages(0).await.unwrap();
        assert_eq!(result.files_deleted, 0);
        assert_eq!(result.bytes_reclaimed, 0);

        // Page should still exist
        assert!(cache.is_cached(book_id, 1, 150).await);
    }

    #[tokio::test]
    async fn test_cleanup_old_pages_empty_cache() {
        let (cache, _temp_dir) = create_test_cache();

        // Cleanup on empty cache should succeed
        let result = cache.cleanup_old_pages(30).await.unwrap();
        assert_eq!(result.files_deleted, 0);
        assert_eq!(result.bytes_reclaimed, 0);
    }

    #[tokio::test]
    async fn test_cleanup_old_pages_recent_files() {
        let (cache, _temp_dir) = create_test_cache();
        let book_id = Uuid::new_v4();

        // Store a page (created now)
        cache.set(book_id, 1, 150, b"test data").await.unwrap();

        // Cleanup with 30 days should not delete recent files
        let result = cache.cleanup_old_pages(30).await.unwrap();
        assert_eq!(result.files_deleted, 0);
        assert_eq!(result.bytes_reclaimed, 0);

        // Page should still exist
        assert!(cache.is_cached(book_id, 1, 150).await);
    }

    #[tokio::test]
    async fn test_cleanup_result_human_size() {
        let result = CleanupResult {
            files_deleted: 10,
            bytes_reclaimed: 1024 * 1024 * 25, // 25 MB
        };
        assert_eq!(result.bytes_reclaimed_human(), "25.0 MB");
    }

    #[test]
    fn test_humanize_bytes() {
        assert_eq!(humanize_bytes(0), "0 B");
        assert_eq!(humanize_bytes(512), "512 B");
        assert_eq!(humanize_bytes(1024), "1.0 KB");
        assert_eq!(humanize_bytes(1536), "1.5 KB");
        assert_eq!(humanize_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(humanize_bytes(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(humanize_bytes(1024 * 1024 * 1024 * 5), "5.0 GB");
    }

    #[tokio::test]
    async fn test_get_metadata() {
        let (cache, _temp_dir) = create_test_cache();
        let book_id = Uuid::new_v4();
        let test_data = b"test image data for metadata";

        // No metadata for uncached page
        assert!(cache.get_metadata(book_id, 1, 150).await.is_none());

        // Store a page
        cache.set(book_id, 1, 150, test_data).await.unwrap();

        // Get metadata
        let meta = cache.get_metadata(book_id, 1, 150).await;
        assert!(meta.is_some());

        let meta = meta.unwrap();
        assert_eq!(meta.size, test_data.len() as u64);
        assert!(meta.modified_unix > 0);
        assert!(meta.etag.starts_with('"') && meta.etag.ends_with('"'));
    }

    #[tokio::test]
    async fn test_get_metadata_disabled_cache() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let cache = PdfPageCache::new(temp_dir.path(), false);
        let book_id = Uuid::new_v4();

        // Disabled cache returns None for metadata
        assert!(cache.get_metadata(book_id, 1, 150).await.is_none());
    }

    #[tokio::test]
    async fn test_get_stream() {
        use tokio_stream::StreamExt;

        let (cache, _temp_dir) = create_test_cache();
        let book_id = Uuid::new_v4();
        let test_data = b"test image data for streaming";

        // No stream for uncached page
        assert!(cache.get_stream(book_id, 1, 150).await.is_none());

        // Store a page
        cache.set(book_id, 1, 150, test_data).await.unwrap();

        // Get stream and read data
        let stream = cache.get_stream(book_id, 1, 150).await;
        assert!(stream.is_some());

        let mut stream = stream.unwrap();
        let mut collected = Vec::new();
        while let Some(chunk) = stream.next().await {
            collected.extend_from_slice(&chunk.unwrap());
        }
        assert_eq!(collected, test_data);
    }

    #[tokio::test]
    async fn test_get_stream_disabled_cache() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let cache = PdfPageCache::new(temp_dir.path(), false);
        let book_id = Uuid::new_v4();

        // Disabled cache returns None for stream
        assert!(cache.get_stream(book_id, 1, 150).await.is_none());
    }

    #[tokio::test]
    async fn test_etag_uniqueness() {
        let (cache, _temp_dir) = create_test_cache();
        let book_id = Uuid::new_v4();

        // Store pages with same data
        cache.set(book_id, 1, 150, b"same data").await.unwrap();
        cache.set(book_id, 2, 150, b"same data").await.unwrap();
        cache.set(book_id, 1, 300, b"same data").await.unwrap();

        // Get metadata for all
        let meta1 = cache.get_metadata(book_id, 1, 150).await.unwrap();
        let meta2 = cache.get_metadata(book_id, 2, 150).await.unwrap();
        let meta3 = cache.get_metadata(book_id, 1, 300).await.unwrap();

        // ETags should be different (different page numbers/DPI)
        assert_ne!(meta1.etag, meta2.etag);
        assert_ne!(meta1.etag, meta3.etag);
        assert_ne!(meta2.etag, meta3.etag);
    }
}
