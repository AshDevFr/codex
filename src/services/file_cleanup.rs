//! File cleanup service for managing orphaned thumbnails and cover files
//!
//! This service provides methods to:
//! - Delete book thumbnails and cover references
//! - Delete series cover files
//! - Scan for orphaned files (thumbnails/covers without database records)
//! - Clean up orphaned files

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::config::FilesConfig;

/// Statistics from a cleanup operation
#[derive(Debug, Clone, Default)]
pub struct CleanupStats {
    /// Number of thumbnails deleted
    pub thumbnails_deleted: u32,
    /// Number of cover files deleted
    pub covers_deleted: u32,
    /// Total bytes freed
    pub bytes_freed: u64,
    /// Number of files that failed to delete
    pub failures: u32,
    /// Error messages for failed deletions
    pub errors: Vec<String>,
}

impl CleanupStats {
    /// Create a new empty stats instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge another stats instance into this one
    pub fn merge(&mut self, other: CleanupStats) {
        self.thumbnails_deleted += other.thumbnails_deleted;
        self.covers_deleted += other.covers_deleted;
        self.bytes_freed += other.bytes_freed;
        self.failures += other.failures;
        self.errors.extend(other.errors);
    }
}

/// Type of orphaned file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrphanedFileType {
    /// A book thumbnail
    Thumbnail,
    /// A series cover
    Cover,
}

/// Service for cleaning up orphaned files
pub struct FileCleanupService {
    config: FilesConfig,
}

impl FileCleanupService {
    /// Create a new file cleanup service
    pub fn new(config: FilesConfig) -> Self {
        Self { config }
    }

    /// Get the thumbnail directory path
    pub fn get_thumbnail_dir(&self) -> PathBuf {
        PathBuf::from(&self.config.thumbnail_dir).join("books")
    }

    /// Get the covers directory path
    pub fn get_covers_dir(&self) -> PathBuf {
        PathBuf::from(&self.config.uploads_dir).join("covers")
    }

    /// Get the thumbnail path for a book
    pub fn get_thumbnail_path(&self, book_id: Uuid) -> PathBuf {
        let id_str = book_id.to_string();
        let prefix = &id_str[..2];
        self.get_thumbnail_dir()
            .join(prefix)
            .join(format!("{}.jpg", book_id))
    }

    /// Get the cover path for a series
    pub fn get_series_cover_path(&self, series_id: Uuid) -> PathBuf {
        self.get_covers_dir().join(format!("{}.jpg", series_id))
    }

    /// Delete a book's thumbnail file
    ///
    /// Returns true if a file was deleted, false if it didn't exist
    pub async fn delete_book_thumbnail(&self, book_id: Uuid) -> Result<bool> {
        let path = self.get_thumbnail_path(book_id);
        self.delete_file_if_exists(&path).await
    }

    /// Delete a book's thumbnail by path (when path is known)
    ///
    /// Returns true if a file was deleted, false if it didn't exist
    pub async fn delete_thumbnail_by_path(&self, path: &Path) -> Result<bool> {
        self.delete_file_if_exists(path).await
    }

    /// Delete a series cover file
    ///
    /// Returns true if a file was deleted, false if it didn't exist
    pub async fn delete_series_cover(&self, series_id: Uuid) -> Result<bool> {
        let path = self.get_series_cover_path(series_id);
        self.delete_file_if_exists(&path).await
    }

    /// Delete a file at the given path if it exists
    async fn delete_file_if_exists(&self, path: &Path) -> Result<bool> {
        match fs::metadata(path).await {
            Ok(_) => {
                fs::remove_file(path)
                    .await
                    .with_context(|| format!("Failed to delete file: {:?}", path))?;
                debug!("Deleted file: {:?}", path);
                Ok(true)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("File not found (already deleted?): {:?}", path);
                Ok(false)
            }
            Err(e) => Err(e).with_context(|| format!("Failed to check file existence: {:?}", path)),
        }
    }

    /// Scan the thumbnail directory for all thumbnail files
    ///
    /// Returns a list of (path, book_id) tuples for all found thumbnails
    pub async fn scan_thumbnails(&self) -> Result<Vec<(PathBuf, Uuid)>> {
        let base_dir = self.get_thumbnail_dir();
        let mut results = Vec::new();

        if !base_dir.exists() {
            return Ok(results);
        }

        // Iterate through bucket directories (first 2 chars of UUID)
        let mut bucket_entries = fs::read_dir(&base_dir)
            .await
            .with_context(|| format!("Failed to read thumbnail directory: {:?}", base_dir))?;

        while let Some(bucket_entry) = bucket_entries.next_entry().await? {
            let bucket_path = bucket_entry.path();
            if !bucket_path.is_dir() {
                continue;
            }

            // Iterate through files in each bucket
            let mut file_entries = fs::read_dir(&bucket_path).await?;

            while let Some(file_entry) = file_entries.next_entry().await? {
                let file_path = file_entry.path();

                // Extract UUID from filename (format: {uuid}.jpg)
                if let Some(uuid) = self.extract_uuid_from_filename(&file_path) {
                    results.push((file_path, uuid));
                }
            }
        }

        Ok(results)
    }

