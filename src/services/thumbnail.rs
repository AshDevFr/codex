//! Thumbnail service for generating and managing cover images
//!
//! TODO: Remove allow(dead_code) once all thumbnail features are fully integrated

#![allow(dead_code)]

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use image::imageops::FilterType;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use tokio::fs;
use tokio_util::io::ReaderStream;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::config::FilesConfig;
use crate::db::entities::books;
use crate::db::repositories::{BookRepository, SeriesRepository, SettingsRepository};
use crate::events::{EntityChangeEvent, EntityEvent, EntityType, EventBroadcaster};

/// Detect image format from magic bytes for diagnostic purposes
fn detect_image_format(data: &[u8]) -> &'static str {
    if data.len() < 4 {
        return "unknown (too short)";
    }

    // Check magic bytes for common image formats
    match &data[..4] {
        // JPEG: FF D8 FF
        [0xFF, 0xD8, 0xFF, _] => "JPEG",
        // PNG: 89 50 4E 47
        [0x89, 0x50, 0x4E, 0x47] => "PNG",
        // GIF: 47 49 46 38
        [0x47, 0x49, 0x46, 0x38] => "GIF",
        // WebP: RIFF....WEBP
        [0x52, 0x49, 0x46, 0x46] if data.len() >= 12 && &data[8..12] == b"WEBP" => "WebP",
        // BMP: 42 4D
        [0x42, 0x4D, _, _] => "BMP",
        // TIFF: 49 49 2A 00 (little-endian) or 4D 4D 00 2A (big-endian)
        [0x49, 0x49, 0x2A, 0x00] | [0x4D, 0x4D, 0x00, 0x2A] => "TIFF",
        // AVIF/HEIF: ....ftyp
        _ if data.len() >= 12 && &data[4..8] == b"ftyp" => {
            // Check specific brand
            match &data[8..12] {
                b"avif" => "AVIF",
                b"heic" | b"heix" | b"mif1" => "HEIF",
                _ => "AVIF/HEIF (unknown brand)",
            }
        }
        // ICO: 00 00 01 00
        [0x00, 0x00, 0x01, 0x00] => "ICO",
        _ => "unknown",
    }
}

