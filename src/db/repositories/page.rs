//! Repository for Page operations
//!
//! TODO: Remove allow(dead_code) when all page features are fully integrated

#![allow(dead_code)]

use anyhow::{Context, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::db::entities::{pages, prelude::*};

/// Repository for Page operations
pub struct PageRepository;

impl PageRepository {
    /// Create a new page
    pub async fn create(
        db: &DatabaseConnection,
        page_model: &pages::Model,
    ) -> Result<pages::Model> {
        let page = pages::ActiveModel {
            id: Set(page_model.id),
            book_id: Set(page_model.book_id),
            page_number: Set(page_model.page_number),
            file_name: Set(page_model.file_name.clone()),
            format: Set(page_model.format.clone()),
            width: Set(page_model.width),
            height: Set(page_model.height),
            file_size: Set(page_model.file_size),
            created_at: Set(page_model.created_at),
        };

        page.insert(db).await.context("Failed to create page")
    }

    /// Create multiple pages in a batch using bulk insert
    /// This is much more efficient than creating pages one by one
    ///
    /// Pages are inserted in chunks to avoid SQLite's 999 variable limit.
    /// With 9 columns per page, we use chunks of 100 pages (900 variables).
    pub async fn create_batch(
        db: &DatabaseConnection,
        pages_models: &[pages::Model],
    ) -> Result<()> {
        if pages_models.is_empty() {
            return Ok(());
        }

        // SQLite has a limit of 999 bound parameters per query.
        // Each page has 9 columns, so we can safely insert 100 pages per batch (900 params).
        const BATCH_SIZE: usize = 100;

        for chunk in pages_models.chunks(BATCH_SIZE) {
            let active_models: Vec<pages::ActiveModel> = chunk
                .iter()
                .map(|page_model| pages::ActiveModel {
                    id: Set(page_model.id),
                    book_id: Set(page_model.book_id),
                    page_number: Set(page_model.page_number),
                    file_name: Set(page_model.file_name.clone()),
                    format: Set(page_model.format.clone()),
                    width: Set(page_model.width),
                    height: Set(page_model.height),
                    file_size: Set(page_model.file_size),
                    created_at: Set(page_model.created_at),
                })
                .collect();

            Pages::insert_many(active_models)
                .exec(db)
                .await
                .context("Failed to batch create pages")?;
        }

        Ok(())
    }

    /// Get a page by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<pages::Model>> {
        Pages::find_by_id(id)
            .one(db)
            .await
            .context("Failed to get page by ID")
    }

    /// Get a page by book ID and page number
    pub async fn get_by_book_and_number(
        db: &DatabaseConnection,
        book_id: Uuid,
        page_number: i32,
    ) -> Result<Option<pages::Model>> {
        Pages::find()
            .filter(pages::Column::BookId.eq(book_id))
            .filter(pages::Column::PageNumber.eq(page_number))
            .one(db)
            .await
            .context("Failed to get page by book and number")
    }

    /// Get all pages for a book
    pub async fn list_by_book(db: &DatabaseConnection, book_id: Uuid) -> Result<Vec<pages::Model>> {
        Pages::find()
            .filter(pages::Column::BookId.eq(book_id))
            .order_by_asc(pages::Column::PageNumber)
            .all(db)
            .await
            .context("Failed to list pages by book")
    }

    /// Delete all pages for a book
    pub async fn delete_by_book(db: &DatabaseConnection, book_id: Uuid) -> Result<()> {
        Pages::delete_many()
            .filter(pages::Column::BookId.eq(book_id))
            .exec(db)
            .await
            .context("Failed to delete pages by book")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::repositories::{BookRepository, LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;
    use chrono::Utc;

    /// Helper to create a test page model
    fn create_page_model(book_id: Uuid, page_number: i32, file_name: &str) -> pages::Model {
        pages::Model {
            id: Uuid::new_v4(),
            book_id,
            page_number,
            file_name: file_name.to_string(),
            format: "jpeg".to_string(),
            width: 800,
            height: 1200,
            file_size: 1024,
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_create_page() {
        let (db, _temp_dir) = create_test_db().await;

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
            file_hash: "hash123".to_string(),
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
            koreader_hash: None,
            epub_positions: None,
        };
        BookRepository::create(db.sea_orm_connection(), &book, None)
            .await
            .unwrap();

        let page = create_page_model(book.id, 1, "page001.jpg");
        let created = PageRepository::create(db.sea_orm_connection(), &page)
            .await
            .unwrap();

        assert_eq!(created.id, page.id);
        assert_eq!(created.page_number, 1);
        assert_eq!(created.file_name, "page001.jpg");
        assert_eq!(created.format, "jpeg");
    }

    #[tokio::test]
    async fn test_get_page_by_id() {
        let (db, _temp_dir) = create_test_db().await;

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
            file_hash: "hash123".to_string(),
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
            koreader_hash: None,
            epub_positions: None,
        };
        BookRepository::create(db.sea_orm_connection(), &book, None)
            .await
            .unwrap();

        let page = create_page_model(book.id, 1, "page001.jpg");
        PageRepository::create(db.sea_orm_connection(), &page)
            .await
            .unwrap();

        let retrieved = PageRepository::get_by_id(db.sea_orm_connection(), page.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.id, page.id);
        assert_eq!(retrieved.page_number, 1);
    }

    #[tokio::test]
    async fn test_get_page_by_book_and_number() {
        let (db, _temp_dir) = create_test_db().await;

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
            file_hash: "hash123".to_string(),
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
            koreader_hash: None,
            epub_positions: None,
        };
        BookRepository::create(db.sea_orm_connection(), &book, None)
            .await
            .unwrap();

        let page = create_page_model(book.id, 5, "page005.jpg");
        PageRepository::create(db.sea_orm_connection(), &page)
            .await
            .unwrap();

        let retrieved = PageRepository::get_by_book_and_number(db.sea_orm_connection(), book.id, 5)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.page_number, 5);
        assert_eq!(retrieved.file_name, "page005.jpg");
    }

    #[tokio::test]
    async fn test_list_pages_by_book() {
        let (db, _temp_dir) = create_test_db().await;

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
            file_hash: "hash123".to_string(),
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
            koreader_hash: None,
            epub_positions: None,
        };
        BookRepository::create(db.sea_orm_connection(), &book, None)
            .await
            .unwrap();

        for i in 1..=3 {
            let page = create_page_model(book.id, i, &format!("page{:03}.jpg", i));
            PageRepository::create(db.sea_orm_connection(), &page)
                .await
                .unwrap();
        }

        let pages = PageRepository::list_by_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();

        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0].page_number, 1);
        assert_eq!(pages[2].page_number, 3);
    }

    #[tokio::test]
    async fn test_create_batch_pages() {
        let (db, _temp_dir) = create_test_db().await;

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
            file_hash: "hash123".to_string(),
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
            koreader_hash: None,
            epub_positions: None,
        };
        BookRepository::create(db.sea_orm_connection(), &book, None)
            .await
            .unwrap();

        let pages: Vec<_> = (1..=5)
            .map(|i| create_page_model(book.id, i, &format!("page{:03}.jpg", i)))
            .collect();

        PageRepository::create_batch(db.sea_orm_connection(), &pages)
            .await
            .unwrap();

        let retrieved = PageRepository::list_by_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();

        assert_eq!(retrieved.len(), 5);
    }

    #[tokio::test]
    async fn test_delete_pages_by_book() {
        let (db, _temp_dir) = create_test_db().await;

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
            file_hash: "hash123".to_string(),
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
            koreader_hash: None,
            epub_positions: None,
        };
        BookRepository::create(db.sea_orm_connection(), &book, None)
            .await
            .unwrap();

        for i in 1..=3 {
            let page = create_page_model(book.id, i, &format!("page{:03}.jpg", i));
            PageRepository::create(db.sea_orm_connection(), &page)
                .await
                .unwrap();
        }

        PageRepository::delete_by_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();

        let pages = PageRepository::list_by_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();

        assert_eq!(pages.len(), 0);
    }

    #[tokio::test]
    async fn test_create_batch_large_page_count() {
        // Test that batch insert works with page counts exceeding SQLite's 999 variable limit
        // 250 pages * 9 columns = 2250 variables, which would fail without chunking
        let (db, _temp_dir) = create_test_db().await;

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
            file_path: "/test/large_book.pdf".to_string(),
            file_name: "large_book.pdf".to_string(),
            file_size: 1024,
            file_hash: "hash123".to_string(),
            partial_hash: String::new(),
            format: "pdf".to_string(),
            page_count: 250,
            deleted: false,
            analyzed: false,
            analysis_error: None,
            analysis_errors: None,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
            koreader_hash: None,
            epub_positions: None,
        };
        BookRepository::create(db.sea_orm_connection(), &book, None)
            .await
            .unwrap();

        // Create 250 pages - this would exceed SQLite's limit without chunking
        let pages: Vec<_> = (1..=250)
            .map(|i| create_page_model(book.id, i, &format!("page{:04}.jpg", i)))
            .collect();

        // This should succeed with chunked inserts
        PageRepository::create_batch(db.sea_orm_connection(), &pages)
            .await
            .unwrap();

        let retrieved = PageRepository::list_by_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();

        assert_eq!(retrieved.len(), 250);
        assert_eq!(retrieved[0].page_number, 1);
        assert_eq!(retrieved[249].page_number, 250);
    }
}
