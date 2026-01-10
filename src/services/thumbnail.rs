use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use image::{imageops::FilterType, ImageFormat};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::config::ThumbnailConfig;
use crate::db::entities::{books, prelude::*};
use crate::db::repositories::{BookRepository, SettingsRepository};

/// Service for managing thumbnail cache
pub struct ThumbnailService {
    config: ThumbnailConfig,
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
    pub fn new(config: ThumbnailConfig) -> Self {
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
        PathBuf::from(&self.config.data_dir).join(&self.config.cache_dir)
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
            book.id,
            book.title.as_ref().unwrap_or(&book.file_name)
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
        let mut book_active: books::ActiveModel = book.clone().into();
        book_active.thumbnail_path = Set(Some(thumbnail_path.to_string_lossy().to_string()));
        book_active.thumbnail_generated_at = Set(Some(Utc::now()));
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

        let mut book_active: books::ActiveModel = book.into();
        book_active.thumbnail_path = Set(Some(thumbnail_path.to_string_lossy().to_string()));
        book_active.thumbnail_generated_at = Set(Some(Utc::now()));
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
        // Load image from bytes
        let img =
            image::load_from_memory(image_data).context("Failed to load image from memory")?;

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

    #[test]
    fn test_thumbnail_path_generation() {
        let config = ThumbnailConfig {
            data_dir: "data".to_string(),
            cache_dir: "thumbnails".to_string(),
        };
        let service = ThumbnailService::new(config);

        let book_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let path = service.get_thumbnail_path(book_id);

        assert!(path.to_string_lossy().contains("data/thumbnails/books/55"));
        assert!(path
            .to_string_lossy()
            .ends_with("550e8400-e29b-41d4-a716-446655440000.jpg"));
    }

    #[test]
    fn test_thumbnail_subdirectory_bucketing() {
        let config = ThumbnailConfig {
            data_dir: "data".to_string(),
            cache_dir: "thumbnails".to_string(),
        };
        let service = ThumbnailService::new(config);

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
}
