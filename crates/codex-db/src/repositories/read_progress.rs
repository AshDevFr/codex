//! Repository for ReadProgress operations
//!
//! TODO: Remove allow(dead_code) when all reading progress features are fully integrated

#![allow(dead_code)]

use crate::entities::{read_progress, read_progress::Entity as ReadProgress};
use crate::repositories::ReadCompletionRepository;
use anyhow::Result;
use chrono::Utc;
use sea_orm::*;
use std::collections::HashMap;
use uuid::Uuid;

pub struct ReadProgressRepository;

impl ReadProgressRepository {
    /// Check if a database error is a unique constraint violation
    /// Handles both SQLite ("UNIQUE constraint failed") and PostgreSQL ("duplicate key")
    /// and matches both DbErr::Query and DbErr::Exec variants
    fn is_unique_constraint_error(err: &DbErr) -> bool {
        let error_str = match err {
            DbErr::Query(RuntimeErr::SqlxError(sqlx_err)) => sqlx_err.to_string(),
            DbErr::Exec(RuntimeErr::SqlxError(sqlx_err)) => sqlx_err.to_string(),
            _ => return false,
        };
        error_str.contains("UNIQUE constraint failed") || error_str.contains("duplicate key")
    }

    /// Get reading progress for a specific user and book
    pub async fn get_by_user_and_book(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Result<Option<read_progress::Model>> {
        Self::get_in(db, user_id, book_id).await
    }

    /// [`Self::get_by_user_and_book`] over any connection, so the upsert can run
    /// it inside its transaction.
    async fn get_in<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Result<Option<read_progress::Model>> {
        let progress = ReadProgress::find()
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::BookId.eq(book_id))
            .one(db)
            .await?;

        Ok(progress)
    }

    /// Create or update reading progress for a user and book
    pub async fn upsert(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
        current_page: i32,
        completed: bool,
    ) -> Result<read_progress::Model> {
        Self::upsert_with_percentage(db, user_id, book_id, current_page, None, completed, None)
            .await
    }

    /// Create or update reading progress for a user and book with optional percentage
    /// The percentage field is primarily used for EPUB books with reflowable content.
    /// The r2_progression field stores the full R2Progression JSON for Readium/OPDS 2.0 sync.
    pub async fn upsert_with_percentage(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
        current_page: i32,
        progress_percentage: Option<f64>,
        completed: bool,
        r2_progression: Option<String>,
    ) -> Result<read_progress::Model> {
        // Two concurrent writers for the same (user, book) can both miss the
        // existence check and race on the unique index. The loser retries the
        // whole transaction rather than re-querying inside it: on PostgreSQL a
        // failed statement poisons the surrounding transaction, so the old
        // fetch-and-update-in-place recovery would itself error out.
        match Self::upsert_txn(
            db,
            user_id,
            book_id,
            current_page,
            progress_percentage,
            completed,
            r2_progression.clone(),
        )
        .await
        {
            Err(e) if Self::is_unique_violation(&e) => {
                // The row exists now, so the retry takes the update path.
                Self::upsert_txn(
                    db,
                    user_id,
                    book_id,
                    current_page,
                    progress_percentage,
                    completed,
                    r2_progression,
                )
                .await
            }
            other => other,
        }
    }

