use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::db::entities::{book_metadata_records, prelude::*};

/// Repository for BookMetadataRecord operations
pub struct BookMetadataRepository;

impl BookMetadataRepository {
    /// Create or update metadata for a book
    pub async fn upsert(
        db: &DatabaseConnection,
        metadata_model: &book_metadata_records::Model,
    ) -> Result<book_metadata_records::Model> {
        let metadata = book_metadata_records::ActiveModel {
            id: Set(metadata_model.id),
            book_id: Set(metadata_model.book_id),
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
            created_at: Set(metadata_model.created_at),
            updated_at: Set(Utc::now()),
        };

        // Try to find existing record
        let existing = BookMetadataRecords::find()
            .filter(book_metadata_records::Column::BookId.eq(metadata_model.book_id))
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
    ) -> Result<Option<book_metadata_records::Model>> {
        BookMetadataRecords::find()
            .filter(book_metadata_records::Column::BookId.eq(book_id))
            .one(db)
            .await
            .context("Failed to get metadata by book ID")
    }

    /// Update metadata
    pub async fn update(
        db: &DatabaseConnection,
        metadata_model: &book_metadata_records::Model,
    ) -> Result<()> {
        let active = book_metadata_records::ActiveModel {
            id: Set(metadata_model.id),
            book_id: Set(metadata_model.book_id),
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
        BookMetadataRecords::delete_many()
            .filter(book_metadata_records::Column::BookId.eq(book_id))
            .exec(db)
            .await
            .context("Failed to delete metadata by book ID")?;

        Ok(())
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

        let series = SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series")
            .await
            .unwrap();

        let book = crate::db::entities::books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            title: None,
            number: None,
            file_path: "/test/book.cbz".to_string(),
            file_name: "book.cbz".to_string(),
            file_size: 1024,
            file_hash: "test_hash".to_string(),
            format: "cbz".to_string(),
            page_count: 10,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        BookRepository::create(db.sea_orm_connection(), &book)
            .await
            .unwrap()
    }

    /// Helper to create a test metadata model
    fn create_metadata_model(book_id: Uuid) -> book_metadata_records::Model {
        book_metadata_records::Model {
            id: Uuid::new_v4(),
            book_id,
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
}