/// Format magic bytes as hex string for logging
fn format_magic_bytes(data: &[u8]) -> String {
    let len = data.len().min(16);
    data[..len]
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Metadata for a cached thumbnail file (for HTTP conditional caching)
#[derive(Debug, Clone)]
pub struct ThumbnailMeta {
    /// File size in bytes
    pub size: u64,
    /// Last modified time as Unix timestamp (seconds)
    pub modified_unix: u64,
    /// ETag based on book ID, size, and modified time
    pub etag: String,
}

/// Service for managing thumbnail cache
pub struct ThumbnailService {
    config: FilesConfig,
}

/// Settings loaded from database for thumbnail generation
#[derive(Debug, Clone)]
pub struct ThumbnailSettings {
    pub max_dimension: u32,
    pub jpeg_quality: u8,
}

impl Default for ThumbnailSettings {
    fn default() -> Self {
        Self {
            max_dimension: 400,
            jpeg_quality: 85,
        }
    }
}

/// Statistics for batch thumbnail generation
#[derive(Debug, Clone)]
pub struct GenerationStats {
    pub total: usize,
    pub generated: usize,
    pub skipped: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

impl ThumbnailService {
    /// Create a new thumbnail service
    pub fn new(config: FilesConfig) -> Self {
        Self { config }
    }

    /// Get thumbnail settings from database
    pub async fn get_settings(&self, db: &DatabaseConnection) -> Result<ThumbnailSettings> {
        let max_dimension = SettingsRepository::get_value::<i64>(db, "thumbnail.max_dimension")
            .await?
            .unwrap_or(400) as u32;

        let jpeg_quality = SettingsRepository::get_value::<i64>(db, "thumbnail.jpeg_quality")
            .await?
            .unwrap_or(85) as u8;

        Ok(ThumbnailSettings {
            max_dimension,
            jpeg_quality,
        })
    }

    /// Get the full path to thumbnail cache directory
    fn get_cache_base_dir(&self) -> PathBuf {
        PathBuf::from(&self.config.thumbnail_dir)
    }

    /// Get the uploads directory path
    pub fn get_uploads_dir(&self) -> PathBuf {
        PathBuf::from(&self.config.uploads_dir)
    }

    /// Get the subdirectory path for a book's thumbnail (based on first 2 chars of UUID)
    fn get_thumbnail_subdir(&self, book_id: Uuid) -> PathBuf {
        let id_str = book_id.to_string();
        let prefix = &id_str[..2]; // First 2 characters for bucketing
        self.get_cache_base_dir().join("books").join(prefix)
    }

    /// Get the full path where a book's thumbnail would be stored
    pub fn get_thumbnail_path(&self, book_id: Uuid) -> PathBuf {
        self.get_thumbnail_subdir(book_id)
            .join(format!("{}.jpg", book_id))
    }

    /// Check if a thumbnail exists for a book
    pub async fn thumbnail_exists(&self, book_id: Uuid) -> bool {
        let path = self.get_thumbnail_path(book_id);
        fs::metadata(&path).await.is_ok()
    }

    /// Read a thumbnail from cache
    pub async fn read_thumbnail(&self, book_id: Uuid) -> Result<Vec<u8>> {
        let path = self.get_thumbnail_path(book_id);
        fs::read(&path)
            .await
            .with_context(|| format!("Failed to read thumbnail from {:?}", path))
    }

    /// Get metadata for a cached thumbnail (for HTTP conditional requests)
    ///
    /// Returns file metadata including size, modified time, and ETag for use
    /// with HTTP caching headers (ETag, Last-Modified, If-None-Match, etc.)
    pub async fn get_thumbnail_metadata(&self, book_id: Uuid) -> Option<ThumbnailMeta> {
        let path = self.get_thumbnail_path(book_id);
        let metadata = fs::metadata(&path).await.ok()?;

        let size = metadata.len();
        let modified_unix = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Generate ETag from book_id + size + modified time for uniqueness
        let etag = format!("\"{:x}-{:x}-{:x}\"", book_id.as_u128(), size, modified_unix);

        Some(ThumbnailMeta {
            size,
            modified_unix,
            etag,
        })
    }

    /// Open a cached thumbnail for streaming
    ///
    /// Returns a stream for reading the cached file directly without loading
    /// the entire file into memory.
    pub async fn get_thumbnail_stream(
        &self,
        book_id: Uuid,
    ) -> Option<ReaderStream<tokio::fs::File>> {
        let path = self.get_thumbnail_path(book_id);
        let file = tokio::fs::File::open(&path).await.ok()?;
        debug!("Streaming thumbnail for book {}", book_id);
        Some(ReaderStream::new(file))
    }

    // ========== Series Thumbnail Methods ==========

    /// Get the subdirectory path for a series thumbnail (based on first 2 chars of UUID)
    fn get_series_thumbnail_subdir(&self, series_id: Uuid) -> PathBuf {
        let id_str = series_id.to_string();
        let prefix = &id_str[..2]; // First 2 characters for bucketing
        self.get_cache_base_dir().join("series").join(prefix)
    }

    /// Get the full path where a series thumbnail would be stored
    pub fn get_series_thumbnail_path(&self, series_id: Uuid) -> PathBuf {
        self.get_series_thumbnail_subdir(series_id)
            .join(format!("{}.jpg", series_id))
    }

    /// Check if a cached thumbnail exists for a series
    pub async fn series_thumbnail_exists(&self, series_id: Uuid) -> bool {
        let path = self.get_series_thumbnail_path(series_id);
        fs::metadata(&path).await.is_ok()
    }

    /// Get metadata for a cached series thumbnail (for HTTP conditional requests)
    pub async fn get_series_thumbnail_metadata(&self, series_id: Uuid) -> Option<ThumbnailMeta> {
        let path = self.get_series_thumbnail_path(series_id);
        let metadata = fs::metadata(&path).await.ok()?;

        let size = metadata.len();
        let modified_unix = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Generate ETag from series_id + size + modified time for uniqueness
        let etag = format!(
            "\"{:x}-{:x}-{:x}\"",
            series_id.as_u128(),
            size,
            modified_unix
        );

        Some(ThumbnailMeta {
            size,
            modified_unix,
            etag,
        })
    }

    /// Open a cached series thumbnail for streaming
    pub async fn get_series_thumbnail_stream(
        &self,
        series_id: Uuid,
    ) -> Option<ReaderStream<tokio::fs::File>> {
        let path = self.get_series_thumbnail_path(series_id);
        let file = tokio::fs::File::open(&path).await.ok()?;
        Some(ReaderStream::new(file))
    }

    /// Save series thumbnail data to disk cache
    pub async fn save_series_thumbnail(&self, series_id: Uuid, data: &[u8]) -> Result<PathBuf> {
        let subdir = self.get_series_thumbnail_subdir(series_id);
        let thumbnail_path = subdir.join(format!("{}.jpg", series_id));

        // Create directory if it doesn't exist
        fs::create_dir_all(&subdir).await.with_context(|| {
            format!("Failed to create series thumbnail directory: {:?}", subdir)
        })?;

        // Write thumbnail file
        fs::write(&thumbnail_path, data)
            .await
            .with_context(|| format!("Failed to write series thumbnail to {:?}", thumbnail_path))?;

        debug!("Saved series thumbnail to {:?}", thumbnail_path);
        Ok(thumbnail_path)
    }

    /// Delete a series thumbnail from cache
    pub async fn delete_series_thumbnail(&self, series_id: Uuid) -> Result<()> {
        let thumbnail_path = self.get_series_thumbnail_path(series_id);

        if fs::metadata(&thumbnail_path).await.is_ok() {
            fs::remove_file(&thumbnail_path).await.with_context(|| {
                format!("Failed to delete series thumbnail: {:?}", thumbnail_path)
            })?;
            debug!("Deleted series thumbnail: {:?}", thumbnail_path);
        }

        Ok(())
    }

    /// Generate a thumbnail from raw image data using configured settings
    ///
    /// This is a public method that can be used by both book and series thumbnail
    /// handlers to generate thumbnails with consistent settings from the database.
    /// Uses spawn_blocking internally for CPU-intensive image processing.
    pub async fn generate_thumbnail_from_image(
        &self,
        db: &DatabaseConnection,
        image_data: Vec<u8>,
    ) -> Result<Vec<u8>> {
        let settings = self.get_settings(db).await?;
        let max_dimension = settings.max_dimension;
        let jpeg_quality = settings.jpeg_quality;

        // Use spawn_blocking for CPU-intensive image processing
        tokio::task::spawn_blocking(move || {
            // Load image from bytes with detailed error context
            let img = image::load_from_memory(&image_data).map_err(|e| {
                let detected_format = detect_image_format(&image_data);
                let magic_bytes = format_magic_bytes(&image_data);
                anyhow!(
                    "Failed to load image: {} (size: {} bytes, detected format: {}, magic bytes: [{}])",
                    e,
                    image_data.len(),
                    detected_format,
                    magic_bytes
                )
            })?;

            // Calculate new dimensions while maintaining aspect ratio
            let (width, height) = (img.width(), img.height());
            let (new_width, new_height) = if width > height {
                let ratio = max_dimension as f32 / width as f32;
                (max_dimension, (height as f32 * ratio) as u32)
            } else {
                let ratio = max_dimension as f32 / height as f32;
                ((width as f32 * ratio) as u32, max_dimension)
            };

            // Resize using Lanczos3 filter for high quality
            let thumbnail = img.resize(new_width, new_height, FilterType::Lanczos3);

            // Encode as JPEG
            let mut output = Cursor::new(Vec::new());
            let mut encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, jpeg_quality);
            encoder
                .encode_image(&thumbnail)
                .context("Failed to encode thumbnail as JPEG")?;

            Ok(output.into_inner())
        })
        .await
        .context("Thumbnail generation task failed")?
    }

    /// Generate and save a thumbnail for a book
    ///
    /// Returns the path where the thumbnail was saved
    pub async fn generate_thumbnail(
        &self,
        db: &DatabaseConnection,
        book: &books::Model,
    ) -> Result<PathBuf> {
        // Check if thumbnail already exists
        let thumbnail_path = self.get_thumbnail_path(book.id);
        if fs::metadata(&thumbnail_path).await.is_ok() {
            debug!("Thumbnail already exists for book {}", book.id);
            return Ok(thumbnail_path);
        }

        info!(
            "Generating thumbnail for book {} ({})",
            book.id, book.file_name
        );

        // Get settings from database
        let settings = self.get_settings(db).await?;

        // Extract first page from book
        let image_data = self.extract_cover_image(book).await?;

        // Generate thumbnail
        let thumbnail_data =
            self.resize_image(&image_data, settings.max_dimension, settings.jpeg_quality)?;

        // Save to cache
        self.save_thumbnail(book.id, &thumbnail_data).await?;

        // Update book record in database
        let now = Utc::now();
        let mut book_active: books::ActiveModel = book.clone().into();
        book_active.thumbnail_path = Set(Some(thumbnail_path.to_string_lossy().to_string()));
        book_active.thumbnail_generated_at = Set(Some(now));
        book_active.updated_at = Set(now); // Update timestamp for cache-busting
        book_active.update(db).await?;

        Ok(thumbnail_path)
    }

    /// Save pre-generated thumbnail data to cache
    ///
    /// Used when a thumbnail is generated on-demand in a handler
    pub async fn save_generated_thumbnail(
        &self,
        db: &DatabaseConnection,
        book_id: Uuid,
        thumbnail_data: &[u8],
    ) -> Result<PathBuf> {
        let thumbnail_path = self.save_thumbnail(book_id, thumbnail_data).await?;

        // Update book record in database
        let book = BookRepository::get_by_id(db, book_id)
            .await?
            .ok_or_else(|| anyhow!("Book not found: {}", book_id))?;

        let now = Utc::now();
        let mut book_active: books::ActiveModel = book.into();
        book_active.thumbnail_path = Set(Some(thumbnail_path.to_string_lossy().to_string()));
        book_active.thumbnail_generated_at = Set(Some(now));
        book_active.updated_at = Set(now); // Update timestamp for cache-busting
        book_active.update(db).await?;

        Ok(thumbnail_path)
    }

    /// Save thumbnail data to disk
    async fn save_thumbnail(&self, book_id: Uuid, data: &[u8]) -> Result<PathBuf> {
        let subdir = self.get_thumbnail_subdir(book_id);
        let thumbnail_path = subdir.join(format!("{}.jpg", book_id));

        // Create directory if it doesn't exist
        fs::create_dir_all(&subdir)
            .await
            .with_context(|| format!("Failed to create thumbnail directory: {:?}", subdir))?;

        // Write thumbnail file
        fs::write(&thumbnail_path, data)
            .await
            .with_context(|| format!("Failed to write thumbnail to {:?}", thumbnail_path))?;

        debug!("Saved thumbnail to {:?}", thumbnail_path);
        Ok(thumbnail_path)
    }

    /// Delete a thumbnail from cache
    pub async fn delete_thumbnail(&self, db: &DatabaseConnection, book_id: Uuid) -> Result<()> {
        let thumbnail_path = self.get_thumbnail_path(book_id);

        // Delete file if it exists
        if fs::metadata(&thumbnail_path).await.is_ok() {
            fs::remove_file(&thumbnail_path)
                .await
                .with_context(|| format!("Failed to delete thumbnail: {:?}", thumbnail_path))?;
            debug!("Deleted thumbnail: {:?}", thumbnail_path);
        }

        // Update book record
        if let Some(book) = BookRepository::get_by_id(db, book_id).await? {
            let mut book_active: books::ActiveModel = book.into();
            book_active.thumbnail_path = Set(None);
            book_active.thumbnail_generated_at = Set(None);
            book_active.update(db).await?;
        }

        Ok(())
    }

    /// Generate thumbnails for multiple books (batch operation)
    pub async fn generate_thumbnails_batch(
        &self,
        db: &DatabaseConnection,
        book_ids: Vec<Uuid>,
        event_broadcaster: Option<&Arc<EventBroadcaster>>,
    ) -> Result<GenerationStats> {
        let total = book_ids.len();
        let mut generated = 0;
        let mut skipped = 0;
        let mut failed = 0;
        let mut errors = Vec::new();

        info!("Starting batch thumbnail generation for {} books", total);

        for book_id in book_ids {
            // Fetch book
            let book = match BookRepository::get_by_id(db, book_id).await? {
                Some(b) => b,
                None => {
                    warn!("Book not found: {}", book_id);
                    failed += 1;
                    errors.push(format!("Book not found: {}", book_id));
                    continue;
                }
            };

            // Check if thumbnail already exists
            if self.thumbnail_exists(book_id).await {
                debug!("Thumbnail already exists for book {}", book_id);
                skipped += 1;
                continue;
            }

            // Generate thumbnail
            match self.generate_thumbnail(db, &book).await {
                Ok(_) => {
                    generated += 1;
                    debug!("Generated thumbnail for book {}", book_id);

                    // Emit CoverUpdated event to notify UI
                    if let Some(broadcaster) = event_broadcaster {
                        // Get library_id from series
                        if let Ok(Some(series)) =
                            SeriesRepository::get_by_id(db, book.series_id).await
                        {
                            let event = EntityChangeEvent {
                                event: EntityEvent::CoverUpdated {
                                    entity_type: EntityType::Book,
                                    entity_id: book_id,
                                    library_id: Some(series.library_id),
                                },
                                user_id: None,
                                timestamp: Utc::now(),
                            };

                            match broadcaster.emit(event) {
                                Ok(count) => {
                                    debug!(
                                        "Emitted CoverUpdated event to {} subscribers for book thumbnail: {}",
                                        count, book_id
                                    );
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to emit CoverUpdated event for book thumbnail {}: {:?}",
                                        book_id, e
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    failed += 1;
                    let error_msg =
                        format!("Failed to generate thumbnail for book {}: {}", book_id, e);
                    error!("{}", error_msg);
                    errors.push(error_msg);
                }
            }
        }

        info!(
            "Batch thumbnail generation complete: {}/{} generated, {} skipped, {} failed",
            generated, total, skipped, failed
        );

        Ok(GenerationStats {
            total,
            generated,
            skipped,
            failed,
            errors,
        })
    }

    /// Extract cover image (first page) from a book
    async fn extract_cover_image(&self, book: &books::Model) -> Result<Vec<u8>> {
        let path = Path::new(&book.file_path);

        // Use the appropriate parser extraction function based on format
        let image_data = match book.format.to_uppercase().as_str() {
            "CBZ" => crate::parsers::cbz::extract_page_from_cbz(path, 1)?,
            #[cfg(feature = "rar")]
            "CBR" => crate::parsers::cbr::extract_page_from_cbr(path, 1)?,
            "EPUB" => crate::parsers::epub::extract_page_from_epub(path, 1)?,
            "PDF" => crate::parsers::pdf::extract_page_from_pdf(path, 1)?,
            _ => {
                return Err(anyhow!(
                    "Unsupported format for thumbnail generation: {}",
                    book.format
                ));
            }
        };

        Ok(image_data)
    }

    /// Resize an image to thumbnail size
    fn resize_image(
        &self,
        image_data: &[u8],
        max_dimension: u32,
        jpeg_quality: u8,
    ) -> Result<Vec<u8>> {
        // Load image from bytes with detailed error context
        let img = image::load_from_memory(image_data).map_err(|e| {
            let detected_format = detect_image_format(image_data);
            let magic_bytes = format_magic_bytes(image_data);
            anyhow!(
                "Failed to load image: {} (size: {} bytes, detected format: {}, magic bytes: [{}])",
                e,
                image_data.len(),
                detected_format,
                magic_bytes
            )
        })?;

        // Calculate new dimensions while maintaining aspect ratio
        let (width, height) = (img.width(), img.height());
        let (new_width, new_height) = if width > height {
            let ratio = max_dimension as f32 / width as f32;
            (max_dimension, (height as f32 * ratio) as u32)
        } else {
            let ratio = max_dimension as f32 / height as f32;
            ((width as f32 * ratio) as u32, max_dimension)
        };

        // Resize using Lanczos3 filter for high quality
        let thumbnail = img.resize(new_width, new_height, FilterType::Lanczos3);

        // Encode as JPEG
        let mut output = Cursor::new(Vec::new());
        let mut encoder =
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, jpeg_quality);
        encoder
            .encode_image(&thumbnail)
            .context("Failed to encode thumbnail as JPEG")?;

        Ok(output.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_files_config() -> FilesConfig {
        FilesConfig {
            thumbnail_dir: "data/thumbnails".to_string(),
            uploads_dir: "data/uploads".to_string(),
        }
    }

    #[test]
    fn test_thumbnail_path_generation() {
        let service = ThumbnailService::new(test_files_config());

        let book_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let path = service.get_thumbnail_path(book_id);

        assert!(path.to_string_lossy().contains("data/thumbnails/books/55"));
        assert!(path
            .to_string_lossy()
            .ends_with("550e8400-e29b-41d4-a716-446655440000.jpg"));
    }

    #[test]
    fn test_thumbnail_subdirectory_bucketing() {
        let service = ThumbnailService::new(test_files_config());

        let book_id1 = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let book_id2 = Uuid::parse_str("55ffffff-e29b-41d4-a716-446655440000").unwrap();
        let book_id3 = Uuid::parse_str("aaaaaaaa-e29b-41d4-a716-446655440000").unwrap();

        let subdir1 = service.get_thumbnail_subdir(book_id1);
        let subdir2 = service.get_thumbnail_subdir(book_id2);
        let subdir3 = service.get_thumbnail_subdir(book_id3);

        // Same prefix should result in same subdirectory
        assert_eq!(subdir1, subdir2);
        // Different prefix should result in different subdirectory
        assert_ne!(subdir1, subdir3);

        assert!(subdir1.to_string_lossy().ends_with("books/55"));
        assert!(subdir3.to_string_lossy().ends_with("books/aa"));
    }

    #[test]
    fn test_default_thumbnail_settings() {
        let settings = ThumbnailSettings::default();
        assert_eq!(settings.max_dimension, 400);
        assert_eq!(settings.jpeg_quality, 85);
    }

    #[test]
    fn test_uploads_dir() {
        let service = ThumbnailService::new(test_files_config());
        let uploads_dir = service.get_uploads_dir();
        assert_eq!(uploads_dir.to_string_lossy(), "data/uploads");
    }

    #[tokio::test]
    async fn test_get_thumbnail_metadata_not_found() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let config = FilesConfig {
            thumbnail_dir: temp_dir.path().to_string_lossy().to_string(),
            uploads_dir: "data/uploads".to_string(),
        };
        let service = ThumbnailService::new(config);
        let book_id = Uuid::new_v4();

        // No metadata for non-existent thumbnail
        assert!(service.get_thumbnail_metadata(book_id).await.is_none());
    }

    #[tokio::test]
    async fn test_get_thumbnail_metadata_exists() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let config = FilesConfig {
            thumbnail_dir: temp_dir.path().to_string_lossy().to_string(),
            uploads_dir: "data/uploads".to_string(),
        };
        let service = ThumbnailService::new(config);
        let book_id = Uuid::new_v4();

        // Create a dummy thumbnail
        let thumb_path = service.get_thumbnail_path(book_id);
        fs::create_dir_all(thumb_path.parent().unwrap())
            .await
            .unwrap();
        fs::write(&thumb_path, b"fake thumbnail data")
            .await
            .unwrap();

        // Get metadata
        let meta = service.get_thumbnail_metadata(book_id).await;
        assert!(meta.is_some());

        let meta = meta.unwrap();
        assert_eq!(meta.size, 19); // "fake thumbnail data" = 19 bytes
        assert!(meta.modified_unix > 0);
        assert!(meta.etag.starts_with('"') && meta.etag.ends_with('"'));
    }

    #[tokio::test]
    async fn test_get_thumbnail_stream_not_found() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let config = FilesConfig {
            thumbnail_dir: temp_dir.path().to_string_lossy().to_string(),
            uploads_dir: "data/uploads".to_string(),
        };
        let service = ThumbnailService::new(config);
        let book_id = Uuid::new_v4();

        // No stream for non-existent thumbnail
        assert!(service.get_thumbnail_stream(book_id).await.is_none());
    }

    #[tokio::test]
    async fn test_get_thumbnail_stream_exists() {
        use tokio_stream::StreamExt;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let config = FilesConfig {
            thumbnail_dir: temp_dir.path().to_string_lossy().to_string(),
            uploads_dir: "data/uploads".to_string(),
        };
        let service = ThumbnailService::new(config);
        let book_id = Uuid::new_v4();

        // Create a dummy thumbnail
        let thumb_path = service.get_thumbnail_path(book_id);
        fs::create_dir_all(thumb_path.parent().unwrap())
            .await
            .unwrap();
        let test_data = b"fake thumbnail data for streaming";
        fs::write(&thumb_path, test_data).await.unwrap();

        // Get stream and read data
        let stream = service.get_thumbnail_stream(book_id).await;
        assert!(stream.is_some());

        let mut stream = stream.unwrap();
        let mut collected = Vec::new();
        while let Some(chunk) = stream.next().await {
            collected.extend_from_slice(&chunk.unwrap());
        }
        assert_eq!(collected, test_data);
    }

    #[test]
    fn test_detect_image_format_jpeg() {
        let data = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        assert_eq!(detect_image_format(&data), "JPEG");
    }

    #[test]
    fn test_detect_image_format_png() {
        let data = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(detect_image_format(&data), "PNG");
    }

    #[test]
    fn test_detect_image_format_gif() {
        let data = [0x47, 0x49, 0x46, 0x38, 0x39, 0x61];
        assert_eq!(detect_image_format(&data), "GIF");
    }

    #[test]
    fn test_detect_image_format_webp() {
        let data = [
            0x52, 0x49, 0x46, 0x46, // RIFF
            0x00, 0x00, 0x00, 0x00, // size
            0x57, 0x45, 0x42, 0x50, // WEBP
        ];
        assert_eq!(detect_image_format(&data), "WebP");
    }

    #[test]
    fn test_detect_image_format_bmp() {
        let data = [0x42, 0x4D, 0x00, 0x00];
        assert_eq!(detect_image_format(&data), "BMP");
    }

    #[test]
    fn test_detect_image_format_avif() {
        let data = [
            0x00, 0x00, 0x00, 0x00, // size
            0x66, 0x74, 0x79, 0x70, // ftyp
            0x61, 0x76, 0x69, 0x66, // avif
        ];
        assert_eq!(detect_image_format(&data), "AVIF");
    }

    #[test]
    fn test_detect_image_format_unknown() {
        let data = [0x00, 0x01, 0x02, 0x03];
        assert_eq!(detect_image_format(&data), "unknown");
    }

    #[test]
    fn test_detect_image_format_too_short() {
        let data = [0xFF, 0xD8];
        assert_eq!(detect_image_format(&data), "unknown (too short)");
    }

    #[test]
    fn test_format_magic_bytes() {
        let data = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(format_magic_bytes(&data), "FF D8 FF E0");
    }

    #[test]
    fn test_format_magic_bytes_truncates_at_16() {
        let data: Vec<u8> = (0..20).collect();
        let result = format_magic_bytes(&data);
        // Should only include first 16 bytes
        assert_eq!(result, "00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F");
    }

    #[test]
    fn test_format_magic_bytes_empty() {
        let data: [u8; 0] = [];
        assert_eq!(format_magic_bytes(&data), "");
    }
}