    /// One attempt at the upsert, wrapped in a transaction together with the
    /// completion it may record.
    ///
    /// The two writes belong together: a completion that is not banked because
    /// the process died between them would be lost permanently, and the log is
    /// meant to be the authoritative record of what has been read.
    async fn upsert_txn(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
        current_page: i32,
        progress_percentage: Option<f64>,
        completed: bool,
        r2_progression: Option<String>,
    ) -> Result<read_progress::Model> {
        let txn = db.begin().await?;
        let now = Utc::now();

        // `started_at` of the pass this write belongs to. For an existing row
        // that is when the current pass began; for a new row it is now.
        let (result, pass_started_at) = match Self::get_in(&txn, user_id, book_id).await? {
            Some(existing_model) => {
                let pass_started_at = existing_model.started_at;
                let updated = Self::update_existing(
                    &txn,
                    existing_model,
                    current_page,
                    progress_percentage,
                    completed,
                    now,
                    r2_progression,
                )
                .await?;
                (updated, pass_started_at)
            }
            None => {
                let new_progress = read_progress::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    user_id: Set(user_id),
                    book_id: Set(book_id),
                    current_page: Set(current_page),
                    progress_percentage: Set(progress_percentage),
                    completed: Set(completed),
                    started_at: Set(now),
                    updated_at: Set(now),
                    completed_at: Set(if completed { Some(now) } else { None }),
                    r2_progression: Set(r2_progression),
                };
                (new_progress.insert(&txn).await?, now)
            }
        };

        if completed {
            Self::record_completion_if_new(
                &txn,
                user_id,
                book_id,
                pass_started_at,
                result.completed_at.unwrap_or(now),
            )
            .await?;
        }

