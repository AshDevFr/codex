//! Repository for BookMetadata operations
//!
//! TODO: Remove allow(dead_code) when all metadata features are fully integrated

#![allow(dead_code)]

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::db::entities::{book_metadata, prelude::*};

/// Repository for BookMetadata operations
pub struct BookMetadataRepository;

impl BookMetadataRepository {
    /// Create or update metadata for a book
    pub async fn upsert(
        db: &DatabaseConnection,
        metadata_model: &book_metadata::Model,
    ) -> Result<book_metadata::Model> {
        let metadata = book_metadata::ActiveModel {
            id: Set(metadata_model.id),
            book_id: Set(metadata_model.book_id),
            // Display fields (moved from books table)
            title: Set(metadata_model.title.clone()),
            title_sort: Set(metadata_model.title_sort.clone()),
            number: Set(metadata_model.number),
            // Content fields
            summary: Set(metadata_model.summary.clone()),
            writer: Set(metadata_model.writer.clone()),
            penciller: Set(metadata_model.penciller.clone()),
            inker: Set(metadata_model.inker.clone()),
            colorist: Set(metadata_model.colorist.clone()),
            letterer: Set(metadata_model.letterer.clone()),
            cover_artist: Set(metadata_model.cover_artist.clone()),
            editor: Set(metadata_model.editor.clone()),
            publisher: Set(metadata_model.publisher.clone()),
            imprint: Set(metadata_model.imprint.clone()),
            genre: Set(metadata_model.genre.clone()),
            web: Set(metadata_model.web.clone()),
            language_iso: Set(metadata_model.language_iso.clone()),
            format_detail: Set(metadata_model.format_detail.clone()),
            black_and_white: Set(metadata_model.black_and_white),
            manga: Set(metadata_model.manga),
            year: Set(metadata_model.year),
            month: Set(metadata_model.month),
            day: Set(metadata_model.day),
            volume: Set(metadata_model.volume),
            count: Set(metadata_model.count),
            isbns: Set(metadata_model.isbns.clone()),
            // Lock fields
            title_lock: Set(metadata_model.title_lock),
            title_sort_lock: Set(metadata_model.title_sort_lock),
            number_lock: Set(metadata_model.number_lock),
            summary_lock: Set(metadata_model.summary_lock),
            writer_lock: Set(metadata_model.writer_lock),
            penciller_lock: Set(metadata_model.penciller_lock),
            inker_lock: Set(metadata_model.inker_lock),
            colorist_lock: Set(metadata_model.colorist_lock),
            letterer_lock: Set(metadata_model.letterer_lock),
            cover_artist_lock: Set(metadata_model.cover_artist_lock),
            editor_lock: Set(metadata_model.editor_lock),
            publisher_lock: Set(metadata_model.publisher_lock),
            imprint_lock: Set(metadata_model.imprint_lock),
            genre_lock: Set(metadata_model.genre_lock),
            web_lock: Set(metadata_model.web_lock),
            language_iso_lock: Set(metadata_model.language_iso_lock),
            format_detail_lock: Set(metadata_model.format_detail_lock),
            black_and_white_lock: Set(metadata_model.black_and_white_lock),
            manga_lock: Set(metadata_model.manga_lock),
            year_lock: Set(metadata_model.year_lock),
            month_lock: Set(metadata_model.month_lock),
            day_lock: Set(metadata_model.day_lock),
            volume_lock: Set(metadata_model.volume_lock),
            count_lock: Set(metadata_model.count_lock),
            isbns_lock: Set(metadata_model.isbns_lock),
            // Timestamps
            created_at: Set(metadata_model.created_at),
            updated_at: Set(Utc::now()),
        };

        // Try to find existing record
        let existing = BookMetadata::find()
            .filter(book_metadata::Column::BookId.eq(metadata_model.book_id))
            .one(db)
            .await
            .context("Failed to check for existing metadata")?;

        let result = if existing.is_some() {
            // Update existing record
            metadata
                .update(db)
                .await
                .context("Failed to update book metadata")?
        } else {
            // Insert new record
            metadata
                .insert(db)
                .await
                .context("Failed to create book metadata")?
        };

        Ok(result)
    }

