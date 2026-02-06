//! Repository for book_external_ids table operations
//!
//! Provides CRUD operations for managing external provider IDs for books.
//! Used to track which external source a book was matched from and enable
//! efficient re-fetching without search.
//!
//! Mirrors the series_external_id repository pattern.

#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::db::entities::book_external_ids::{
    self, Entity as BookExternalIds, Model as BookExternalId,
};

/// Repository for book external ID operations
pub struct BookExternalIdRepository;

impl BookExternalIdRepository {
    /// Get an external ID record by its primary key
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<BookExternalId>> {
        let result = BookExternalIds::find_by_id(id).one(db).await?;
        Ok(result)
    }

    /// Get all external IDs for a book
    pub async fn get_for_book(
        db: &DatabaseConnection,
        book_id: Uuid,
    ) -> Result<Vec<BookExternalId>> {
        let results = BookExternalIds::find()
            .filter(book_external_ids::Column::BookId.eq(book_id))
            .all(db)
            .await?;
        Ok(results)
    }

    /// Get an external ID by book ID and source
    pub async fn get_by_source(
        db: &DatabaseConnection,
        book_id: Uuid,
        source: &str,
    ) -> Result<Option<BookExternalId>> {
        let result = BookExternalIds::find()
            .filter(book_external_ids::Column::BookId.eq(book_id))
            .filter(book_external_ids::Column::Source.eq(source))
            .one(db)
            .await?;
        Ok(result)
    }

    /// Get an external ID for a book from a specific plugin
    pub async fn get_for_plugin(
        db: &DatabaseConnection,
        book_id: Uuid,
        plugin_name: &str,
    ) -> Result<Option<BookExternalId>> {
        let source = book_external_ids::Model::plugin_source(plugin_name);
        Self::get_by_source(db, book_id, &source).await
    }

