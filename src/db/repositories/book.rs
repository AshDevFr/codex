use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::db::entities::{books, prelude::*};

/// Repository for Book operations
pub struct BookRepository;

impl BookRepository {
    /// Create a new book from entity model
    pub async fn create(
        db: &DatabaseConnection,
        book_model: &books::Model,
    ) -> Result<books::Model> {
        let book = books::ActiveModel {
            id: Set(book_model.id),
            series_id: Set(book_model.series_id),
            title: Set(book_model.title.clone()),
            number: Set(book_model.number),
            file_path: Set(book_model.file_path.clone()),
            file_name: Set(book_model.file_name.clone()),
            file_size: Set(book_model.file_size),
            file_hash: Set(book_model.file_hash.clone()),
            format: Set(book_model.format.clone()),
            page_count: Set(book_model.page_count),
            modified_at: Set(book_model.modified_at),
            created_at: Set(book_model.created_at),
            updated_at: Set(book_model.updated_at),
        };

        book.insert(db).await.context("Failed to create book")
    }

    /// Get a book by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<books::Model>> {
        Books::find_by_id(id)
            .one(db)
            .await
            .context("Failed to get book by ID")
    }

    /// Get a book by file hash (for duplicate detection)
    pub async fn get_by_hash(db: &DatabaseConnection, hash: &str) -> Result<Option<books::Model>> {
        Books::find()
            .filter(books::Column::FileHash.eq(hash))
            .one(db)
            .await
            .context("Failed to get book by hash")
    }

    /// Get a book by file path
    pub async fn get_by_path(db: &DatabaseConnection, path: &str) -> Result<Option<books::Model>> {
        Books::find()
            .filter(books::Column::FilePath.eq(path))
            .one(db)
            .await
            .context("Failed to get book by path")
    }

    /// Get all books in a series
    pub async fn list_by_series(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Vec<books::Model>> {
        Books::find()
            .filter(books::Column::SeriesId.eq(series_id))
            .order_by_asc(books::Column::Number)
            .order_by_asc(books::Column::Title)
            .order_by_asc(books::Column::FileName)
            .all(db)
            .await
            .context("Failed to list books by series")
    }

    /// Update book
    pub async fn update(db: &DatabaseConnection, book_model: &books::Model) -> Result<()> {
        let active = books::ActiveModel {
            id: Set(book_model.id),
            series_id: Set(book_model.series_id),
            title: Set(book_model.title.clone()),
            number: Set(book_model.number),
            file_path: Set(book_model.file_path.clone()),
            file_name: Set(book_model.file_name.clone()),
            file_size: Set(book_model.file_size),
            file_hash: Set(book_model.file_hash.clone()),
            format: Set(book_model.format.clone()),
            page_count: Set(book_model.page_count),
            modified_at: Set(book_model.modified_at),
            created_at: Set(book_model.created_at),
            updated_at: Set(Utc::now()),
        };

        active.update(db).await.context("Failed to update book")?;

        Ok(())
    }

    /// Delete a book
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        Books::delete_by_id(id)
            .exec(db)
            .await
            .context("Failed to delete book")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::{LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;
    use crate::db::ScanningStrategy;
    use sea_orm::prelude::Decimal;

    /// Helper to create a test book model
    fn create_book_model(series_id: Uuid, path: &str, name: &str) -> books::Model {
        let now = Utc::now();
        books::Model {
            id: Uuid::new_v4(),
            series_id,
            title: None,
            number: None,
            file_path: path.to_string(),
            file_name: name.to_string(),
            file_size: 1024,
            file_hash: format!("hash_{}", Uuid::new_v4()),
            format: "cbz".to_string(),
            page_count: 10,
            modified_at: now,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn test_create_book() {
        let (db, _temp_dir) = create_test_db().await;

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

        let book = create_book_model(series.id, "/test/book.cbz", "book.cbz");
        let created = BookRepository::create(db.sea_orm_connection(), &book)
            .await
            .unwrap();

        assert_eq!(created.id, book.id);
        assert_eq!(created.file_path, "/test/book.cbz");
        assert_eq!(created.format, "cbz");
    }

    #[tokio::test]
    async fn test_get_book_by_id() {
        let (db, _temp_dir) = create_test_db().await;

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

        let book = create_book_model(series.id, "/test/book.cbz", "book.cbz");
        BookRepository::create(db.sea_orm_connection(), &book)
            .await
            .unwrap();

        let retrieved = BookRepository::get_by_id(db.sea_orm_connection(), book.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.id, book.id);
        assert_eq!(retrieved.file_path, "/test/book.cbz");
    }

    #[tokio::test]
    async fn test_get_book_by_hash() {
        let (db, _temp_dir) = create_test_db().await;

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

        let mut book = create_book_model(series.id, "/test/book.cbz", "book.cbz");
        book.file_hash = "unique_hash_123".to_string();

        BookRepository::create(db.sea_orm_connection(), &book)
            .await
            .unwrap();

        let retrieved = BookRepository::get_by_hash(db.sea_orm_connection(), "unique_hash_123")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.id, book.id);
        assert_eq!(retrieved.file_hash, "unique_hash_123");
    }

    #[tokio::test]
    async fn test_get_book_by_path() {
        let (db, _temp_dir) = create_test_db().await;

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

        let book = create_book_model(series.id, "/test/book.cbz", "book.cbz");
        BookRepository::create(db.sea_orm_connection(), &book)
            .await
            .unwrap();

        let retrieved = BookRepository::get_by_path(db.sea_orm_connection(), "/test/book.cbz")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.id, book.id);
        assert_eq!(retrieved.file_path, "/test/book.cbz");
    }

    #[tokio::test]
    async fn test_list_books_by_series() {
        let (db, _temp_dir) = create_test_db().await;

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

        let mut book1 = create_book_model(series.id, "/test/book1.cbz", "book1.cbz");
        book1.number = Some(Decimal::from(1));

        let mut book2 = create_book_model(series.id, "/test/book2.cbz", "book2.cbz");
        book2.number = Some(Decimal::from(2));

        BookRepository::create(db.sea_orm_connection(), &book1)
            .await
            .unwrap();
        BookRepository::create(db.sea_orm_connection(), &book2)
            .await
            .unwrap();

        let books = BookRepository::list_by_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();

        assert_eq!(books.len(), 2);
    }

    #[tokio::test]
    async fn test_update_book() {
        let (db, _temp_dir) = create_test_db().await;

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

        let mut book = create_book_model(series.id, "/test/book.cbz", "book.cbz");
        BookRepository::create(db.sea_orm_connection(), &book)
            .await
            .unwrap();

        book.title = Some("Updated Title".to_string());
        book.number = Some(Decimal::from(5));

        BookRepository::update(db.sea_orm_connection(), &book)
            .await
            .unwrap();

        let retrieved = BookRepository::get_by_id(db.sea_orm_connection(), book.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.title, Some("Updated Title".to_string()));
        assert_eq!(retrieved.number, Some(Decimal::from(5)));
    }

    #[tokio::test]
    async fn test_delete_book() {
        let (db, _temp_dir) = create_test_db().await;

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

        let book = create_book_model(series.id, "/test/book.cbz", "book.cbz");
        BookRepository::create(db.sea_orm_connection(), &book)
            .await
            .unwrap();

        BookRepository::delete(db.sea_orm_connection(), book.id)
            .await
            .unwrap();

        let result = BookRepository::get_by_id(db.sea_orm_connection(), book.id)
            .await
            .unwrap();

        assert!(result.is_none());
    }
}