    /// Get metadata by book ID
    pub async fn get_by_book_id(
        db: &DatabaseConnection,
        book_id: Uuid,
    ) -> Result<Option<book_metadata::Model>> {
        BookMetadata::find()
            .filter(book_metadata::Column::BookId.eq(book_id))
            .one(db)
            .await
            .context("Failed to get metadata by book ID")
    }

    /// Update metadata
    pub async fn update(
        db: &DatabaseConnection,
        metadata_model: &book_metadata::Model,
    ) -> Result<()> {
        let active = book_metadata::ActiveModel {
            id: Set(metadata_model.id),
            book_id: Set(metadata_model.book_id),
            // Display fields (moved from books table)
            title: Set(metadata_model.title.clone()),
            title_sort: Set(metadata_model.title_sort.clone()),
            number: Set(metadata_model.number),
            // Content fields
            summary: Set(metadata_model.summary.clone()),
            writer: Set(metadata_model.writer.clone()),
            penciller: Set(metadata_model.penciller.clone()),
            inker: Set(metadata_model.inker.clone()),
            colorist: Set(metadata_model.colorist.clone()),
            letterer: Set(metadata_model.letterer.clone()),
            cover_artist: Set(metadata_model.cover_artist.clone()),
            editor: Set(metadata_model.editor.clone()),
            publisher: Set(metadata_model.publisher.clone()),
            imprint: Set(metadata_model.imprint.clone()),
            genre: Set(metadata_model.genre.clone()),
            web: Set(metadata_model.web.clone()),
            language_iso: Set(metadata_model.language_iso.clone()),
            format_detail: Set(metadata_model.format_detail.clone()),
            black_and_white: Set(metadata_model.black_and_white),
            manga: Set(metadata_model.manga),
            year: Set(metadata_model.year),
            month: Set(metadata_model.month),
            day: Set(metadata_model.day),
            volume: Set(metadata_model.volume),
            count: Set(metadata_model.count),
            isbns: Set(metadata_model.isbns.clone()),
            // Lock fields
            title_lock: Set(metadata_model.title_lock),
            title_sort_lock: Set(metadata_model.title_sort_lock),
            number_lock: Set(metadata_model.number_lock),
            summary_lock: Set(metadata_model.summary_lock),
            writer_lock: Set(metadata_model.writer_lock),
            penciller_lock: Set(metadata_model.penciller_lock),
            inker_lock: Set(metadata_model.inker_lock),
            colorist_lock: Set(metadata_model.colorist_lock),
            letterer_lock: Set(metadata_model.letterer_lock),
            cover_artist_lock: Set(metadata_model.cover_artist_lock),
            editor_lock: Set(metadata_model.editor_lock),
            publisher_lock: Set(metadata_model.publisher_lock),
            imprint_lock: Set(metadata_model.imprint_lock),
            genre_lock: Set(metadata_model.genre_lock),
            web_lock: Set(metadata_model.web_lock),
            language_iso_lock: Set(metadata_model.language_iso_lock),
            format_detail_lock: Set(metadata_model.format_detail_lock),
            black_and_white_lock: Set(metadata_model.black_and_white_lock),
            manga_lock: Set(metadata_model.manga_lock),
            year_lock: Set(metadata_model.year_lock),
            month_lock: Set(metadata_model.month_lock),
            day_lock: Set(metadata_model.day_lock),
            volume_lock: Set(metadata_model.volume_lock),
            count_lock: Set(metadata_model.count_lock),
            isbns_lock: Set(metadata_model.isbns_lock),
            // Timestamps
            created_at: Set(metadata_model.created_at),
            updated_at: Set(Utc::now()),
        };

        active
            .update(db)
            .await
            .context("Failed to update book metadata")?;

        Ok(())
    }

    /// Delete metadata by book ID
    pub async fn delete_by_book_id(db: &DatabaseConnection, book_id: Uuid) -> Result<()> {
        BookMetadata::delete_many()
            .filter(book_metadata::Column::BookId.eq(book_id))
            .exec(db)
            .await
            .context("Failed to delete metadata by book ID")?;

        Ok(())
    }