    /// Create a new external ID record
    pub async fn create(
        db: &DatabaseConnection,
        book_id: Uuid,
        source: &str,
        external_id: &str,
        external_url: Option<&str>,
        metadata_hash: Option<&str>,
    ) -> Result<BookExternalId> {
        let now = Utc::now();

        let active_model = book_external_ids::ActiveModel {
            id: Set(Uuid::new_v4()),
            book_id: Set(book_id),
            source: Set(source.to_string()),
            external_id: Set(external_id.to_string()),
            external_url: Set(external_url.map(|s| s.to_string())),
            metadata_hash: Set(metadata_hash.map(|s| s.to_string())),
            last_synced_at: Set(Some(now)),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let model = active_model.insert(db).await?;
        Ok(model)
    }

    /// Create an external ID record for a plugin source
    pub async fn create_for_plugin(
        db: &DatabaseConnection,
        book_id: Uuid,
        plugin_name: &str,
        external_id: &str,
        external_url: Option<&str>,
        metadata_hash: Option<&str>,
    ) -> Result<BookExternalId> {
        let source = book_external_ids::Model::plugin_source(plugin_name);
        Self::create(
            db,
            book_id,
            &source,
            external_id,
            external_url,
            metadata_hash,
        )
        .await
    }

    /// Create an external ID from EPUB metadata
    pub async fn create_from_epub(
        db: &DatabaseConnection,
        book_id: Uuid,
        external_id: &str,
        external_url: Option<&str>,
    ) -> Result<BookExternalId> {
        Self::create(db, book_id, "epub", external_id, external_url, None).await
    }

    /// Create an external ID from PDF metadata
    pub async fn create_from_pdf(
        db: &DatabaseConnection,
        book_id: Uuid,
        external_id: &str,
        external_url: Option<&str>,
    ) -> Result<BookExternalId> {
        Self::create(db, book_id, "pdf", external_id, external_url, None).await
    }

    /// Create or update an external ID record (upsert by book_id + source)
    pub async fn upsert(
        db: &DatabaseConnection,
        book_id: Uuid,
        source: &str,
        external_id: &str,
        external_url: Option<&str>,
        metadata_hash: Option<&str>,
    ) -> Result<BookExternalId> {
        let existing = Self::get_by_source(db, book_id, source).await?;

        match existing {
            Some(existing) => {
                let now = Utc::now();
                let mut active_model: book_external_ids::ActiveModel = existing.into();
                active_model.external_id = Set(external_id.to_string());
                active_model.external_url = Set(external_url.map(|s| s.to_string()));
                active_model.metadata_hash = Set(metadata_hash.map(|s| s.to_string()));
                active_model.last_synced_at = Set(Some(now));
                active_model.updated_at = Set(now);

                let model = active_model.update(db).await?;
                Ok(model)
            }
            None => {
                Self::create(
                    db,
                    book_id,
                    source,
                    external_id,
                    external_url,
                    metadata_hash,
                )
                .await
            }
        }
    }

    /// Upsert an external ID for a plugin source
    pub async fn upsert_for_plugin(
        db: &DatabaseConnection,
        book_id: Uuid,
        plugin_name: &str,
        external_id: &str,
        external_url: Option<&str>,
        metadata_hash: Option<&str>,
    ) -> Result<BookExternalId> {
        let source = book_external_ids::Model::plugin_source(plugin_name);
        Self::upsert(
            db,
            book_id,
            &source,
            external_id,
            external_url,
            metadata_hash,
        )
        .await
    }

    /// Update the metadata hash and last synced timestamp
    pub async fn update_sync_info(
        db: &DatabaseConnection,
        id: Uuid,
        metadata_hash: Option<&str>,
    ) -> Result<Option<BookExternalId>> {
        let existing = BookExternalIds::find_by_id(id).one(db).await?;

        let Some(existing) = existing else {
            return Ok(None);
        };

        let now = Utc::now();
        let mut active_model: book_external_ids::ActiveModel = existing.into();
        active_model.metadata_hash = Set(metadata_hash.map(|s| s.to_string()));
        active_model.last_synced_at = Set(Some(now));
        active_model.updated_at = Set(now);

        let model = active_model.update(db).await?;
        Ok(Some(model))
    }

    /// Delete an external ID record by ID
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = BookExternalIds::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Delete an external ID by book ID and source
    pub async fn delete_by_source(
        db: &DatabaseConnection,
        book_id: Uuid,
        source: &str,
    ) -> Result<bool> {
        let result = BookExternalIds::delete_many()
            .filter(book_external_ids::Column::BookId.eq(book_id))
            .filter(book_external_ids::Column::Source.eq(source))
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// Delete all external IDs for a book
    pub async fn delete_all_for_book(db: &DatabaseConnection, book_id: Uuid) -> Result<u64> {
        let result = BookExternalIds::delete_many()
            .filter(book_external_ids::Column::BookId.eq(book_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Get external IDs for multiple books by their IDs
    ///
    /// Returns a HashMap keyed by book_id for efficient lookups
    pub async fn get_for_book_ids(
        db: &DatabaseConnection,
        book_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<BookExternalId>>> {
        if book_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let results = BookExternalIds::find()
            .filter(book_external_ids::Column::BookId.is_in(book_ids.to_vec()))
            .all(db)
            .await?;

        let mut map: HashMap<Uuid, Vec<BookExternalId>> = HashMap::new();

        for external_id in results {
            map.entry(external_id.book_id)
                .or_default()
                .push(external_id);
        }

        Ok(map)
    }

    /// Check if an external ID record belongs to a specific book
    pub async fn belongs_to_book(db: &DatabaseConnection, id: Uuid, book_id: Uuid) -> Result<bool> {
        let result = BookExternalIds::find_by_id(id)
            .filter(book_external_ids::Column::BookId.eq(book_id))
            .one(db)
            .await?;
        Ok(result.is_some())
    }

    /// Count external IDs for a book
    pub async fn count_for_book(db: &DatabaseConnection, book_id: Uuid) -> Result<u64> {
        let count = BookExternalIds::find()
            .filter(book_external_ids::Column::BookId.eq(book_id))
            .count(db)
            .await?;
        Ok(count)
    }

    /// Find all books with a specific external source
    pub async fn find_by_source(
        db: &DatabaseConnection,
        source: &str,
    ) -> Result<Vec<BookExternalId>> {
        let results = BookExternalIds::find()
            .filter(book_external_ids::Column::Source.eq(source))
            .all(db)
            .await?;
        Ok(results)
    }

    /// Find book by external ID value
    ///
    /// Useful for reverse lookups (e.g., "find book with ISBN X")
    pub async fn find_by_external_id(
        db: &DatabaseConnection,
        external_id: &str,
    ) -> Result<Vec<BookExternalId>> {
        let results = BookExternalIds::find()
            .filter(book_external_ids::Column::ExternalId.eq(external_id))
            .all(db)
            .await?;
        Ok(results)
    }

    /// Find book by external ID value and source
    pub async fn find_by_external_id_and_source(
        db: &DatabaseConnection,
        external_id: &str,
        source: &str,
    ) -> Result<Option<BookExternalId>> {
        let result = BookExternalIds::find()
            .filter(book_external_ids::Column::ExternalId.eq(external_id))
            .filter(book_external_ids::Column::Source.eq(source))
            .one(db)
            .await?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::entities::books;
    use crate::db::repositories::{BookRepository, LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;
    use chrono::Utc;

    async fn setup_test_book(db: &DatabaseConnection) -> (Uuid, Uuid) {
        let library =
            LibraryRepository::create(db, "Test Library", "/test/path", ScanningStrategy::Default)
                .await
                .unwrap();

        let series = SeriesRepository::create(db, library.id, "Test Series", None)
            .await
            .unwrap();

        let book_model = books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
            file_path: "/test/path/test.cbz".to_string(),
            file_name: "test.cbz".to_string(),
            file_size: 1024,
            file_hash: "test_hash".to_string(),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 10,
            deleted: false,
            analyzed: false,
            analysis_error: None,
            analysis_errors: None,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
        };

        let book = BookRepository::create(db, &book_model, None).await.unwrap();

        (series.id, book.id)
    }

    #[tokio::test]
    async fn test_create_and_get_external_id() {
        let (db, _temp_dir) = create_test_db().await;
        let (_, book_id) = setup_test_book(db.sea_orm_connection()).await;

        let external = BookExternalIdRepository::create(
            db.sea_orm_connection(),
            book_id,
            "plugin:openlibrary",
            "OL12345M",
            Some("https://openlibrary.org/books/OL12345M"),
            Some("abc123hash"),
        )
        .await
        .unwrap();

        assert_eq!(external.source, "plugin:openlibrary");
        assert_eq!(external.external_id, "OL12345M");
        assert_eq!(
            external.external_url,
            Some("https://openlibrary.org/books/OL12345M".to_string())
        );
        assert_eq!(external.metadata_hash, Some("abc123hash".to_string()));
        assert!(external.last_synced_at.is_some());
        assert_eq!(external.book_id, book_id);

        let fetched = BookExternalIdRepository::get_by_id(db.sea_orm_connection(), external.id)
            .await
            .unwrap();
        assert!(fetched.is_some());
    }

    #[tokio::test]
    async fn test_create_for_plugin() {
        let (db, _temp_dir) = create_test_db().await;
        let (_, book_id) = setup_test_book(db.sea_orm_connection()).await;

        let external = BookExternalIdRepository::create_for_plugin(
            db.sea_orm_connection(),
            book_id,
            "openlibrary",
            "OL12345M",
            None,
            None,
        )
        .await
        .unwrap();

        assert_eq!(external.source, "plugin:openlibrary");
        assert!(external.is_plugin_source());
        assert_eq!(external.plugin_name(), Some("openlibrary"));
    }

    #[tokio::test]
    async fn test_create_from_epub() {
        let (db, _temp_dir) = create_test_db().await;
        let (_, book_id) = setup_test_book(db.sea_orm_connection()).await;

        let external = BookExternalIdRepository::create_from_epub(
            db.sea_orm_connection(),
            book_id,
            "978-0-123456-78-9",
            None,
        )
        .await
        .unwrap();

        assert_eq!(external.source, "epub");
        assert!(external.is_epub_source());
        assert_eq!(external.external_id, "978-0-123456-78-9");
    }

    #[tokio::test]
    async fn test_get_for_book() {
        let (db, _temp_dir) = create_test_db().await;
        let (_, book_id) = setup_test_book(db.sea_orm_connection()).await;

        BookExternalIdRepository::create(
            db.sea_orm_connection(),
            book_id,
            "plugin:openlibrary",
            "1",
            None,
            None,
        )
        .await
        .unwrap();

        BookExternalIdRepository::create(db.sea_orm_connection(), book_id, "epub", "2", None, None)
            .await
            .unwrap();

        BookExternalIdRepository::create(
            db.sea_orm_connection(),
            book_id,
            "manual",
            "3",
            None,
            None,
        )
        .await
        .unwrap();

        let externals = BookExternalIdRepository::get_for_book(db.sea_orm_connection(), book_id)
            .await
            .unwrap();

        assert_eq!(externals.len(), 3);
    }

    #[tokio::test]
    async fn test_get_by_source() {
        let (db, _temp_dir) = create_test_db().await;
        let (_, book_id) = setup_test_book(db.sea_orm_connection()).await;

        BookExternalIdRepository::create(
            db.sea_orm_connection(),
            book_id,
            "plugin:openlibrary",
            "OL12345M",
            None,
            None,
        )
        .await
        .unwrap();

        let found = BookExternalIdRepository::get_by_source(
            db.sea_orm_connection(),
            book_id,
            "plugin:openlibrary",
        )
        .await
        .unwrap();

        assert!(found.is_some());
        assert_eq!(found.unwrap().external_id, "OL12345M");

        let not_found = BookExternalIdRepository::get_by_source(
            db.sea_orm_connection(),
            book_id,
            "plugin:other",
        )
        .await
        .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_upsert_external_id() {
        let (db, _temp_dir) = create_test_db().await;
        let (_, book_id) = setup_test_book(db.sea_orm_connection()).await;

        // First upsert creates
        let external1 = BookExternalIdRepository::upsert(
            db.sea_orm_connection(),
            book_id,
            "plugin:openlibrary",
            "old-id",
            Some("https://old.url"),
            Some("old-hash"),
        )
        .await
        .unwrap();

        assert_eq!(external1.external_id, "old-id");

        // Second upsert updates
        let external2 = BookExternalIdRepository::upsert(
            db.sea_orm_connection(),
            book_id,
            "plugin:openlibrary",
            "new-id",
            Some("https://new.url"),
            Some("new-hash"),
        )
        .await
        .unwrap();

        assert_eq!(external1.id, external2.id);
        assert_eq!(external2.external_id, "new-id");
        assert_eq!(external2.external_url, Some("https://new.url".to_string()));
        assert_eq!(external2.metadata_hash, Some("new-hash".to_string()));

        // Verify only one record exists
        let externals = BookExternalIdRepository::get_for_book(db.sea_orm_connection(), book_id)
            .await
            .unwrap();
        assert_eq!(externals.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_external_id() {
        let (db, _temp_dir) = create_test_db().await;
        let (_, book_id) = setup_test_book(db.sea_orm_connection()).await;

        let external = BookExternalIdRepository::create(
            db.sea_orm_connection(),
            book_id,
            "plugin:openlibrary",
            "12345",
            None,
            None,
        )
        .await
        .unwrap();

        let deleted = BookExternalIdRepository::delete(db.sea_orm_connection(), external.id)
            .await
            .unwrap();
        assert!(deleted);

        let fetched = BookExternalIdRepository::get_by_id(db.sea_orm_connection(), external.id)
            .await
            .unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_find_by_external_id() {
        let (db, _temp_dir) = create_test_db().await;
        let (_, book_id) = setup_test_book(db.sea_orm_connection()).await;

        BookExternalIdRepository::create(
            db.sea_orm_connection(),
            book_id,
            "epub",
            "978-0-123456-78-9",
            None,
            None,
        )
        .await
        .unwrap();

        let found = BookExternalIdRepository::find_by_external_id(
            db.sea_orm_connection(),
            "978-0-123456-78-9",
        )
        .await
        .unwrap();

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].book_id, book_id);
    }

    #[tokio::test]
    async fn test_count_for_book() {
        let (db, _temp_dir) = create_test_db().await;
        let (_, book_id) = setup_test_book(db.sea_orm_connection()).await;

        let count = BookExternalIdRepository::count_for_book(db.sea_orm_connection(), book_id)
            .await
            .unwrap();
        assert_eq!(count, 0);

        BookExternalIdRepository::create(
            db.sea_orm_connection(),
            book_id,
            "plugin:openlibrary",
            "1",
            None,
            None,
        )
        .await
        .unwrap();

        BookExternalIdRepository::create(db.sea_orm_connection(), book_id, "epub", "2", None, None)
            .await
            .unwrap();

        let count = BookExternalIdRepository::count_for_book(db.sea_orm_connection(), book_id)
            .await
            .unwrap();
        assert_eq!(count, 2);
    }
}
