use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, RelationTrait, Set,
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
            partial_hash: Set(book_model.partial_hash.clone()),
            format: Set(book_model.format.clone()),
            page_count: Set(book_model.page_count),
            deleted: Set(book_model.deleted),
            analyzed: Set(book_model.analyzed),
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
        include_deleted: bool,
    ) -> Result<Vec<books::Model>> {
        let mut query = Books::find().filter(books::Column::SeriesId.eq(series_id));

        if !include_deleted {
            query = query.filter(books::Column::Deleted.eq(false));
        }

        query
            .order_by_asc(books::Column::Number)
            .order_by_asc(books::Column::Title)
            .order_by_asc(books::Column::FileName)
            .all(db)
            .await
            .context("Failed to list books by series")
    }

    /// List all books with pagination
    pub async fn list_all(
        db: &DatabaseConnection,
        include_deleted: bool,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<books::Model>, u64)> {
        let mut query = Books::find();

        if !include_deleted {
            query = query.filter(books::Column::Deleted.eq(false));
        }

        // Get total count
        let total = query
            .clone()
            .paginate(db, 1)
            .num_items()
            .await
            .context("Failed to count books")?;

        // Get paginated results
        let books = query
            .order_by_asc(books::Column::Title)
            .order_by_asc(books::Column::FileName)
            .offset(page * page_size)
            .limit(page_size)
            .all(db)
            .await
            .context("Failed to list all books")?;

        Ok((books, total))
    }

    /// List books by library with pagination
    pub async fn list_by_library(
        db: &DatabaseConnection,
        library_id: Uuid,
        include_deleted: bool,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<books::Model>, u64)> {
        use crate::db::entities::series;
        use sea_orm::JoinType;

        // Build query joining books with series to filter by library
        let mut query = Books::find()
            .join(JoinType::InnerJoin, books::Relation::Series.def())
            .filter(series::Column::LibraryId.eq(library_id));

        if !include_deleted {
            query = query.filter(books::Column::Deleted.eq(false));
        }

        // Get total count
        let total = query
            .clone()
            .paginate(db, 1)
            .num_items()
            .await
            .context("Failed to count books in library")?;

        // Get paginated results
        let books = query
            .order_by_asc(books::Column::Title)
            .order_by_asc(books::Column::FileName)
            .offset(page * page_size)
            .limit(page_size)
            .all(db)
            .await
            .context("Failed to list books by library")?;

        Ok((books, total))
    }

    /// List recently added books with pagination
    pub async fn list_recently_added(
        db: &DatabaseConnection,
        library_id: Option<Uuid>,
        include_deleted: bool,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<books::Model>, u64)> {
        use crate::db::entities::series;
        use sea_orm::JoinType;

        let mut query = Books::find();

        // Join with series if filtering by library
        if let Some(lib_id) = library_id {
            query = query
                .join(JoinType::InnerJoin, books::Relation::Series.def())
                .filter(series::Column::LibraryId.eq(lib_id));
        }

        if !include_deleted {
            query = query.filter(books::Column::Deleted.eq(false));
        }

        // Get total count
        let total = query
            .clone()
            .paginate(db, 1)
            .num_items()
            .await
            .context("Failed to count recently added books")?;

        // Get paginated results, ordered by created_at descending (most recent first)
        let books = query
            .order_by_desc(books::Column::CreatedAt)
            .offset(page * page_size)
            .limit(page_size)
            .all(db)
            .await
            .context("Failed to list recently added books")?;

        Ok((books, total))
    }

    /// Get books with reading progress for a user (in-progress books)
    pub async fn list_with_progress(
        db: &DatabaseConnection,
        user_id: Uuid,
        library_id: Option<Uuid>,
        completed: Option<bool>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<books::Model>, u64)> {
        use crate::db::entities::{read_progress, series};
        use sea_orm::JoinType;

        let mut query = Books::find()
            .join(JoinType::InnerJoin, books::Relation::ReadProgress.def())
            .filter(read_progress::Column::UserId.eq(user_id));

        // Filter by library if specified
        if let Some(lib_id) = library_id {
            query = query
                .join(JoinType::InnerJoin, books::Relation::Series.def())
                .filter(series::Column::LibraryId.eq(lib_id));
        }

        // Filter by completion status if specified
        if let Some(is_completed) = completed {
            query = query.filter(read_progress::Column::Completed.eq(is_completed));
        }

        // Always exclude deleted books
        query = query.filter(books::Column::Deleted.eq(false));

        // Get total count
        let total = query
            .clone()
            .paginate(db, 1)
            .num_items()
            .await
            .context("Failed to count books with progress")?;

        // Get paginated results, ordered by most recently updated
        let books = query
            .order_by_desc(read_progress::Column::UpdatedAt)
            .offset(page * page_size)
            .limit(page_size)
            .all(db)
            .await
            .context("Failed to list books with progress")?;

        Ok((books, total))
    }

    /// Search books by title (case-insensitive)
    pub async fn search_by_title(
        db: &DatabaseConnection,
        query: &str,
    ) -> Result<Vec<books::Model>> {
        let pattern = format!("%{}%", query.to_lowercase());

        Books::find()
            .filter(books::Column::Title.contains(&pattern))
            .filter(books::Column::Deleted.eq(false))
            .order_by_asc(books::Column::Title)
            .limit(50)
            .all(db)
            .await
            .context("Failed to search books by title")
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
            partial_hash: Set(book_model.partial_hash.clone()),
            format: Set(book_model.format.clone()),
            page_count: Set(book_model.page_count),
            deleted: Set(book_model.deleted),
            analyzed: Set(book_model.analyzed),
            modified_at: Set(book_model.modified_at),
            created_at: Set(book_model.created_at),
            updated_at: Set(Utc::now()),
        };

        active.update(db).await.context("Failed to update book")?;

        Ok(())
    }

    /// Mark a book as deleted or restore it
    pub async fn mark_deleted(db: &DatabaseConnection, book_id: Uuid, deleted: bool) -> Result<()> {
        let book = Books::find_by_id(book_id)
            .one(db)
            .await
            .context("Failed to find book")?
            .ok_or_else(|| anyhow::anyhow!("Book not found"))?;

        let mut active: books::ActiveModel = book.into();
        active.deleted = Set(deleted);
        active.updated_at = Set(Utc::now());

        active
            .update(db)
            .await
            .context("Failed to mark book as deleted")?;

        // Clean up duplicates when soft-deleting (removed books shouldn't appear in duplicates)
        if deleted {
            use crate::db::repositories::BookDuplicatesRepository;
            BookDuplicatesRepository::cleanup_for_book(db, book_id).await?;
        }

        Ok(())
    }

    /// Delete a book
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        Books::delete_by_id(id)
            .exec(db)
            .await
            .context("Failed to delete book")?;

        // Clean up duplicates after deleting a book
        use crate::db::repositories::BookDuplicatesRepository;
        BookDuplicatesRepository::cleanup_for_book(db, id).await?;

        Ok(())
    }

    /// Count books in a library (excluding deleted books)
    pub async fn count_by_library(db: &DatabaseConnection, library_id: Uuid) -> Result<i64> {
        // Get all series in the library
        let series_list =
            crate::db::repositories::SeriesRepository::list_by_library(db, library_id).await?;
        let series_ids: Vec<Uuid> = series_list.iter().map(|s| s.id).collect();

        if series_ids.is_empty() {
            return Ok(0);
        }

        use sea_orm::PaginatorTrait;

        let count = Books::find()
            .filter(books::Column::SeriesId.is_in(series_ids))
            .filter(books::Column::Deleted.eq(false))
            .paginate(db, 1)
            .num_items()
            .await
            .context("Failed to count books")?;

        Ok(count as i64)
    }

    /// Purge all deleted books in a library (permanently delete from database)
    pub async fn purge_deleted_in_library(
        db: &DatabaseConnection,
        library_id: Uuid,
    ) -> Result<u64> {
        // Get all series in the library
        let series_list =
            crate::db::repositories::SeriesRepository::list_by_library(db, library_id).await?;
        let series_ids: Vec<Uuid> = series_list.iter().map(|s| s.id).collect();

        if series_ids.is_empty() {
            return Ok(0);
        }

        // Delete all books that are marked as deleted in this library
        let result = Books::delete_many()
            .filter(books::Column::SeriesId.is_in(series_ids))
            .filter(books::Column::Deleted.eq(true))
            .exec(db)
            .await
            .context("Failed to purge deleted books")?;

        let deleted_count = result.rows_affected;

        // Check if we should purge empty series
        let purge_empty_series = crate::db::repositories::SettingsRepository::get_value::<bool>(
            db,
            "purge.purge_empty_series",
        )
        .await
        .unwrap_or(Some(true))
        .unwrap_or(true);

        if purge_empty_series {
            // Purge empty series after deleting books
            let _series_deleted =
                crate::db::repositories::SeriesRepository::purge_empty_series_in_library(
                    db, library_id,
                )
                .await
                .context("Failed to purge empty series")?;
        }

        Ok(deleted_count)
    }

    /// Purge all deleted books in a series (permanently delete from database)
    pub async fn purge_deleted_in_series(db: &DatabaseConnection, series_id: Uuid) -> Result<u64> {
        let result = Books::delete_many()
            .filter(books::Column::SeriesId.eq(series_id))
            .filter(books::Column::Deleted.eq(true))
            .exec(db)
            .await
            .context("Failed to purge deleted books in series")?;

        let deleted_count = result.rows_affected;

        // Check if we should purge empty series
        let purge_empty_series = crate::db::repositories::SettingsRepository::get_value::<bool>(
            db,
            "purge.purge_empty_series",
        )
        .await
        .unwrap_or(Some(true))
        .unwrap_or(true);

        if purge_empty_series {
            // Check if series is now empty and delete it if so
            let _series_deleted =
                crate::db::repositories::SeriesRepository::purge_if_empty(db, series_id)
                    .await
                    .context("Failed to check/purge empty series")?;
        }

        Ok(deleted_count)
    }

    /// Get all unanalyzed books in a library
    pub async fn get_unanalyzed_in_library(
        db: &DatabaseConnection,
        library_id: Uuid,
    ) -> Result<Vec<books::Model>> {
        // Get all series in the library
        let series_list =
            crate::db::repositories::SeriesRepository::list_by_library(db, library_id).await?;
        let series_ids: Vec<Uuid> = series_list.iter().map(|s| s.id).collect();

        if series_ids.is_empty() {
            return Ok(vec![]);
        }

        Books::find()
            .filter(books::Column::SeriesId.is_in(series_ids))
            .filter(books::Column::Analyzed.eq(false))
            .filter(books::Column::Deleted.eq(false))
            .all(db)
            .await
            .context("Failed to get unanalyzed books")
    }

    /// Get all unanalyzed books in a series
    pub async fn get_unanalyzed_in_series(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Vec<books::Model>> {
        Books::find()
            .filter(books::Column::SeriesId.eq(series_id))
            .filter(books::Column::Analyzed.eq(false))
            .filter(books::Column::Deleted.eq(false))
            .all(db)
            .await
            .context("Failed to get unanalyzed books in series")
    }

    /// Check if a book is analyzed
    pub async fn is_analyzed(db: &DatabaseConnection, book_id: Uuid) -> Result<bool> {
        let book = Books::find_by_id(book_id)
            .one(db)
            .await
            .context("Failed to find book")?
            .ok_or_else(|| anyhow::anyhow!("Book not found"))?;

        Ok(book.analyzed)
    }

    /// Mark a book as analyzed
    pub async fn mark_analyzed(
        db: &DatabaseConnection,
        book_id: Uuid,
        analyzed: bool,
    ) -> Result<()> {
        let book = Books::find_by_id(book_id)
            .one(db)
            .await
            .context("Failed to find book")?
            .ok_or_else(|| anyhow::anyhow!("Book not found"))?;

        let mut active: books::ActiveModel = book.into();
        active.analyzed = Set(analyzed);
        active.updated_at = Set(Utc::now());

        active
            .update(db)
            .await
            .context("Failed to mark book as analyzed")?;

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
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 10,
            deleted: false,
            analyzed: false,
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

        let books = BookRepository::list_by_series(db.sea_orm_connection(), series.id, false)
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

    #[tokio::test]
    async fn test_list_all_books() {
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

        // Create test books
        for i in 1..=5 {
            let book = create_book_model(
                series.id,
                &format!("/test/book{}.cbz", i),
                &format!("book{}.cbz", i),
            );
            BookRepository::create(db.sea_orm_connection(), &book)
                .await
                .unwrap();
        }

        let (books, total) = BookRepository::list_all(db.sea_orm_connection(), false, 0, 10)
            .await
            .unwrap();

        assert_eq!(books.len(), 5);
        assert_eq!(total, 5);
    }

    #[tokio::test]
    async fn test_list_all_books_with_pagination() {
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

        // Create 10 test books
        for i in 1..=10 {
            let book = create_book_model(
                series.id,
                &format!("/test/book{:02}.cbz", i),
                &format!("book{:02}.cbz", i),
            );
            BookRepository::create(db.sea_orm_connection(), &book)
                .await
                .unwrap();
        }

        // Get first page (5 items)
        let (books_page1, total) = BookRepository::list_all(db.sea_orm_connection(), false, 0, 5)
            .await
            .unwrap();

        assert_eq!(books_page1.len(), 5);
        assert_eq!(total, 10);

        // Get second page (5 items)
        let (books_page2, total) = BookRepository::list_all(db.sea_orm_connection(), false, 1, 5)
            .await
            .unwrap();

        assert_eq!(books_page2.len(), 5);
        assert_eq!(total, 10);

        // Verify different books on each page
        assert_ne!(books_page1[0].id, books_page2[0].id);
    }

    #[tokio::test]
    async fn test_list_all_books_excludes_deleted() {
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

        // Create 3 books
        let mut book_ids = vec![];
        for i in 1..=3 {
            let book = create_book_model(
                series.id,
                &format!("/test/book{}.cbz", i),
                &format!("book{}.cbz", i),
            );
            let created = BookRepository::create(db.sea_orm_connection(), &book)
                .await
                .unwrap();
            book_ids.push(created.id);
        }

        // Mark one book as deleted
        BookRepository::mark_deleted(db.sea_orm_connection(), book_ids[1], true)
            .await
            .unwrap();

        // List without deleted
        let (books, total) = BookRepository::list_all(db.sea_orm_connection(), false, 0, 10)
            .await
            .unwrap();

        assert_eq!(books.len(), 2);
        assert_eq!(total, 2);

        // List with deleted
        let (books_with_deleted, total_with_deleted) =
            BookRepository::list_all(db.sea_orm_connection(), true, 0, 10)
                .await
                .unwrap();

        assert_eq!(books_with_deleted.len(), 3);
        assert_eq!(total_with_deleted, 3);
    }

    #[tokio::test]
    async fn test_list_all_books_empty() {
        let (db, _temp_dir) = create_test_db().await;

        let (books, total) = BookRepository::list_all(db.sea_orm_connection(), false, 0, 10)
            .await
            .unwrap();

        assert_eq!(books.len(), 0);
        assert_eq!(total, 0);
    }

    #[tokio::test]
    async fn test_list_all_books_orders_by_title() {
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

        // Create books with different titles
        let titles = vec!["Zebra", "Apple", "Monkey", "Banana"];
        for title in titles {
            let mut book = create_book_model(
                series.id,
                &format!("/test/{}.cbz", title),
                &format!("{}.cbz", title),
            );
            book.title = Some(title.to_string());
            BookRepository::create(db.sea_orm_connection(), &book)
                .await
                .unwrap();
        }

        let (books, _) = BookRepository::list_all(db.sea_orm_connection(), false, 0, 10)
            .await
            .unwrap();

        assert_eq!(books[0].title, Some("Apple".to_string()));
        assert_eq!(books[1].title, Some("Banana".to_string()));
        assert_eq!(books[2].title, Some("Monkey".to_string()));
        assert_eq!(books[3].title, Some("Zebra".to_string()));
    }
}