    /// Scan the covers directory for all cover files
    ///
    /// Returns a list of (path, series_id) tuples for all found covers
    pub async fn scan_covers(&self) -> Result<Vec<(PathBuf, Uuid)>> {
        let covers_dir = self.get_covers_dir();
        let mut results = Vec::new();

        if !covers_dir.exists() {
            return Ok(results);
        }

        let mut entries = fs::read_dir(&covers_dir)
            .await
            .with_context(|| format!("Failed to read covers directory: {:?}", covers_dir))?;

        while let Some(entry) = entries.next_entry().await? {
            let file_path = entry.path();

            // Extract UUID from filename (format: {uuid}.jpg)
            if let Some(uuid) = self.extract_uuid_from_filename(&file_path) {
                results.push((file_path, uuid));
            }
        }

        Ok(results)
    }

    /// Extract UUID from a filename like "{uuid}.jpg"
    fn extract_uuid_from_filename(&self, path: &Path) -> Option<Uuid> {
        let stem = path.file_stem()?.to_str()?;
        Uuid::parse_str(stem).ok()
    }

    /// Delete multiple files and return stats
    pub async fn delete_files(
        &self,
        paths: Vec<PathBuf>,
        file_type: OrphanedFileType,
    ) -> CleanupStats {
        let mut stats = CleanupStats::new();

        for path in paths {
            // Get file size before deletion
            let size = match fs::metadata(&path).await {
                Ok(meta) => meta.len(),
                Err(_) => 0,
            };

            match fs::remove_file(&path).await {
                Ok(_) => {
                    match file_type {
                        OrphanedFileType::Thumbnail => stats.thumbnails_deleted += 1,
                        OrphanedFileType::Cover => stats.covers_deleted += 1,
                    }
                    stats.bytes_freed += size;
                    debug!("Deleted orphaned file: {:?}", path);
                }
                Err(e) => {
                    stats.failures += 1;
                    stats
                        .errors
                        .push(format!("Failed to delete {:?}: {}", path, e));
                    warn!("Failed to delete orphaned file {:?}: {}", path, e);
                }
            }
        }

        stats
    }