    /// Create metadata with just title and number (convenience function for tests)
    pub async fn create_with_title_and_number(
        db: &DatabaseConnection,
        book_id: Uuid,
        title: Option<String>,
        number: Option<sea_orm::prelude::Decimal>,
    ) -> Result<book_metadata::Model> {
        let now = Utc::now();
        let metadata = book_metadata::ActiveModel {
            id: Set(Uuid::new_v4()),
            book_id: Set(book_id),
            title: Set(title),
            title_sort: Set(None),
            number: Set(number),
            summary: Set(None),
            writer: Set(None),
            penciller: Set(None),
            inker: Set(None),
            colorist: Set(None),
            letterer: Set(None),
            cover_artist: Set(None),
            editor: Set(None),
            publisher: Set(None),
            imprint: Set(None),
            genre: Set(None),
            web: Set(None),
            language_iso: Set(None),
            format_detail: Set(None),
            black_and_white: Set(None),
            manga: Set(None),
            year: Set(None),
            month: Set(None),
            day: Set(None),
            volume: Set(None),
            count: Set(None),
            isbns: Set(None),
            title_lock: Set(false),
            title_sort_lock: Set(false),
            number_lock: Set(false),
            summary_lock: Set(false),
            writer_lock: Set(false),
            penciller_lock: Set(false),
            inker_lock: Set(false),
            colorist_lock: Set(false),
            letterer_lock: Set(false),
            cover_artist_lock: Set(false),
            editor_lock: Set(false),
            publisher_lock: Set(false),
            imprint_lock: Set(false),
            genre_lock: Set(false),
            web_lock: Set(false),
            language_iso_lock: Set(false),
            format_detail_lock: Set(false),
            black_and_white_lock: Set(false),
            manga_lock: Set(false),
            year_lock: Set(false),
            month_lock: Set(false),
            day_lock: Set(false),
            volume_lock: Set(false),
            count_lock: Set(false),
            isbns_lock: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
        };

        metadata
            .insert(db)
            .await
            .context("Failed to create book metadata")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::{BookRepository, LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;
    use crate::db::ScanningStrategy;
    use chrono::Utc;

    /// Helper to create a test book
    async fn create_test_book(db: &crate::db::Database) -> crate::db::entities::books::Model {
        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        let book = crate::db::entities::books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
            file_path: "/test/book.cbz".to_string(),
            file_name: "book.cbz".to_string(),
            file_size: 1024,
            file_hash: "test_hash".to_string(),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 10,
            deleted: false,
            analyzed: false,
            analysis_error: None,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
        };

        BookRepository::create(db.sea_orm_connection(), &book, None)
            .await
            .unwrap()
    }

    /// Helper to create a test metadata model with all lock fields set to false
    fn create_metadata_model(book_id: Uuid) -> book_metadata::Model {
        book_metadata::Model {
            id: Uuid::new_v4(),
            book_id,
            // Display fields (moved from books table)
            title: None,
            title_sort: None,
            number: None,
            // Content fields
            summary: None,
            writer: None,
            penciller: None,
            inker: None,
            colorist: None,
            letterer: None,
            cover_artist: None,
            editor: None,
            publisher: None,
            imprint: None,
            genre: None,
            web: None,
            language_iso: None,
            format_detail: None,
            black_and_white: None,
            manga: None,
            year: None,
            month: None,
            day: None,
            volume: None,
            count: None,
            isbns: None,
            // All locks default to false
            title_lock: false,
            title_sort_lock: false,
            number_lock: false,
            summary_lock: false,
            writer_lock: false,
            penciller_lock: false,
            inker_lock: false,
            colorist_lock: false,
            letterer_lock: false,
            cover_artist_lock: false,
            editor_lock: false,
            publisher_lock: false,
            imprint_lock: false,
            genre_lock: false,
            web_lock: false,
            language_iso_lock: false,
            format_detail_lock: false,
            black_and_white_lock: false,
            manga_lock: false,
            year_lock: false,
            month_lock: false,
            day_lock: false,
            volume_lock: false,
            count_lock: false,
            isbns_lock: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_upsert_metadata() {
        let (db, _temp_dir) = create_test_db().await;
        let book = create_test_book(&db).await;

        let mut metadata = create_metadata_model(book.id);
        metadata.summary = Some("Test summary".to_string());
        metadata.writer = Some("Test Writer".to_string());
        metadata.publisher = Some("Test Publisher".to_string());
        metadata.year = Some(2024);

        BookMetadataRepository::upsert(db.sea_orm_connection(), &metadata)
            .await
            .unwrap();

        let retrieved = BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.summary, Some("Test summary".to_string()));
        assert_eq!(retrieved.writer, Some("Test Writer".to_string()));
        assert_eq!(retrieved.year, Some(2024));
    }

    #[tokio::test]
    async fn test_get_metadata_by_book_id() {
        let (db, _temp_dir) = create_test_db().await;
        let book = create_test_book(&db).await;

        let mut metadata = create_metadata_model(book.id);
        metadata.summary = Some("Test summary".to_string());

        BookMetadataRepository::upsert(db.sea_orm_connection(), &metadata)
            .await
            .unwrap();

        let retrieved = BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.book_id, book.id);
        assert_eq!(retrieved.summary, Some("Test summary".to_string()));
    }

    #[tokio::test]
    async fn test_get_metadata_not_found() {
        let (db, _temp_dir) = create_test_db().await;
        let _book = create_test_book(&db).await;

        let result =
            BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), Uuid::new_v4())
                .await
                .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_update_metadata() {
        let (db, _temp_dir) = create_test_db().await;
        let book = create_test_book(&db).await;

        let mut metadata = create_metadata_model(book.id);
        metadata.summary = Some("Original summary".to_string());
        metadata.writer = Some("Original Writer".to_string());

        BookMetadataRepository::upsert(db.sea_orm_connection(), &metadata)
            .await
            .unwrap();

        metadata.summary = Some("Updated summary".to_string());
        metadata.writer = Some("Updated Writer".to_string());

        BookMetadataRepository::update(db.sea_orm_connection(), &metadata)
            .await
            .unwrap();

        let retrieved = BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.summary, Some("Updated summary".to_string()));
        assert_eq!(retrieved.writer, Some("Updated Writer".to_string()));
    }

    #[tokio::test]
    async fn test_upsert_existing_metadata() {
        let (db, _temp_dir) = create_test_db().await;
        let book = create_test_book(&db).await;

        let mut metadata = create_metadata_model(book.id);
        metadata.summary = Some("Original summary".to_string());
        metadata.writer = Some("Original Writer".to_string());

        // First upsert creates the record
        BookMetadataRepository::upsert(db.sea_orm_connection(), &metadata)
            .await
            .unwrap();

        // Second upsert updates the record
        metadata.summary = Some("Updated summary".to_string());
        metadata.writer = Some("Updated Writer".to_string());

        BookMetadataRepository::upsert(db.sea_orm_connection(), &metadata)
            .await
            .unwrap();

        let retrieved = BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.summary, Some("Updated summary".to_string()));
        assert_eq!(retrieved.writer, Some("Updated Writer".to_string()));
    }

    #[tokio::test]
    async fn test_delete_metadata() {
        let (db, _temp_dir) = create_test_db().await;
        let book = create_test_book(&db).await;

        let mut metadata = create_metadata_model(book.id);
        metadata.summary = Some("Test summary".to_string());

        BookMetadataRepository::upsert(db.sea_orm_connection(), &metadata)
            .await
            .unwrap();

        BookMetadataRepository::delete_by_book_id(db.sea_orm_connection(), book.id)
            .await
            .unwrap();

        let result = BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book.id)
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_lock_fields_persistence() {
        let (db, _temp_dir) = create_test_db().await;
        let book = create_test_book(&db).await;

        let mut metadata = create_metadata_model(book.id);
        metadata.summary = Some("Test summary".to_string());
        metadata.summary_lock = true;
        metadata.writer_lock = true;
        metadata.year_lock = true;

        BookMetadataRepository::upsert(db.sea_orm_connection(), &metadata)
            .await
            .unwrap();

        let retrieved = BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book.id)
            .await
            .unwrap()
            .unwrap();

        assert!(retrieved.summary_lock);
        assert!(retrieved.writer_lock);
        assert!(retrieved.year_lock);
        // Others should still be false
        assert!(!retrieved.penciller_lock);
        assert!(!retrieved.publisher_lock);
    }
}
