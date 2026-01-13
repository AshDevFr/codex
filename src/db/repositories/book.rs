use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{
    sea_query::{Expr, Func},
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, RelationTrait, Set,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::db::entities::{books, prelude::*};
use crate::db::repositories::SeriesRepository;
use crate::events::{EntityChangeEvent, EntityEvent, EventBroadcaster};

/// Repository for Book operations
pub struct BookRepository;

impl BookRepository {
    /// Create a new book from entity model
    pub async fn create(
        db: &DatabaseConnection,
        book_model: &books::Model,
        event_broadcaster: Option<&Arc<EventBroadcaster>>,
    ) -> Result<books::Model> {
        let book = books::ActiveModel {
            id: Set(book_model.id),
            series_id: Set(book_model.series_id),
            library_id: Set(book_model.library_id),
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
            analysis_error: Set(book_model.analysis_error.clone()),
            modified_at: Set(book_model.modified_at),
            created_at: Set(book_model.created_at),
            updated_at: Set(book_model.updated_at),
            thumbnail_path: Set(book_model.thumbnail_path.clone()),
            thumbnail_generated_at: Set(book_model.thumbnail_generated_at),
        };

        let created_book = book.insert(db).await.context("Failed to create book")?;

        // Emit BookCreated event if broadcaster is available
        if let Some(broadcaster) = event_broadcaster {
            // Get library_id by finding the series
            if let Ok(Some(series)) =
                crate::db::repositories::SeriesRepository::get_by_id(db, created_book.series_id)
                    .await
            {
                let event = EntityChangeEvent::new(
                    EntityEvent::BookCreated {
                        book_id: created_book.id,
                        series_id: created_book.series_id,
                        library_id: series.library_id,
                    },
                    None, // System-triggered, no user_id
                );
                let _ = broadcaster.emit(event);
            }
        }

        Ok(created_book)
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

    /// Get a book by file path and library ID
    pub async fn get_by_path(
        db: &DatabaseConnection,
        library_id: Uuid,
        path: &str,
    ) -> Result<Option<books::Model>> {
        Books::find()
            .filter(books::Column::LibraryId.eq(library_id))
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

    /// Count books in a series (excluding deleted)
    pub async fn count_by_series(db: &DatabaseConnection, series_id: Uuid) -> Result<u64> {
        Books::find()
            .filter(books::Column::SeriesId.eq(series_id))
            .filter(books::Column::Deleted.eq(false))
            .count(db)
            .await
            .context("Failed to count books in series")
    }

    /// Get the adjacent (previous and next) books in the same series
    ///
    /// Returns books ordered by number, then title, then filename.
    /// Previous is the book that comes before the given book, next is after.
    pub async fn get_adjacent_in_series(
        db: &DatabaseConnection,
        book_id: Uuid,
    ) -> Result<(Option<books::Model>, Option<books::Model>)> {
        // First get the target book
        let book = Self::get_by_id(db, book_id)
            .await?
            .context("Book not found")?;

        // Get all non-deleted books in the series, ordered
        let all_books = Books::find()
            .filter(books::Column::SeriesId.eq(book.series_id))
            .filter(books::Column::Deleted.eq(false))
            .order_by_asc(books::Column::Number)
            .order_by_asc(books::Column::Title)
            .order_by_asc(books::Column::FileName)
            .all(db)
            .await
            .context("Failed to list books in series")?;

        // Find the position of the target book
        let position = all_books.iter().position(|b| b.id == book_id);

        match position {
            Some(pos) => {
                let prev = if pos > 0 {
                    all_books.get(pos - 1).cloned()
                } else {
                    None
                };
                let next = all_books.get(pos + 1).cloned();
                Ok((prev, next))
            }
            None => Ok((None, None)),
        }
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

    /// List books by their IDs with pagination
    pub async fn list_by_ids(
        db: &DatabaseConnection,
        ids: &[Uuid],
        include_deleted: bool,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<books::Model>, u64)> {
        if ids.is_empty() {
            return Ok((vec![], 0));
        }

        // Total count is the number of IDs
        let total = ids.len() as u64;

        // Get paginated results
        let mut query = Books::find().filter(books::Column::Id.is_in(ids.to_vec()));

        if !include_deleted {
            query = query.filter(books::Column::Deleted.eq(false));
        }

        let books = query
            .order_by_asc(books::Column::Title)
            .order_by_asc(books::Column::FileName)
            .offset(page * page_size)
            .limit(page_size)
            .all(db)
            .await
            .context("Failed to list books by IDs")?;

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
        // Build query filtering directly by library_id (now on books table)
        let mut query = Books::find().filter(books::Column::LibraryId.eq(library_id));

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

    /// List books by library with series compound sort (series name + book number)
    ///
    /// This sort groups books by their series name alphabetically, then sorts
    /// books within each series by their book number. This is the "reading order" sort.
    pub async fn list_by_library_series_sorted(
        db: &DatabaseConnection,
        library_id: Uuid,
        include_deleted: bool,
        ascending: bool,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<books::Model>, u64)> {
        use crate::db::entities::{series, series_metadata};
        use sea_orm::{JoinType, Order};

        // Build query filtering directly by library_id (now on books table)
        let mut query = Books::find().filter(books::Column::LibraryId.eq(library_id));

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

        // Determine sort order
        let order = if ascending { Order::Asc } else { Order::Desc };

        // Get paginated results with series sorting
        // JOIN with series and series_metadata to get series name for sorting
        let books = query
            .join(JoinType::LeftJoin, books::Relation::Series.def())
            .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def())
            // Sort by series title_sort (if set) or series name
            .order_by(series_metadata::Column::TitleSort, order.clone())
            .order_by(series::Column::Name, order.clone())
            // Then by book number within series
            .order_by(books::Column::Number, Order::Asc)
            // Then by title as fallback
            .order_by(books::Column::Title, Order::Asc)
            .offset(page * page_size)
            .limit(page_size)
            .all(db)
            .await
            .context("Failed to list books by library with series sort")?;

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

    /// List recently read books (ordered by read_progress updated_at descending)
    /// This returns books that have been read recently, regardless of completion status
    pub async fn list_recently_read(
        db: &DatabaseConnection,
        user_id: Uuid,
        library_id: Option<Uuid>,
        limit: u64,
    ) -> Result<Vec<books::Model>> {
        use crate::db::entities::{read_progress, series};
        use sea_orm::JoinType;

        let mut query = Books::find()
            .join(JoinType::InnerJoin, books::Relation::ReadProgress.def())
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(books::Column::Deleted.eq(false));

        // Filter by library if specified
        if let Some(lib_id) = library_id {
            query = query
                .join(JoinType::InnerJoin, books::Relation::Series.def())
                .filter(series::Column::LibraryId.eq(lib_id));
        }

        query
            .order_by_desc(read_progress::Column::UpdatedAt)
            .limit(limit)
            .all(db)
            .await
            .context("Failed to list recently read books")
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

    /// Get "on deck" books - next unread book in series where user has completed at least one book
    /// and has no books currently in-progress in that series.
    ///
    /// Logic:
    /// 1. Find series where user has completed at least one book
    /// 2. Exclude series that have any book with in-progress reading (completed=false)
    /// 3. For each qualifying series, find the first unread book (by sort order)
    pub async fn list_on_deck(
        db: &DatabaseConnection,
        user_id: Uuid,
        library_id: Option<Uuid>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<books::Model>, u64)> {
        use crate::db::entities::{read_progress, series};
        use sea_orm::JoinType;

        // Step 1: Get series IDs where user has completed at least one book
        let completed_series_query = Books::find()
            .select_only()
            .column(books::Column::SeriesId)
            .join(JoinType::InnerJoin, books::Relation::ReadProgress.def())
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::Completed.eq(true))
            .group_by(books::Column::SeriesId);

        let completed_series: Vec<Uuid> = completed_series_query
            .into_tuple::<Uuid>()
            .all(db)
            .await
            .context("Failed to get completed series")?;

        if completed_series.is_empty() {
            return Ok((vec![], 0));
        }

        // Step 2: Get series IDs where user has in-progress books (to exclude)
        let in_progress_series_query = Books::find()
            .select_only()
            .column(books::Column::SeriesId)
            .join(JoinType::InnerJoin, books::Relation::ReadProgress.def())
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::Completed.eq(false))
            .group_by(books::Column::SeriesId);

        let in_progress_series: Vec<Uuid> = in_progress_series_query
            .into_tuple::<Uuid>()
            .all(db)
            .await
            .context("Failed to get in-progress series")?;

        // Step 3: Calculate eligible series (completed - in_progress)
        let eligible_series: Vec<Uuid> = completed_series
            .into_iter()
            .filter(|s| !in_progress_series.contains(s))
            .collect();

        if eligible_series.is_empty() {
            return Ok((vec![], 0));
        }

        // Step 4: Get all book IDs that have progress for this user (to exclude from unread)
        let books_with_progress: Vec<Uuid> = read_progress::Entity::find()
            .select_only()
            .column(read_progress::Column::BookId)
            .filter(read_progress::Column::UserId.eq(user_id))
            .into_tuple::<Uuid>()
            .all(db)
            .await
            .context("Failed to get books with progress")?;

        // Step 5: Get all books in eligible series that are unread
        let mut unread_query = Books::find()
            .filter(books::Column::SeriesId.is_in(eligible_series.clone()))
            .filter(books::Column::Deleted.eq(false));

        // Exclude books that have progress
        if !books_with_progress.is_empty() {
            unread_query = unread_query.filter(books::Column::Id.is_not_in(books_with_progress));
        }

        // Filter by library if specified
        if let Some(lib_id) = library_id {
            unread_query = unread_query
                .join(JoinType::InnerJoin, books::Relation::Series.def())
                .filter(series::Column::LibraryId.eq(lib_id));
        }

        // Order by series, then by book number/title/filename
        let all_unread_books = unread_query
            .order_by_asc(books::Column::SeriesId)
            .order_by_asc(books::Column::Number)
            .order_by_asc(books::Column::Title)
            .order_by_asc(books::Column::FileName)
            .all(db)
            .await
            .context("Failed to get unread books")?;

        // Step 6: Pick the first book from each series
        let mut seen_series: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
        let mut on_deck_books: Vec<books::Model> = Vec::new();

        for book in all_unread_books {
            if !seen_series.contains(&book.series_id) {
                seen_series.insert(book.series_id);
                on_deck_books.push(book);
            }
        }

        let total = on_deck_books.len() as u64;

        // Apply pagination
        let start = (page * page_size) as usize;
        if start >= on_deck_books.len() {
            return Ok((vec![], total));
        }
        let end = (start + page_size as usize).min(on_deck_books.len());
        let paginated_books = on_deck_books[start..end].to_vec();

        Ok((paginated_books, total))
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

    /// Full-text search books by title (truly case-insensitive using LOWER())
    /// Returns book IDs matching the search query with pagination
    pub async fn full_text_search(
        db: &DatabaseConnection,
        query: &str,
        include_deleted: bool,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<books::Model>, u64)> {
        let pattern = format!("%{}%", query.to_lowercase());

        // Use LOWER(title) LIKE LOWER(pattern) for case-insensitive search
        let lower_title = Func::lower(Expr::col(books::Column::Title));
        let mut search_condition = Condition::all().add(Expr::expr(lower_title).like(&pattern));

        if !include_deleted {
            search_condition = search_condition.add(books::Column::Deleted.eq(false));
        }

        let total = Books::find()
            .filter(search_condition.clone())
            .count(db)
            .await
            .context("Failed to count full-text search results")?;

        let books_list = Books::find()
            .filter(search_condition)
            .order_by_asc(books::Column::Title)
            .offset(page * page_size)
            .limit(page_size)
            .all(db)
            .await
            .context("Failed to execute full-text search")?;

        Ok((books_list, total))
    }

    /// Full-text search books by title within a set of candidate IDs
    pub async fn full_text_search_filtered(
        db: &DatabaseConnection,
        query: &str,
        candidate_ids: &[Uuid],
        include_deleted: bool,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<books::Model>, u64)> {
        if candidate_ids.is_empty() {
            return Ok((vec![], 0));
        }

        let pattern = format!("%{}%", query.to_lowercase());

        // Use LOWER(title) LIKE LOWER(pattern) for case-insensitive search
        let lower_title = Func::lower(Expr::col(books::Column::Title));
        let mut search_condition = Condition::all()
            .add(Expr::expr(lower_title).like(&pattern))
            .add(books::Column::Id.is_in(candidate_ids.to_vec()));

        if !include_deleted {
            search_condition = search_condition.add(books::Column::Deleted.eq(false));
        }

        let total = Books::find()
            .filter(search_condition.clone())
            .count(db)
            .await
            .context("Failed to count full-text search results")?;

        let books_list = Books::find()
            .filter(search_condition)
            .order_by_asc(books::Column::Title)
            .offset(page * page_size)
            .limit(page_size)
            .all(db)
            .await
            .context("Failed to execute full-text search")?;

        Ok((books_list, total))
    }

    /// Update book
    pub async fn update(
        db: &DatabaseConnection,
        book_model: &books::Model,
        event_broadcaster: Option<&Arc<EventBroadcaster>>,
    ) -> Result<()> {
        let active = books::ActiveModel {
            id: Set(book_model.id),
            series_id: Set(book_model.series_id),
            library_id: Set(book_model.library_id),
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
            analysis_error: Set(book_model.analysis_error.clone()),
            modified_at: Set(book_model.modified_at),
            created_at: Set(book_model.created_at),
            updated_at: Set(Utc::now()),
            thumbnail_path: Set(book_model.thumbnail_path.clone()),
            thumbnail_generated_at: Set(book_model.thumbnail_generated_at),
        };

        active.update(db).await.context("Failed to update book")?;

        // Emit BookUpdated event if broadcaster is available
        if let Some(broadcaster) = event_broadcaster {
            // Get library_id by finding the series
            if let Ok(Some(series)) = SeriesRepository::get_by_id(db, book_model.series_id).await {
                let event = EntityChangeEvent::new(
                    EntityEvent::BookUpdated {
                        book_id: book_model.id,
                        series_id: book_model.series_id,
                        library_id: series.library_id,
                        fields: None, // Could track specific fields that changed if needed
                    },
                    None, // System-triggered, no user_id
                );
                let _ = broadcaster.emit(event);
            }
        }

        Ok(())
    }

    /// Mark a book as deleted or restore it
    pub async fn mark_deleted(
        db: &DatabaseConnection,
        book_id: Uuid,
        deleted: bool,
        event_broadcaster: Option<&Arc<EventBroadcaster>>,
    ) -> Result<()> {
        let book = Books::find_by_id(book_id)
            .one(db)
            .await
            .context("Failed to find book")?
            .ok_or_else(|| anyhow::anyhow!("Book not found"))?;

        let series_id = book.series_id;
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

        // Emit BookUpdated event if broadcaster is available
        if let Some(broadcaster) = event_broadcaster {
            // Get library_id by finding the series
            if let Ok(Some(series)) = SeriesRepository::get_by_id(db, series_id).await {
                let event = EntityChangeEvent::new(
                    EntityEvent::BookUpdated {
                        book_id,
                        series_id,
                        library_id: series.library_id,
                        fields: Some(vec!["deleted".to_string()]),
                    },
                    None, // System-triggered, no user_id
                );
                let _ = broadcaster.emit(event);
            }
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
        event_broadcaster: Option<&Arc<crate::events::EventBroadcaster>>,
    ) -> Result<u64> {
        // Get all series in the library
        let series_list =
            crate::db::repositories::SeriesRepository::list_by_library(db, library_id).await?;
        let series_ids: Vec<Uuid> = series_list.iter().map(|s| s.id).collect();

        if series_ids.is_empty() {
            return Ok(0);
        }

        // First, fetch all books that will be deleted so we can emit events
        let books_to_delete = Books::find()
            .filter(books::Column::SeriesId.is_in(series_ids.clone()))
            .filter(books::Column::Deleted.eq(true))
            .all(db)
            .await
            .context("Failed to fetch books to purge")?;

        // Delete all books that are marked as deleted in this library
        let result = Books::delete_many()
            .filter(books::Column::SeriesId.is_in(series_ids))
            .filter(books::Column::Deleted.eq(true))
            .exec(db)
            .await
            .context("Failed to purge deleted books")?;

        let deleted_count = result.rows_affected;

        // Emit BookDeleted events for each purged book
        if let Some(broadcaster) = event_broadcaster {
            use crate::events::{EntityChangeEvent, EntityEvent};
            use tracing::warn;

            for book in books_to_delete {
                let event = EntityChangeEvent {
                    event: EntityEvent::BookDeleted {
                        library_id,
                        series_id: book.series_id,
                        book_id: book.id,
                    },
                    user_id: None,
                    timestamp: chrono::Utc::now(),
                };

                if let Err(e) = broadcaster.emit(event) {
                    warn!(
                        "Failed to emit BookDeleted event for book {}: {:?}",
                        book.id, e
                    );
                }
            }
        }

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
                    db,
                    library_id,
                    event_broadcaster,
                )
                .await
                .context("Failed to purge empty series")?;
        }

        Ok(deleted_count)
    }

    /// Purge all deleted books in a series (permanently delete from database)
    pub async fn purge_deleted_in_series(
        db: &DatabaseConnection,
        series_id: Uuid,
        event_broadcaster: Option<&Arc<crate::events::EventBroadcaster>>,
    ) -> Result<u64> {
        // First, fetch the series to get library_id and all books that will be deleted
        let series = crate::db::repositories::SeriesRepository::get_by_id(db, series_id)
            .await?
            .context("Series not found")?;

        let books_to_delete = Books::find()
            .filter(books::Column::SeriesId.eq(series_id))
            .filter(books::Column::Deleted.eq(true))
            .all(db)
            .await
            .context("Failed to fetch books to purge")?;

        let result = Books::delete_many()
            .filter(books::Column::SeriesId.eq(series_id))
            .filter(books::Column::Deleted.eq(true))
            .exec(db)
            .await
            .context("Failed to purge deleted books in series")?;

        let deleted_count = result.rows_affected;

        // Emit BookDeleted events for each purged book
        if let Some(broadcaster) = event_broadcaster {
            use crate::events::{EntityChangeEvent, EntityEvent};
            use tracing::warn;

            for book in books_to_delete {
                let event = EntityChangeEvent {
                    event: EntityEvent::BookDeleted {
                        library_id: series.library_id,
                        series_id,
                        book_id: book.id,
                    },
                    user_id: None,
                    timestamp: chrono::Utc::now(),
                };

                if let Err(e) = broadcaster.emit(event) {
                    warn!(
                        "Failed to emit BookDeleted event for book {}: {:?}",
                        book.id, e
                    );
                }
            }
        }

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
            let _series_deleted = crate::db::repositories::SeriesRepository::purge_if_empty(
                db,
                series_id,
                event_broadcaster,
            )
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

    /// Count unread books in a series for a specific user
    /// A book is considered unread if it has no read progress or the progress is not completed
    pub async fn count_unread_in_series(
        db: &DatabaseConnection,
        series_id: Uuid,
        user_id: Uuid,
    ) -> Result<i64> {
        use crate::db::entities::read_progress;
        use sea_orm::JoinType;

        // Count all non-deleted books in the series
        let total_books = Books::find()
            .filter(books::Column::SeriesId.eq(series_id))
            .filter(books::Column::Deleted.eq(false))
            .count(db)
            .await
            .context("Failed to count books in series")?;

        // Count books with completed read progress
        let completed_count = Books::find()
            .filter(books::Column::SeriesId.eq(series_id))
            .filter(books::Column::Deleted.eq(false))
            .join(JoinType::InnerJoin, books::Relation::ReadProgress.def())
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::Completed.eq(true))
            .count(db)
            .await
            .context("Failed to count completed books in series")?;

        // Unread = total - completed
        Ok((total_books - completed_count) as i64)
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

    /// Set or clear analysis error for a book
    pub async fn set_analysis_error(
        db: &DatabaseConnection,
        book_id: Uuid,
        error: Option<String>,
    ) -> Result<()> {
        let book = Books::find_by_id(book_id)
            .one(db)
            .await
            .context("Failed to find book")?
            .ok_or_else(|| anyhow::anyhow!("Book not found"))?;

        let mut active: books::ActiveModel = book.into();
        active.analysis_error = Set(error);
        active.updated_at = Set(Utc::now());

        active
            .update(db)
            .await
            .context("Failed to set analysis error")?;

        Ok(())
    }

    /// List books with analysis errors
    /// Optional filters by library_id or series_id
    pub async fn list_with_errors(
        db: &DatabaseConnection,
        library_id: Option<Uuid>,
        series_id: Option<Uuid>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<books::Model>, u64)> {
        let mut query = Books::find()
            .filter(books::Column::AnalysisError.is_not_null())
            .filter(books::Column::Deleted.eq(false));

        if let Some(lib_id) = library_id {
            query = query.filter(books::Column::LibraryId.eq(lib_id));
        }

        if let Some(ser_id) = series_id {
            query = query.filter(books::Column::SeriesId.eq(ser_id));
        }

        // Get total count
        let total = query
            .clone()
            .count(db)
            .await
            .context("Failed to count books with errors")?;

        // Get paginated results
        let books = query
            .order_by_desc(books::Column::UpdatedAt)
            .offset(page * page_size)
            .limit(page_size)
            .all(db)
            .await
            .context("Failed to list books with errors")?;

        Ok((books, total))
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
    fn create_book_model(
        series_id: Uuid,
        library_id: Uuid,
        path: &str,
        name: &str,
    ) -> books::Model {
        let now = Utc::now();
        books::Model {
            id: Uuid::new_v4(),
            series_id,
            library_id,
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
            analysis_error: None,
            modified_at: now,
            created_at: now,
            updated_at: now,
            thumbnail_path: None,
            thumbnail_generated_at: None,
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

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        let book = create_book_model(series.id, library.id, "/test/book.cbz", "book.cbz");
        let created = BookRepository::create(db.sea_orm_connection(), &book, None)
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

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        let book = create_book_model(series.id, library.id, "/test/book.cbz", "book.cbz");
        BookRepository::create(db.sea_orm_connection(), &book, None)
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

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        let mut book = create_book_model(series.id, library.id, "/test/book.cbz", "book.cbz");
        book.file_hash = "unique_hash_123".to_string();

        BookRepository::create(db.sea_orm_connection(), &book, None)
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

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        let book = create_book_model(series.id, library.id, "/test/book.cbz", "book.cbz");
        BookRepository::create(db.sea_orm_connection(), &book, None)
            .await
            .unwrap();

        let retrieved =
            BookRepository::get_by_path(db.sea_orm_connection(), library.id, "/test/book.cbz")
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

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        let mut book1 = create_book_model(series.id, library.id, "/test/book1.cbz", "book1.cbz");
        book1.number = Some(Decimal::from(1));

        let mut book2 = create_book_model(series.id, library.id, "/test/book2.cbz", "book2.cbz");
        book2.number = Some(Decimal::from(2));

        BookRepository::create(db.sea_orm_connection(), &book1, None)
            .await
            .unwrap();
        BookRepository::create(db.sea_orm_connection(), &book2, None)
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

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        let mut book = create_book_model(series.id, library.id, "/test/book.cbz", "book.cbz");
        BookRepository::create(db.sea_orm_connection(), &book, None)
            .await
            .unwrap();

        book.title = Some("Updated Title".to_string());
        book.number = Some(Decimal::from(5));

        BookRepository::update(db.sea_orm_connection(), &book, None)
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

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        let book = create_book_model(series.id, library.id, "/test/book.cbz", "book.cbz");
        BookRepository::create(db.sea_orm_connection(), &book, None)
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

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        // Create test books
        for i in 1..=5 {
            let book = create_book_model(
                series.id,
                library.id,
                &format!("/test/book{}.cbz", i),
                &format!("book{}.cbz", i),
            );
            BookRepository::create(db.sea_orm_connection(), &book, None)
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

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        // Create 10 test books
        for i in 1..=10 {
            let book = create_book_model(
                series.id,
                library.id,
                &format!("/test/book{:02}.cbz", i),
                &format!("book{:02}.cbz", i),
            );
            BookRepository::create(db.sea_orm_connection(), &book, None)
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

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        // Create 3 books
        let mut book_ids = vec![];
        for i in 1..=3 {
            let book = create_book_model(
                series.id,
                library.id,
                &format!("/test/book{}.cbz", i),
                &format!("book{}.cbz", i),
            );
            let created = BookRepository::create(db.sea_orm_connection(), &book, None)
                .await
                .unwrap();
            book_ids.push(created.id);
        }

        // Mark one book as deleted
        BookRepository::mark_deleted(db.sea_orm_connection(), book_ids[1], true, None)
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

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        // Create books with different titles
        let titles = vec!["Zebra", "Apple", "Monkey", "Banana"];
        for title in titles {
            let mut book = create_book_model(
                series.id,
                library.id,
                &format!("/test/{}.cbz", title),
                &format!("{}.cbz", title),
            );
            book.title = Some(title.to_string());
            BookRepository::create(db.sea_orm_connection(), &book, None)
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

    #[tokio::test]
    async fn test_list_by_library() {
        let (db, _temp_dir) = create_test_db().await;

        // Create two libraries
        let library1 = LibraryRepository::create(
            db.sea_orm_connection(),
            "Library 1",
            "/lib1",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let library2 = LibraryRepository::create(
            db.sea_orm_connection(),
            "Library 2",
            "/lib2",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Create series in each library
        let series1 =
            SeriesRepository::create(db.sea_orm_connection(), library1.id, "Series 1", None)
                .await
                .unwrap();
        let series2 =
            SeriesRepository::create(db.sea_orm_connection(), library2.id, "Series 2", None)
                .await
                .unwrap();

        // Create books in library 1
        for i in 1..=3 {
            let book = create_book_model(
                series1.id,
                library1.id,
                &format!("/lib1/book{}.cbz", i),
                "book.cbz",
            );
            BookRepository::create(db.sea_orm_connection(), &book, None)
                .await
                .unwrap();
        }

        // Create books in library 2
        for i in 1..=2 {
            let book = create_book_model(
                series2.id,
                library2.id,
                &format!("/lib2/book{}.cbz", i),
                "book.cbz",
            );
            BookRepository::create(db.sea_orm_connection(), &book, None)
                .await
                .unwrap();
        }

        // Test library 1 books
        let (books, total) =
            BookRepository::list_by_library(db.sea_orm_connection(), library1.id, false, 0, 10)
                .await
                .unwrap();

        assert_eq!(books.len(), 3);
        assert_eq!(total, 3);

        // Test library 2 books
        let (books, total) =
            BookRepository::list_by_library(db.sea_orm_connection(), library2.id, false, 0, 10)
                .await
                .unwrap();

        assert_eq!(books.len(), 2);
        assert_eq!(total, 2);
    }

    #[tokio::test]
    async fn test_list_with_progress() {
        let (db, _temp_dir) = create_test_db().await;

        // Create library and series
        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        // Create books
        let mut book_ids = Vec::new();
        for i in 1..=5 {
            let book = create_book_model(
                series.id,
                library.id,
                &format!("/test/book{}.cbz", i),
                "book.cbz",
            );
            let created = BookRepository::create(db.sea_orm_connection(), &book, None)
                .await
                .unwrap();
            book_ids.push(created.id);
        }

        // Create user
        use crate::api::permissions::ADMIN_PERMISSIONS;
        use crate::db::entities::users;
        use crate::db::repositories::{ReadProgressRepository, UserRepository};
        use crate::utils::password;

        let password_hash = password::hash_password("test123").unwrap();
        let permissions_vec: Vec<_> = ADMIN_PERMISSIONS.iter().cloned().collect();
        let user = users::Model {
            id: Uuid::new_v4(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password_hash,
            is_admin: false,
            is_active: true,
            email_verified: true,
            permissions: serde_json::to_value(&permissions_vec).unwrap(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };
        let created_user = UserRepository::create(db.sea_orm_connection(), &user)
            .await
            .unwrap();

        // Add reading progress for some books
        for i in 0..3 {
            ReadProgressRepository::upsert(
                db.sea_orm_connection(),
                created_user.id,
                book_ids[i],
                5,
                false,
            )
            .await
            .unwrap();
        }

        // Mark one as completed
        ReadProgressRepository::upsert(
            db.sea_orm_connection(),
            created_user.id,
            book_ids[3],
            10,
            true,
        )
        .await
        .unwrap();

        // Test getting in-progress books (not completed)
        let (books, total) = BookRepository::list_with_progress(
            db.sea_orm_connection(),
            created_user.id,
            None,
            Some(false), // only in-progress
            0,
            10,
        )
        .await
        .unwrap();

        assert_eq!(books.len(), 3);
        assert_eq!(total, 3);

        // Test getting all books with progress
        let (books, total) = BookRepository::list_with_progress(
            db.sea_orm_connection(),
            created_user.id,
            None,
            None, // all with progress
            0,
            10,
        )
        .await
        .unwrap();

        assert_eq!(books.len(), 4); // 3 in-progress + 1 completed
        assert_eq!(total, 4);
    }

    #[tokio::test]
    async fn test_list_recently_added() {
        let (db, _temp_dir) = create_test_db().await;

        // Create library and series
        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        // Create books with delays to ensure different timestamps
        for i in 1..=5 {
            let book = create_book_model(
                series.id,
                library.id,
                &format!("/test/book{}.cbz", i),
                "book.cbz",
            );
            BookRepository::create(db.sea_orm_connection(), &book, None)
                .await
                .unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        // Test getting recently added books
        let (books, total) =
            BookRepository::list_recently_added(db.sea_orm_connection(), None, false, 0, 10)
                .await
                .unwrap();

        assert_eq!(books.len(), 5);
        assert_eq!(total, 5);

        // Verify books are ordered by created_at descending (most recent first)
        for i in 0..books.len() - 1 {
            assert!(
                books[i].created_at >= books[i + 1].created_at,
                "Books should be ordered by created_at descending"
            );
        }

        // Test filtering by library
        let (books, total) = BookRepository::list_recently_added(
            db.sea_orm_connection(),
            Some(library.id),
            false,
            0,
            10,
        )
        .await
        .unwrap();

        assert_eq!(books.len(), 5);
        assert_eq!(total, 5);
    }

    #[tokio::test]
    async fn test_set_analysis_error() {
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

        let book = create_book_model(series.id, library.id, "/test/book.cbz", "book.cbz");
        BookRepository::create(db.sea_orm_connection(), &book, None)
            .await
            .unwrap();

        // Verify initial state has no error
        let retrieved = BookRepository::get_by_id(db.sea_orm_connection(), book.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retrieved.analysis_error, None);

        // Set an analysis error
        BookRepository::set_analysis_error(
            db.sea_orm_connection(),
            book.id,
            Some("Test error: invalid archive".to_string()),
        )
        .await
        .unwrap();

        let retrieved = BookRepository::get_by_id(db.sea_orm_connection(), book.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            retrieved.analysis_error,
            Some("Test error: invalid archive".to_string())
        );

        // Clear the analysis error
        BookRepository::set_analysis_error(db.sea_orm_connection(), book.id, None)
            .await
            .unwrap();

        let retrieved = BookRepository::get_by_id(db.sea_orm_connection(), book.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retrieved.analysis_error, None);
    }

    #[tokio::test]
    async fn test_list_with_errors() {
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

        // Create a book without error
        let book1 = create_book_model(series.id, library.id, "/test/book1.cbz", "book1.cbz");
        BookRepository::create(db.sea_orm_connection(), &book1, None)
            .await
            .unwrap();

        // Create a book with error
        let mut book2 = create_book_model(series.id, library.id, "/test/book2.cbz", "book2.cbz");
        book2.analysis_error = Some("Failed to parse: invalid archive".to_string());
        BookRepository::create(db.sea_orm_connection(), &book2, None)
            .await
            .unwrap();

        // Create another book with error
        let mut book3 = create_book_model(series.id, library.id, "/test/book3.cbz", "book3.cbz");
        book3.analysis_error = Some("Unsupported format".to_string());
        BookRepository::create(db.sea_orm_connection(), &book3, None)
            .await
            .unwrap();

        // List all books with errors (no filter)
        let (books, total) =
            BookRepository::list_with_errors(db.sea_orm_connection(), None, None, 0, 10)
                .await
                .unwrap();

        assert_eq!(total, 2);
        assert_eq!(books.len(), 2);
        assert!(books.iter().all(|b| b.analysis_error.is_some()));

        // List with library filter
        let (books, total) = BookRepository::list_with_errors(
            db.sea_orm_connection(),
            Some(library.id),
            None,
            0,
            10,
        )
        .await
        .unwrap();

        assert_eq!(total, 2);
        assert_eq!(books.len(), 2);

        // List with series filter
        let (books, total) =
            BookRepository::list_with_errors(db.sea_orm_connection(), None, Some(series.id), 0, 10)
                .await
                .unwrap();

        assert_eq!(total, 2);
        assert_eq!(books.len(), 2);

        // Test pagination
        let (books, total) =
            BookRepository::list_with_errors(db.sea_orm_connection(), None, None, 0, 1)
                .await
                .unwrap();

        assert_eq!(total, 2);
        assert_eq!(books.len(), 1);

        let (books, total) =
            BookRepository::list_with_errors(db.sea_orm_connection(), None, None, 1, 1)
                .await
                .unwrap();

        assert_eq!(total, 2);
        assert_eq!(books.len(), 1);
    }
}