        txn.commit().await?;
        Ok(result)
    }

    /// Bank a completion unless this read-through already has one.
    ///
    /// The guard compares against the progress row's `started_at`, not against
    /// `completed` or `completed_at`. Both of those are cleared when a book is
    /// un-completed, so tapping back one page from the end and forward again
    /// looks identical to a first-ever completion, and a guard reading either
    /// column would bank a second row for a single read-through. `started_at`
    /// survives that bounce and so delimits the pass.
    ///
    /// Marking a book unread deletes the progress row outright, which is what
    /// makes a genuine re-read record: the next pass gets a later `started_at`,
    /// so the earlier completion no longer falls inside it.
    async fn record_completion_if_new<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_id: Uuid,
        pass_started_at: chrono::DateTime<Utc>,
        completed_at: chrono::DateTime<Utc>,
    ) -> Result<()> {
        let already_recorded =
            ReadCompletionRepository::has_completion_since(db, user_id, book_id, pass_started_at)
                .await?;
        if already_recorded {
            return Ok(());
        }

        ReadCompletionRepository::record(db, user_id, book_id, pass_started_at, completed_at)
            .await?;
        Ok(())
    }

    /// Whether an error is the unique-index collision from two writers racing on
    /// the same `(user_id, book_id)`.
    fn is_unique_violation(err: &anyhow::Error) -> bool {
        err.downcast_ref::<DbErr>()
            .is_some_and(Self::is_unique_constraint_error)
    }

    /// Helper to update an existing progress record
    async fn update_existing<C: ConnectionTrait>(
        db: &C,
        existing_model: read_progress::Model,
        current_page: i32,
        progress_percentage: Option<f64>,
        completed: bool,
        now: chrono::DateTime<Utc>,
        r2_progression: Option<String>,
    ) -> Result<read_progress::Model> {
        let mut active_model: read_progress::ActiveModel = existing_model.clone().into();
        active_model.current_page = Set(current_page);
        active_model.progress_percentage = Set(progress_percentage);
        active_model.completed = Set(completed);
        active_model.updated_at = Set(now);
        // Only update r2_progression if a new value is provided;
        // passing None means "don't change", not "clear it"
        if r2_progression.is_some() {
            active_model.r2_progression = Set(r2_progression);
        }

        // Keep completed_at consistent with the completed flag: set it on the
        // transition to completed, and clear it when a book is un-completed so
        // a downgraded record never keeps a stale completion timestamp.
        if completed && existing_model.completed_at.is_none() {
            active_model.completed_at = Set(Some(now));
        } else if !completed && existing_model.completed_at.is_some() {
            active_model.completed_at = Set(None);
        }

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Delete reading progress
    pub async fn delete(db: &DatabaseConnection, user_id: Uuid, book_id: Uuid) -> Result<()> {
        ReadProgress::delete_many()
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::BookId.eq(book_id))
            .exec(db)
            .await?;

        Ok(())
    }

    /// Get all reading progress for a user
    pub async fn get_by_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Vec<read_progress::Model>> {
        let progress_list = ReadProgress::find()
            .filter(read_progress::Column::UserId.eq(user_id))
            .order_by_desc(read_progress::Column::UpdatedAt)
            .all(db)
            .await?;

        Ok(progress_list)
    }

    /// Get reading progress for a user and a batch of book IDs.
    /// Returns a HashMap keyed by book_id.
    pub async fn get_by_user_books(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, read_progress::Model>> {
        if book_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let results = ReadProgress::find()
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::BookId.is_in(book_ids.to_vec()))
            .all(db)
            .await?;

        Ok(results.into_iter().map(|p| (p.book_id, p)).collect())
    }

    /// Get currently reading books (not completed, sorted by most recently updated)
    pub async fn get_currently_reading(
        db: &DatabaseConnection,
        user_id: Uuid,
        limit: u64,
    ) -> Result<Vec<read_progress::Model>> {
        let progress_list = ReadProgress::find()
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::Completed.eq(false))
            .order_by_desc(read_progress::Column::UpdatedAt)
            .limit(limit)
            .all(db)
            .await?;

        Ok(progress_list)
    }

    /// Get completed books for a user
    pub async fn get_completed(
        db: &DatabaseConnection,
        user_id: Uuid,
        limit: Option<u64>,
    ) -> Result<Vec<read_progress::Model>> {
        let mut query = ReadProgress::find()
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::Completed.eq(true))
            .order_by_desc(read_progress::Column::CompletedAt);

        if let Some(limit_val) = limit {
            query = query.limit(limit_val);
        }

        let progress_list = query.all(db).await?;

        Ok(progress_list)
    }

    /// Mark a book as read (completed) for a user
    /// Sets current_page to the book's last page (1-indexed)
    pub async fn mark_as_read(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
        page_count: i32,
    ) -> Result<read_progress::Model> {
        // Mark as completed with the last page (1-indexed, same as page_count)
        Self::upsert(db, user_id, book_id, page_count, true).await
    }

    /// Mark a book as unread for a user
    /// Deletes the reading progress record entirely
    pub async fn mark_as_unread(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Result<()> {
        Self::delete(db, user_id, book_id).await
    }

    /// Get reading progress for a user across multiple books
    ///
    /// Returns a HashMap keyed by book_id for efficient lookups.
    /// Only returns books that have progress records for the given user.
    pub async fn get_for_user_books(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, read_progress::Model>> {
        if book_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let results = ReadProgress::find()
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::BookId.is_in(book_ids.to_vec()))
            .all(db)
            .await?;

        Ok(results.into_iter().map(|p| (p.book_id, p)).collect())
    }

    /// Mark all books in a series as read for a user
    /// Returns the number of books marked as read
    pub async fn mark_series_as_read(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_ids: Vec<(Uuid, i32)>, // Vec of (book_id, page_count)
    ) -> Result<usize> {
        let _now = Utc::now();
        let mut count = 0;

        // Process each book - page_count is 1-indexed (last page = page_count)
        for (book_id, page_count) in book_ids {
            Self::upsert(db, user_id, book_id, page_count, true).await?;
            count += 1;
        }

        Ok(count)
    }

    /// Mark all books in a series as unread for a user
    /// Deletes all reading progress records for the books
    /// Returns the number of books marked as unread
    pub async fn mark_series_as_unread(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_ids: Vec<Uuid>,
    ) -> Result<u64> {
        let result = ReadProgress::delete_many()
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::BookId.is_in(book_ids))
            .exec(db)
            .await?;

        Ok(result.rows_affected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::entities::{books, users};
    use crate::repositories::{
        BookRepository, LibraryRepository, SeriesRepository, UserRepository,
    };
    use crate::test_helpers::setup_test_db;
    use codex_models::ScanningStrategy;
    use codex_utils::password;

    async fn create_test_user(db: &DatabaseConnection) -> users::Model {
        let password_hash = password::hash_password("password").unwrap();
        let user = users::Model {
            id: Uuid::new_v4(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password_hash,
            role: "admin".to_string(),
            is_active: true,
            email_verified: false,
            permissions: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };
        UserRepository::create(db, &user).await.unwrap()
    }

    async fn create_test_book(db: &DatabaseConnection) -> books::Model {
        // Create a library first
        let library = LibraryRepository::create(
            db,
            "Test Library",
            "/test/library",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Create a series
        let series = SeriesRepository::create(db, library.id, "Test Series", None)
            .await
            .unwrap();

        // Create a book (title/number are now in book_metadata table)
        let book = books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
            path: format!("/test/book_{}.cbz", Uuid::new_v4()),
            file_name: "book.cbz".to_string(),
            file_size: 1024,
            file_hash: format!("hash_{}", Uuid::new_v4()),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 50,
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
            epub_spine_items: None,
        };
        BookRepository::create(db, &book, None).await.unwrap()
    }

    #[tokio::test]
    async fn test_create_progress() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        let progress = ReadProgressRepository::upsert(&db, user.id, book.id, 10, false)
            .await
            .unwrap();

        assert_eq!(progress.user_id, user.id);
        assert_eq!(progress.book_id, book.id);
        assert_eq!(progress.current_page, 10);
        assert!(!progress.completed);
        assert!(progress.completed_at.is_none());
    }

    #[tokio::test]
    async fn test_update_progress() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        // Create initial progress
        ReadProgressRepository::upsert(&db, user.id, book.id, 10, false)
            .await
            .unwrap();

        // Update progress
        let updated = ReadProgressRepository::upsert(&db, user.id, book.id, 25, false)
            .await
            .unwrap();

        assert_eq!(updated.current_page, 25);
        assert!(!updated.completed);
    }

    #[tokio::test]
    async fn test_mark_as_completed() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        // Create progress
        ReadProgressRepository::upsert(&db, user.id, book.id, 10, false)
            .await
            .unwrap();

        // Mark as completed
        let completed = ReadProgressRepository::upsert(&db, user.id, book.id, 50, true)
            .await
            .unwrap();

        assert!(completed.completed);
        assert!(completed.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_uncomplete_clears_completed_at() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        // Mark as completed so completed_at is populated.
        let completed = ReadProgressRepository::upsert(&db, user.id, book.id, 50, true)
            .await
            .unwrap();
        assert!(completed.completed);
        assert!(completed.completed_at.is_some());

        // A subsequent save that reports not-completed must clear completed_at
        // so the record stays consistent (completed_at is set iff completed).
        let uncompleted = ReadProgressRepository::upsert(&db, user.id, book.id, 10, false)
            .await
            .unwrap();
        assert!(!uncompleted.completed);
        assert!(uncompleted.completed_at.is_none());
    }

    #[tokio::test]
    async fn test_get_by_user() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book1 = create_test_book(&db).await;
        let book2 = create_test_book(&db).await;

        ReadProgressRepository::upsert(&db, user.id, book1.id, 10, false)
            .await
            .unwrap();
        ReadProgressRepository::upsert(&db, user.id, book2.id, 25, true)
            .await
            .unwrap();

        let progress_list = ReadProgressRepository::get_by_user(&db, user.id)
            .await
            .unwrap();

        assert_eq!(progress_list.len(), 2);
    }

    #[tokio::test]
    async fn test_get_currently_reading() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book1 = create_test_book(&db).await;
        let book2 = create_test_book(&db).await;
        let book3 = create_test_book(&db).await;

        ReadProgressRepository::upsert(&db, user.id, book1.id, 10, false)
            .await
            .unwrap();
        ReadProgressRepository::upsert(&db, user.id, book2.id, 25, false)
            .await
            .unwrap();
        ReadProgressRepository::upsert(&db, user.id, book3.id, 50, true)
            .await
            .unwrap();

        let currently_reading = ReadProgressRepository::get_currently_reading(&db, user.id, 10)
            .await
            .unwrap();

        assert_eq!(currently_reading.len(), 2);
        assert!(!currently_reading[0].completed);
        assert!(!currently_reading[1].completed);
    }

    #[tokio::test]
    async fn test_get_completed() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book1 = create_test_book(&db).await;
        let book2 = create_test_book(&db).await;

        ReadProgressRepository::upsert(&db, user.id, book1.id, 50, true)
            .await
            .unwrap();
        ReadProgressRepository::upsert(&db, user.id, book2.id, 25, false)
            .await
            .unwrap();

        let completed = ReadProgressRepository::get_completed(&db, user.id, None)
            .await
            .unwrap();

        assert_eq!(completed.len(), 1);
        assert!(completed[0].completed);
    }

    #[tokio::test]
    async fn test_delete_progress() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        ReadProgressRepository::upsert(&db, user.id, book.id, 10, false)
            .await
            .unwrap();

        ReadProgressRepository::delete(&db, user.id, book.id)
            .await
            .unwrap();

        let progress = ReadProgressRepository::get_by_user_and_book(&db, user.id, book.id)
            .await
            .unwrap();

        assert!(progress.is_none());
    }

    #[tokio::test]
    async fn test_mark_as_read() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        // Mark book as read
        let progress = ReadProgressRepository::mark_as_read(&db, user.id, book.id, book.page_count)
            .await
            .unwrap();

        assert_eq!(progress.user_id, user.id);
        assert_eq!(progress.book_id, book.id);
        assert_eq!(progress.current_page, book.page_count); // 1-indexed (last page = page_count)
        assert!(progress.completed);
        assert!(progress.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_mark_as_unread() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        // Create progress first
        ReadProgressRepository::upsert(&db, user.id, book.id, 25, false)
            .await
            .unwrap();

        // Mark as unread
        ReadProgressRepository::mark_as_unread(&db, user.id, book.id)
            .await
            .unwrap();

        // Verify progress is deleted
        let progress = ReadProgressRepository::get_by_user_and_book(&db, user.id, book.id)
            .await
            .unwrap();

        assert!(progress.is_none());
    }

    #[tokio::test]
    async fn test_mark_series_as_read() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book1 = create_test_book(&db).await;
        let book2 = create_test_book(&db).await;
        let book3 = create_test_book(&db).await;

        // Create book data with page counts
        let book_data = vec![
            (book1.id, book1.page_count),
            (book2.id, book2.page_count),
            (book3.id, book3.page_count),
        ];

        // Mark all books as read
        let count = ReadProgressRepository::mark_series_as_read(&db, user.id, book_data)
            .await
            .unwrap();

        assert_eq!(count, 3);

        // Verify all books are marked as read
        let progress1 = ReadProgressRepository::get_by_user_and_book(&db, user.id, book1.id)
            .await
            .unwrap()
            .unwrap();
        let progress2 = ReadProgressRepository::get_by_user_and_book(&db, user.id, book2.id)
            .await
            .unwrap()
            .unwrap();
        let progress3 = ReadProgressRepository::get_by_user_and_book(&db, user.id, book3.id)
            .await
            .unwrap()
            .unwrap();

        assert!(progress1.completed);
        assert!(progress2.completed);
        assert!(progress3.completed);
        // 1-indexed: last page = page_count
        assert_eq!(progress1.current_page, book1.page_count);
        assert_eq!(progress2.current_page, book2.page_count);
        assert_eq!(progress3.current_page, book3.page_count);
    }

    #[tokio::test]
    async fn test_mark_series_as_unread() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book1 = create_test_book(&db).await;
        let book2 = create_test_book(&db).await;
        let book3 = create_test_book(&db).await;

        // Create progress for all books
        ReadProgressRepository::upsert(&db, user.id, book1.id, 10, false)
            .await
            .unwrap();
        ReadProgressRepository::upsert(&db, user.id, book2.id, 20, true)
            .await
            .unwrap();
        ReadProgressRepository::upsert(&db, user.id, book3.id, 30, false)
            .await
            .unwrap();

        // Mark all books as unread
        let book_ids = vec![book1.id, book2.id, book3.id];
        let count = ReadProgressRepository::mark_series_as_unread(&db, user.id, book_ids)
            .await
            .unwrap();

        assert_eq!(count, 3);

        // Verify all progress is deleted
        let progress1 = ReadProgressRepository::get_by_user_and_book(&db, user.id, book1.id)
            .await
            .unwrap();
        let progress2 = ReadProgressRepository::get_by_user_and_book(&db, user.id, book2.id)
            .await
            .unwrap();
        let progress3 = ReadProgressRepository::get_by_user_and_book(&db, user.id, book3.id)
            .await
            .unwrap();

        assert!(progress1.is_none());
        assert!(progress2.is_none());
        assert!(progress3.is_none());
    }

    #[tokio::test]
    async fn test_unique_constraint_prevents_duplicates() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        // Create initial progress
        let progress1 = ReadProgressRepository::upsert(&db, user.id, book.id, 10, false)
            .await
            .unwrap();

        // Attempting to create another progress for the same user/book should update, not create duplicate
        let progress2 = ReadProgressRepository::upsert(&db, user.id, book.id, 20, false)
            .await
            .unwrap();

        // Should be the same record (same ID), just updated
        assert_eq!(progress1.id, progress2.id);
        assert_eq!(progress2.current_page, 20);

        // Verify only one record exists
        let all_progress = ReadProgressRepository::get_by_user(&db, user.id)
            .await
            .unwrap();
        assert_eq!(all_progress.len(), 1);
    }

    #[tokio::test]
    async fn test_get_for_user_books_empty_input() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        let result = ReadProgressRepository::get_for_user_books(&db, user.id, &[])
            .await
            .unwrap();

        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_get_for_user_books_multiple_books() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book1 = create_test_book(&db).await;
        let book2 = create_test_book(&db).await;
        let book3 = create_test_book(&db).await;

        // Create progress for book1 and book2 only
        ReadProgressRepository::upsert(&db, user.id, book1.id, 10, false)
            .await
            .unwrap();
        ReadProgressRepository::upsert(&db, user.id, book2.id, 25, true)
            .await
            .unwrap();

        // Query for all three books — only two should have progress
        let result = ReadProgressRepository::get_for_user_books(
            &db,
            user.id,
            &[book1.id, book2.id, book3.id],
        )
        .await
        .unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.contains_key(&book1.id));
        assert!(result.contains_key(&book2.id));
        assert!(!result.contains_key(&book3.id));
        assert_eq!(result.get(&book1.id).unwrap().current_page, 10);
        assert_eq!(result.get(&book2.id).unwrap().current_page, 25);
        assert!(result.get(&book2.id).unwrap().completed);
    }
    // ========================================================================
    // Completion log: the read-through guard
    //
    // The rule under test is that one read-through banks exactly one
    // completion, however many times the client re-asserts it, while a genuine
    // re-read (signalled by marking unread) banks another.
    // ========================================================================

    async fn completion_count(db: &DatabaseConnection, user_id: Uuid, book_id: Uuid) -> i64 {
        crate::repositories::ReadCompletionRepository::count_for_book(db, user_id, book_id)
            .await
            .unwrap()
    }

    /// (d) A first-ever completion, arriving through the insert path because no
    /// progress row exists yet.
    #[tokio::test]
    async fn first_completion_via_insert_records_one_row() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        ReadProgressRepository::upsert(&db, user.id, book.id, 50, true)
            .await
            .unwrap();

        assert_eq!(completion_count(&db, user.id, book.id).await, 1);
    }

    /// A completion arriving through the update path (progress existed first).
    #[tokio::test]
    async fn first_completion_via_update_records_one_row() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        ReadProgressRepository::upsert(&db, user.id, book.id, 10, false)
            .await
            .unwrap();
        assert_eq!(completion_count(&db, user.id, book.id).await, 0);

        ReadProgressRepository::upsert(&db, user.id, book.id, 50, true)
            .await
            .unwrap();
        assert_eq!(completion_count(&db, user.id, book.id).await, 1);
    }

    /// Reading without finishing banks nothing.
    #[tokio::test]
    async fn progress_without_completion_records_nothing() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        for page in [5, 20, 49] {
            ReadProgressRepository::upsert(&db, user.id, book.id, page, false)
                .await
                .unwrap();
        }

        assert_eq!(completion_count(&db, user.id, book.id).await, 0);
    }

    /// (a) Tapping back one page from the end and forward again. This is the
    /// case a guard keyed on `completed` or `completed_at` gets wrong: both are
    /// cleared by the back-tap, so the second completion looks like a first.
    #[tokio::test]
    async fn completing_after_a_back_tap_does_not_record_a_second_row() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        ReadProgressRepository::upsert(&db, user.id, book.id, 50, true)
            .await
            .unwrap();
        assert_eq!(completion_count(&db, user.id, book.id).await, 1);

        // Back one page: the row is un-completed and `completed_at` is wiped.
        let backed = ReadProgressRepository::upsert(&db, user.id, book.id, 49, false)
            .await
            .unwrap();
        assert!(!backed.completed);
        assert!(
            backed.completed_at.is_none(),
            "the un-complete path clears completed_at, which is why the guard \
             cannot key on it"
        );

        // Forward again.
        ReadProgressRepository::upsert(&db, user.id, book.id, 50, true)
            .await
            .unwrap();

        assert_eq!(
            completion_count(&db, user.id, book.id).await,
            1,
            "one read-through must bank exactly one completion"
        );
    }

    /// Several back-and-forth bounces still bank one completion.
    #[tokio::test]
    async fn repeated_bounces_record_one_row() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        for _ in 0..3 {
            ReadProgressRepository::upsert(&db, user.id, book.id, 50, true)
                .await
                .unwrap();
            ReadProgressRepository::upsert(&db, user.id, book.id, 49, false)
                .await
                .unwrap();
        }
        ReadProgressRepository::upsert(&db, user.id, book.id, 50, true)
            .await
            .unwrap();

        assert_eq!(completion_count(&db, user.id, book.id).await, 1);
    }

    /// (c) A client re-sending `completed = true` on an already-complete book.
    #[tokio::test]
    async fn re_asserting_completion_does_not_record_a_second_row() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        for _ in 0..4 {
            ReadProgressRepository::upsert(&db, user.id, book.id, 50, true)
                .await
                .unwrap();
        }

        assert_eq!(completion_count(&db, user.id, book.id).await, 1);
    }

    /// (b) A genuine re-read: marking unread deletes the progress row, so the
    /// next pass starts fresh and its completion banks a second row.
    #[tokio::test]
    async fn completing_after_marking_unread_records_a_second_row() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        ReadProgressRepository::upsert(&db, user.id, book.id, 50, true)
            .await
            .unwrap();
        assert_eq!(completion_count(&db, user.id, book.id).await, 1);

        ReadProgressRepository::mark_as_unread(&db, user.id, book.id)
            .await
            .unwrap();
        assert_eq!(
            completion_count(&db, user.id, book.id).await,
            1,
            "marking unread resets progress but must not erase history"
        );

        ReadProgressRepository::upsert(&db, user.id, book.id, 50, true)
            .await
            .unwrap();
        assert_eq!(completion_count(&db, user.id, book.id).await, 2);

        // And again, to prove the count keeps climbing.
        ReadProgressRepository::mark_as_unread(&db, user.id, book.id)
            .await
            .unwrap();
        ReadProgressRepository::upsert(&db, user.id, book.id, 50, true)
            .await
            .unwrap();
        assert_eq!(completion_count(&db, user.id, book.id).await, 3);
    }

    /// `mark_as_read` goes through the same hook, so it banks a completion.
    #[tokio::test]
    async fn mark_as_read_records_a_completion() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        ReadProgressRepository::mark_as_read(&db, user.id, book.id, 50)
            .await
            .unwrap();

        assert_eq!(completion_count(&db, user.id, book.id).await, 1);
    }

    /// (e) Re-running `mark_series_as_read` on an already-read series banks
    /// nothing new.
    #[tokio::test]
    async fn mark_series_as_read_is_idempotent_for_the_log() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let first = create_test_book(&db).await;
        let second = create_test_book(&db).await;

        let books = vec![(first.id, 50), (second.id, 50)];
        ReadProgressRepository::mark_series_as_read(&db, user.id, books.clone())
            .await
            .unwrap();
        assert_eq!(completion_count(&db, user.id, first.id).await, 1);
        assert_eq!(completion_count(&db, user.id, second.id).await, 1);

        ReadProgressRepository::mark_series_as_read(&db, user.id, books)
            .await
            .unwrap();
        assert_eq!(
            completion_count(&db, user.id, first.id).await,
            1,
            "re-marking an already-read series must not inflate the log"
        );
        assert_eq!(completion_count(&db, user.id, second.id).await, 1);
    }

    /// The series-level unread path also spares the log, and a subsequent
    /// re-read is counted.
    #[tokio::test]
    async fn mark_series_as_unread_preserves_history() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let first = create_test_book(&db).await;
        let second = create_test_book(&db).await;

        ReadProgressRepository::mark_series_as_read(
            &db,
            user.id,
            vec![(first.id, 50), (second.id, 50)],
        )
        .await
        .unwrap();

        ReadProgressRepository::mark_series_as_unread(&db, user.id, vec![first.id, second.id])
            .await
            .unwrap();

        assert_eq!(completion_count(&db, user.id, first.id).await, 1);
        assert_eq!(completion_count(&db, user.id, second.id).await, 1);

        // Re-read just the first volume.
        ReadProgressRepository::mark_as_read(&db, user.id, first.id, 50)
            .await
            .unwrap();
        assert_eq!(completion_count(&db, user.id, first.id).await, 2);
        assert_eq!(completion_count(&db, user.id, second.id).await, 1);
    }

    /// The banked row carries the pass's own dates, not just "now".
    #[tokio::test]
    async fn the_recorded_row_spans_the_pass() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        let started = ReadProgressRepository::upsert(&db, user.id, book.id, 1, false)
            .await
            .unwrap()
            .started_at;
        let finished = ReadProgressRepository::upsert(&db, user.id, book.id, 50, true)
            .await
            .unwrap();

        let entries =
            crate::repositories::ReadCompletionRepository::list_for_book(&db, user.id, book.id)
                .await
                .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].started_at, started);
        assert_eq!(entries[0].completed_at, finished.completed_at.unwrap());
    }

    /// Two users completing the same book each get their own entry.
    #[tokio::test]
    async fn completions_are_recorded_per_user() {
        let db = setup_test_db().await;
        let alice = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        let password_hash = password::hash_password("password").unwrap();
        let bob = UserRepository::create(
            &db,
            &users::Model {
                id: Uuid::new_v4(),
                username: "bob".to_string(),
                email: "bob@example.com".to_string(),
                password_hash,
                role: "admin".to_string(),
                is_active: true,
                email_verified: false,
                permissions: serde_json::json!([]),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                last_login_at: None,
            },
        )
        .await
        .unwrap();

        ReadProgressRepository::mark_as_read(&db, alice.id, book.id, 50)
            .await
            .unwrap();

        assert_eq!(completion_count(&db, alice.id, book.id).await, 1);
        assert_eq!(completion_count(&db, bob.id, book.id).await, 0);
    }
}