    /// Get file size, returning 0 if file doesn't exist or can't be read
    pub async fn get_file_size(&self, path: &Path) -> u64 {
        fs::metadata(path).await.map(|m| m.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config(temp_dir: &TempDir) -> FilesConfig {
        FilesConfig {
            thumbnail_dir: temp_dir
                .path()
                .join("thumbnails")
                .to_string_lossy()
                .to_string(),
            uploads_dir: temp_dir
                .path()
                .join("uploads")
                .to_string_lossy()
                .to_string(),
        }
    }

    #[test]
    fn test_thumbnail_path_generation() {
        let temp_dir = TempDir::new().unwrap();
        let service = FileCleanupService::new(test_config(&temp_dir));

        let book_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let path = service.get_thumbnail_path(book_id);

        assert!(path.to_string_lossy().contains("thumbnails/books/55"));
        assert!(path
            .to_string_lossy()
            .ends_with("550e8400-e29b-41d4-a716-446655440000.jpg"));
    }

    #[test]
    fn test_cover_path_generation() {
        let temp_dir = TempDir::new().unwrap();
        let service = FileCleanupService::new(test_config(&temp_dir));

        let series_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let path = service.get_series_cover_path(series_id);

        assert!(path.to_string_lossy().contains("uploads/covers"));
        assert!(path
            .to_string_lossy()
            .ends_with("550e8400-e29b-41d4-a716-446655440000.jpg"));
    }

    #[test]
    fn test_extract_uuid_from_filename() {
        let temp_dir = TempDir::new().unwrap();
        let service = FileCleanupService::new(test_config(&temp_dir));

        let path = PathBuf::from("/some/path/550e8400-e29b-41d4-a716-446655440000.jpg");
        let uuid = service.extract_uuid_from_filename(&path);

        assert!(uuid.is_some());
        assert_eq!(
            uuid.unwrap(),
            Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
        );

        // Invalid UUID
        let path = PathBuf::from("/some/path/not-a-uuid.jpg");
        let uuid = service.extract_uuid_from_filename(&path);
        assert!(uuid.is_none());

        // No extension
        let path = PathBuf::from("/some/path/550e8400-e29b-41d4-a716-446655440000");
        let uuid = service.extract_uuid_from_filename(&path);
        assert!(uuid.is_some());
    }

    #[tokio::test]
    async fn test_delete_book_thumbnail() {
        let temp_dir = TempDir::new().unwrap();
        let service = FileCleanupService::new(test_config(&temp_dir));

        let book_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let thumb_path = service.get_thumbnail_path(book_id);

        // Create the directory structure and file
        fs::create_dir_all(thumb_path.parent().unwrap())
            .await
            .unwrap();
        fs::write(&thumb_path, b"test thumbnail").await.unwrap();

        // Verify file exists
        assert!(fs::metadata(&thumb_path).await.is_ok());

        // Delete it
        let deleted = service.delete_book_thumbnail(book_id).await.unwrap();
        assert!(deleted);

        // Verify it's gone
        assert!(fs::metadata(&thumb_path).await.is_err());

        // Try deleting again - should return false
        let deleted = service.delete_book_thumbnail(book_id).await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_delete_series_cover() {
        let temp_dir = TempDir::new().unwrap();
        let service = FileCleanupService::new(test_config(&temp_dir));

        let series_id = Uuid::parse_str("660e8400-e29b-41d4-a716-446655440000").unwrap();
        let cover_path = service.get_series_cover_path(series_id);

        // Create the directory structure and file
        fs::create_dir_all(cover_path.parent().unwrap())
            .await
            .unwrap();
        fs::write(&cover_path, b"test cover").await.unwrap();

        // Verify file exists
        assert!(fs::metadata(&cover_path).await.is_ok());

        // Delete it
        let deleted = service.delete_series_cover(series_id).await.unwrap();
        assert!(deleted);

        // Verify it's gone
        assert!(fs::metadata(&cover_path).await.is_err());
    }

    #[tokio::test]
    async fn test_scan_thumbnails() {
        let temp_dir = TempDir::new().unwrap();
        let service = FileCleanupService::new(test_config(&temp_dir));

        // Create some thumbnail files
        let book_id1 = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let book_id2 = Uuid::parse_str("660e8400-e29b-41d4-a716-446655440000").unwrap();

        let path1 = service.get_thumbnail_path(book_id1);
        let path2 = service.get_thumbnail_path(book_id2);

        fs::create_dir_all(path1.parent().unwrap()).await.unwrap();
        fs::create_dir_all(path2.parent().unwrap()).await.unwrap();
        fs::write(&path1, b"thumb1").await.unwrap();
        fs::write(&path2, b"thumb2").await.unwrap();

        // Scan
        let thumbnails = service.scan_thumbnails().await.unwrap();

        assert_eq!(thumbnails.len(), 2);

        let ids: Vec<Uuid> = thumbnails.iter().map(|(_, id)| *id).collect();
        assert!(ids.contains(&book_id1));
        assert!(ids.contains(&book_id2));
    }

    #[tokio::test]
    async fn test_scan_covers() {
        let temp_dir = TempDir::new().unwrap();
        let service = FileCleanupService::new(test_config(&temp_dir));

        let covers_dir = service.get_covers_dir();
        fs::create_dir_all(&covers_dir).await.unwrap();

        // Create some cover files
        let series_id1 = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let series_id2 = Uuid::parse_str("660e8400-e29b-41d4-a716-446655440000").unwrap();

        let path1 = covers_dir.join(format!("{}.jpg", series_id1));
        let path2 = covers_dir.join(format!("{}.jpg", series_id2));

        fs::write(&path1, b"cover1").await.unwrap();
        fs::write(&path2, b"cover2").await.unwrap();

        // Scan
        let covers = service.scan_covers().await.unwrap();

        assert_eq!(covers.len(), 2);

        let ids: Vec<Uuid> = covers.iter().map(|(_, id)| *id).collect();
        assert!(ids.contains(&series_id1));
        assert!(ids.contains(&series_id2));
    }

    #[tokio::test]
    async fn test_delete_files_stats() {
        let temp_dir = TempDir::new().unwrap();
        let service = FileCleanupService::new(test_config(&temp_dir));

        let covers_dir = service.get_covers_dir();
        fs::create_dir_all(&covers_dir).await.unwrap();

        // Create test files
        let file1 = covers_dir.join("test1.jpg");
        let file2 = covers_dir.join("test2.jpg");

        fs::write(&file1, b"content1").await.unwrap();
        fs::write(&file2, b"longer_content2").await.unwrap();

        let paths = vec![file1.clone(), file2.clone()];
        let stats = service.delete_files(paths, OrphanedFileType::Cover).await;

        assert_eq!(stats.covers_deleted, 2);
        assert_eq!(stats.thumbnails_deleted, 0);
        assert_eq!(stats.failures, 0);
        assert!(stats.bytes_freed > 0);

        // Verify files are gone
        assert!(fs::metadata(&file1).await.is_err());
        assert!(fs::metadata(&file2).await.is_err());
    }

    #[test]
    fn test_cleanup_stats_merge() {
        let mut stats1 = CleanupStats {
            thumbnails_deleted: 5,
            covers_deleted: 2,
            bytes_freed: 1000,
            failures: 1,
            errors: vec!["error1".to_string()],
        };

        let stats2 = CleanupStats {
            thumbnails_deleted: 3,
            covers_deleted: 1,
            bytes_freed: 500,
            failures: 0,
            errors: vec![],
        };

        stats1.merge(stats2);

        assert_eq!(stats1.thumbnails_deleted, 8);
        assert_eq!(stats1.covers_deleted, 3);
        assert_eq!(stats1.bytes_freed, 1500);
        assert_eq!(stats1.failures, 1);
        assert_eq!(stats1.errors.len(), 1);
    }
}
