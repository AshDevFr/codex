//! Repository for book_external_links table operations

#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::db::entities::book_external_links::{
    self, Entity as ExternalLinks, Model as ExternalLink,
};

/// Repository for book external link operations
pub struct BookExternalLinkRepository;

impl BookExternalLinkRepository {
    /// Get an external link by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<ExternalLink>> {
        let result = ExternalLinks::find_by_id(id).one(db).await?;
        Ok(result)
    }

    /// Get all external links for a book
    pub async fn get_for_book(db: &DatabaseConnection, book_id: Uuid) -> Result<Vec<ExternalLink>> {
        let results = ExternalLinks::find()
            .filter(book_external_links::Column::BookId.eq(book_id))
            .all(db)
            .await?;
        Ok(results)
    }

    /// Get an external link by book ID and source name
    pub async fn get_by_source(
        db: &DatabaseConnection,
        book_id: Uuid,
        source_name: &str,
    ) -> Result<Option<ExternalLink>> {
        let normalized = source_name.to_lowercase().trim().to_string();
        let result = ExternalLinks::find()
            .filter(book_external_links::Column::BookId.eq(book_id))
            .filter(book_external_links::Column::SourceName.eq(&normalized))
            .one(db)
            .await?;
        Ok(result)
    }

    /// Create a new external link for a book
    pub async fn create(
        db: &DatabaseConnection,
        book_id: Uuid,
        source_name: &str,
        url: &str,
        external_id: Option<&str>,
    ) -> Result<ExternalLink> {
        let now = Utc::now();
        let normalized_source = source_name.to_lowercase().trim().to_string();

        let active_model = book_external_links::ActiveModel {
            id: Set(Uuid::new_v4()),
            book_id: Set(book_id),
            source_name: Set(normalized_source),
            url: Set(url.trim().to_string()),
            external_id: Set(external_id.map(|s| s.trim().to_string())),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let model = active_model.insert(db).await?;
        Ok(model)
    }

    /// Create or update an external link (upsert by book_id + source_name)
    pub async fn upsert(
        db: &DatabaseConnection,
        book_id: Uuid,
        source_name: &str,
        url: &str,
        external_id: Option<&str>,
    ) -> Result<ExternalLink> {
        let existing = Self::get_by_source(db, book_id, source_name).await?;

        match existing {
            Some(existing) => {
                let mut active_model: book_external_links::ActiveModel = existing.into();
                active_model.url = Set(url.trim().to_string());
                active_model.external_id = Set(external_id.map(|s| s.trim().to_string()));
                active_model.updated_at = Set(Utc::now());

                let model = active_model.update(db).await?;
                Ok(model)
            }
            None => Self::create(db, book_id, source_name, url, external_id).await,
        }
    }

    /// Update an external link by ID
    pub async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        url: Option<&str>,
        external_id: Option<Option<&str>>,
    ) -> Result<Option<ExternalLink>> {
        let existing = ExternalLinks::find_by_id(id).one(db).await?;

        let Some(existing) = existing else {
            return Ok(None);
        };

        let mut active_model: book_external_links::ActiveModel = existing.into();
        active_model.updated_at = Set(Utc::now());

        if let Some(url) = url {
            active_model.url = Set(url.trim().to_string());
        }

        if let Some(external_id) = external_id {
            active_model.external_id = Set(external_id.map(|s| s.trim().to_string()));
        }

        let model = active_model.update(db).await?;
        Ok(Some(model))
    }

    /// Delete an external link by ID
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = ExternalLinks::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Delete an external link by book ID and source name
    pub async fn delete_by_source(
        db: &DatabaseConnection,
        book_id: Uuid,
        source_name: &str,
    ) -> Result<bool> {
        let normalized = source_name.to_lowercase().trim().to_string();
        let result = ExternalLinks::delete_many()
            .filter(book_external_links::Column::BookId.eq(book_id))
            .filter(book_external_links::Column::SourceName.eq(&normalized))
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// Delete all external links for a book
    pub async fn delete_all_for_book(db: &DatabaseConnection, book_id: Uuid) -> Result<u64> {
        let result = ExternalLinks::delete_many()
            .filter(book_external_links::Column::BookId.eq(book_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Check if an external link belongs to a specific book
    pub async fn belongs_to_book(db: &DatabaseConnection, id: Uuid, book_id: Uuid) -> Result<bool> {
        let result = ExternalLinks::find_by_id(id)
            .filter(book_external_links::Column::BookId.eq(book_id))
            .one(db)
            .await?;
        Ok(result.is_some())
    }

    /// Get external links for multiple books by their IDs
    ///
    /// Returns a HashMap keyed by book_id for efficient lookups
    pub async fn get_for_book_ids(
        db: &DatabaseConnection,
        book_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<ExternalLink>>> {
        if book_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let results = ExternalLinks::find()
            .filter(book_external_links::Column::BookId.is_in(book_ids.to_vec()))
            .all(db)
            .await?;

        let mut map: std::collections::HashMap<Uuid, Vec<ExternalLink>> =
            std::collections::HashMap::new();

        for link in results {
            map.entry(link.book_id).or_default().push(link);
        }

        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::books;
    use crate::db::repositories::{BookRepository, LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;
    use crate::db::ScanningStrategy;
    use chrono::Utc;

    async fn create_test_book(db: &DatabaseConnection) -> books::Model {
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
            file_path: "/test/path/book.cbz".to_string(),
            file_name: "book.cbz".to_string(),
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

        BookRepository::create(db, &book_model, None).await.unwrap()
    }

    #[tokio::test]
    async fn test_create_and_get_external_link() {
        let (db, _temp_dir) = create_test_db().await;
        let book = create_test_book(db.sea_orm_connection()).await;

        let link = BookExternalLinkRepository::create(
            db.sea_orm_connection(),
            book.id,
            "openlibrary",
            "https://openlibrary.org/works/OL123W",
            Some("OL123W"),
        )
        .await
        .unwrap();

        assert_eq!(link.source_name, "openlibrary");
        assert_eq!(link.url, "https://openlibrary.org/works/OL123W");
        assert_eq!(link.external_id, Some("OL123W".to_string()));
        assert_eq!(link.book_id, book.id);

        let fetched = BookExternalLinkRepository::get_by_id(db.sea_orm_connection(), link.id)
            .await
            .unwrap();
        assert!(fetched.is_some());
    }

    #[tokio::test]
    async fn test_get_for_book() {
        let (db, _temp_dir) = create_test_db().await;
        let book = create_test_book(db.sea_orm_connection()).await;

        BookExternalLinkRepository::create(
            db.sea_orm_connection(),
            book.id,
            "openlibrary",
            "https://openlibrary.org/1",
            Some("1"),
        )
        .await
        .unwrap();

        BookExternalLinkRepository::create(
            db.sea_orm_connection(),
            book.id,
            "goodreads",
            "https://goodreads.com/2",
            Some("2"),
        )
        .await
        .unwrap();

        BookExternalLinkRepository::create(
            db.sea_orm_connection(),
            book.id,
            "amazon",
            "https://amazon.com/3",
            None,
        )
        .await
        .unwrap();

        let links = BookExternalLinkRepository::get_for_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();

        assert_eq!(links.len(), 3);
    }

    #[tokio::test]
    async fn test_get_by_source() {
        let (db, _temp_dir) = create_test_db().await;
        let book = create_test_book(db.sea_orm_connection()).await;

        BookExternalLinkRepository::create(
            db.sea_orm_connection(),
            book.id,
            "openlibrary",
            "https://openlibrary.org/1",
            Some("1"),
        )
        .await
        .unwrap();

        let link = BookExternalLinkRepository::get_by_source(
            db.sea_orm_connection(),
            book.id,
            "OpenLibrary",
        )
        .await
        .unwrap();

        assert!(link.is_some());
        assert_eq!(link.unwrap().source_name, "openlibrary");

        let not_found = BookExternalLinkRepository::get_by_source(
            db.sea_orm_connection(),
            book.id,
            "goodreads",
        )
        .await
        .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_upsert_external_link() {
        let (db, _temp_dir) = create_test_db().await;
        let book = create_test_book(db.sea_orm_connection()).await;

        // First upsert creates
        let link1 = BookExternalLinkRepository::upsert(
            db.sea_orm_connection(),
            book.id,
            "goodreads",
            "https://goodreads.com/old",
            Some("old-id"),
        )
        .await
        .unwrap();

        assert_eq!(link1.url, "https://goodreads.com/old");

        // Second upsert updates
        let link2 = BookExternalLinkRepository::upsert(
            db.sea_orm_connection(),
            book.id,
            "goodreads",
            "https://goodreads.com/new",
            Some("new-id"),
        )
        .await
        .unwrap();

        assert_eq!(link1.id, link2.id);
        assert_eq!(link2.url, "https://goodreads.com/new");
        assert_eq!(link2.external_id, Some("new-id".to_string()));

        // Verify only one link exists
        let links = BookExternalLinkRepository::get_for_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();
        assert_eq!(links.len(), 1);
    }

    #[tokio::test]
    async fn test_update_external_link() {
        let (db, _temp_dir) = create_test_db().await;
        let book = create_test_book(db.sea_orm_connection()).await;

        let link = BookExternalLinkRepository::create(
            db.sea_orm_connection(),
            book.id,
            "goodreads",
            "https://old.url",
            Some("old-id"),
        )
        .await
        .unwrap();

        // Update URL only
        let updated = BookExternalLinkRepository::update(
            db.sea_orm_connection(),
            link.id,
            Some("https://new.url"),
            None,
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(updated.url, "https://new.url");
        assert_eq!(updated.external_id, Some("old-id".to_string()));

        // Update external_id only
        let updated = BookExternalLinkRepository::update(
            db.sea_orm_connection(),
            link.id,
            None,
            Some(Some("new-id")),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(updated.url, "https://new.url");
        assert_eq!(updated.external_id, Some("new-id".to_string()));

        // Set external_id to None
        let updated =
            BookExternalLinkRepository::update(db.sea_orm_connection(), link.id, None, Some(None))
                .await
                .unwrap()
                .unwrap();

        assert_eq!(updated.external_id, None);
    }

    #[tokio::test]
    async fn test_delete_external_link() {
        let (db, _temp_dir) = create_test_db().await;
        let book = create_test_book(db.sea_orm_connection()).await;

        let link = BookExternalLinkRepository::create(
            db.sea_orm_connection(),
            book.id,
            "goodreads",
            "https://goodreads.com/1",
            None,
        )
        .await
        .unwrap();

        let deleted = BookExternalLinkRepository::delete(db.sea_orm_connection(), link.id)
            .await
            .unwrap();
        assert!(deleted);

        let fetched = BookExternalLinkRepository::get_by_id(db.sea_orm_connection(), link.id)
            .await
            .unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_delete_by_source() {
        let (db, _temp_dir) = create_test_db().await;
        let book = create_test_book(db.sea_orm_connection()).await;

        BookExternalLinkRepository::create(
            db.sea_orm_connection(),
            book.id,
            "goodreads",
            "https://goodreads.com/1",
            None,
        )
        .await
        .unwrap();

        let deleted = BookExternalLinkRepository::delete_by_source(
            db.sea_orm_connection(),
            book.id,
            "Goodreads",
        )
        .await
        .unwrap();
        assert!(deleted);

        let fetched = BookExternalLinkRepository::get_by_source(
            db.sea_orm_connection(),
            book.id,
            "goodreads",
        )
        .await
        .unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_delete_all_for_book() {
        let (db, _temp_dir) = create_test_db().await;
        let book = create_test_book(db.sea_orm_connection()).await;

        for source in ["openlibrary", "goodreads", "amazon"] {
            BookExternalLinkRepository::create(
                db.sea_orm_connection(),
                book.id,
                source,
                &format!("https://{}.com", source),
                None,
            )
            .await
            .unwrap();
        }

        let count =
            BookExternalLinkRepository::delete_all_for_book(db.sea_orm_connection(), book.id)
                .await
                .unwrap();

        assert_eq!(count, 3);

        let remaining = BookExternalLinkRepository::get_for_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn test_source_name_normalization() {
        let (db, _temp_dir) = create_test_db().await;
        let book = create_test_book(db.sea_orm_connection()).await;

        let link = BookExternalLinkRepository::create(
            db.sea_orm_connection(),
            book.id,
            "  OpenLibrary  ",
            "https://openlibrary.org/1",
            None,
        )
        .await
        .unwrap();

        assert_eq!(link.source_name, "openlibrary");

        // Should find with different case
        let found = BookExternalLinkRepository::get_by_source(
            db.sea_orm_connection(),
            book.id,
            "OPENLIBRARY",
        )
        .await
        .unwrap();
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn test_url_and_external_id_trimming() {
        let (db, _temp_dir) = create_test_db().await;
        let book = create_test_book(db.sea_orm_connection()).await;

        let link = BookExternalLinkRepository::create(
            db.sea_orm_connection(),
            book.id,
            "openlibrary",
            "  https://openlibrary.org/1  ",
            Some("  OL123W  "),
        )
        .await
        .unwrap();

        assert_eq!(link.url, "https://openlibrary.org/1");
        assert_eq!(link.external_id, Some("OL123W".to_string()));
    }

    #[tokio::test]
    async fn test_belongs_to_book() {
        let (db, _temp_dir) = create_test_db().await;
        let db = db.sea_orm_connection();

        let library =
            LibraryRepository::create(db, "Test Library", "/test/path", ScanningStrategy::Default)
                .await
                .unwrap();

        let series = SeriesRepository::create(db, library.id, "Test Series", None)
            .await
            .unwrap();

        let book1_model = books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
            file_path: "/test/path/b1.cbz".to_string(),
            file_name: "b1.cbz".to_string(),
            file_size: 1024,
            file_hash: "hash1".to_string(),
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

        let book2_model = books::Model {
            id: Uuid::new_v4(),
            file_path: "/test/path/b2.cbz".to_string(),
            file_name: "b2.cbz".to_string(),
            file_hash: "hash2".to_string(),
            ..book1_model.clone()
        };

        let book1 = BookRepository::create(db, &book1_model, None)
            .await
            .unwrap();
        let book2 = BookRepository::create(db, &book2_model, None)
            .await
            .unwrap();

        let link = BookExternalLinkRepository::create(
            db,
            book1.id,
            "openlibrary",
            "https://openlibrary.org/1",
            None,
        )
        .await
        .unwrap();

        let belongs = BookExternalLinkRepository::belongs_to_book(db, link.id, book1.id)
            .await
            .unwrap();
        assert!(belongs);

        let belongs = BookExternalLinkRepository::belongs_to_book(db, link.id, book2.id)
            .await
            .unwrap();
        assert!(!belongs);
    }
}
