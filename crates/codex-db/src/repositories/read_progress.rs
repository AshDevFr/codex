//! Repository for ReadProgress operations
//!
//! TODO: Remove allow(dead_code) when all reading progress features are fully integrated

#![allow(dead_code)]

use crate::entities::reading_sessions::SessionKind;
use crate::entities::{read_progress, read_progress::Entity as ReadProgress};
use crate::repositories::ReadCompletionRepository;
use crate::repositories::reading_sessions::{
    AppendOutcome, DeviceContext, NewSession, ReadingSessionRepository, fold,
};
use anyhow::{Result, anyhow};
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

    /// [`Self::upsert`] attributed to a device.
    ///
    /// For compatibility surfaces, whose requests carry enough to identify the
    /// client even though the protocol has no device concept.
    pub async fn upsert_with_device(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
        current_page: i32,
        completed: bool,
        device: &DeviceContext,
    ) -> Result<read_progress::Model> {
        Self::upsert_with_percentage_and_device(
            db,
            user_id,
            book_id,
            current_page,
            None,
            completed,
            None,
            device,
        )
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
        Self::upsert_with_percentage_and_device(
            db,
            user_id,
            book_id,
            current_page,
            progress_percentage,
            completed,
            r2_progression,
            &DeviceContext::legacy(),
        )
        .await
    }

    /// [`Self::upsert_with_percentage`] attributed to a device.
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_with_percentage_and_device(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
        current_page: i32,
        progress_percentage: Option<f64>,
        completed: bool,
        r2_progression: Option<String>,
        device: &DeviceContext,
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
            device,
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
                    device,
                )
                .await
            }
            other => other,
        }
    }

    /// One attempt at the upsert: append the event, then rebuild the projection
    /// from the log.
    ///
    /// Everything in one transaction. The session, the progress row it implies,
    /// and any completion it banks are one fact about what the reader did, and a
    /// process dying between them would leave the log and its projections
    /// disagreeing.
    #[allow(clippy::too_many_arguments)]
    async fn upsert_txn(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
        current_page: i32,
        progress_percentage: Option<f64>,
        completed: bool,
        r2_progression: Option<String>,
        device: &DeviceContext,
    ) -> Result<read_progress::Model> {
        let txn = db.begin().await?;
        let result = Self::upsert_in(
            &txn,
            user_id,
            book_id,
            current_page,
            progress_percentage,
            completed,
            r2_progression,
            device,
            Utc::now(),
        )
        .await?;
        txn.commit().await?;
        Ok(result)
    }

    /// The upsert itself, over a caller-supplied connection.
    ///
    /// Separate from [`Self::upsert_txn`] so a series-wide operation can run
    /// hundreds of these in one transaction rather than paying a commit per
    /// book. A single book still gets its own transaction through the wrapper.
    #[allow(clippy::too_many_arguments)]
    async fn upsert_in<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_id: Uuid,
        current_page: i32,
        progress_percentage: Option<f64>,
        completed: bool,
        r2_progression: Option<String>,
        device: &DeviceContext,
        now: chrono::DateTime<Utc>,
    ) -> Result<read_progress::Model> {
        let kind = if completed {
            SessionKind::Completed
        } else {
            SessionKind::Progress
        };
        let session = NewSession::from_legacy_write(user_id, book_id, device, kind, now)
            .with_page(current_page)
            .with_percentage(progress_percentage)
            .with_progression(r2_progression);

        ReadingSessionRepository::append(db, session, now).await?;
        let result = Self::refold(db, user_id, book_id).await?;

        // A write that reports a position always leaves a row behind, so the
        // fold cannot legitimately come back empty here.
        result.ok_or_else(|| {
            anyhow!("reading progress vanished after recording a session for book {book_id}")
        })
    }

    /// Record a client-measured session and rebuild the projections from it.
    ///
    /// The entry point for the sessions API. Everything runs in one transaction
    /// so the log and its projections cannot disagree, and a replayed id is
    /// reported rather than applied twice.
    pub async fn record_session(
        db: &DatabaseConnection,
        session: NewSession,
    ) -> Result<(AppendOutcome, Option<read_progress::Model>)> {
        let txn = db.begin().await?;
        let now = Utc::now();
        let user_id = session.user_id;
        let book_id = session.book_id;

        let outcome = ReadingSessionRepository::append(&txn, session, now).await?;

        // A duplicate changed nothing, but the caller still wants the current
        // state so it can reconcile without a second request.
        let progress = if outcome == AppendOutcome::Duplicate {
            Self::get_in(&txn, user_id, book_id).await?
        } else {
            Self::refold(&txn, user_id, book_id).await?
        };

        txn.commit().await?;
        Ok((outcome, progress))
    }

    /// Rebuild `read_progress` and `read_completions` for one user and book from
    /// the session log.
    ///
    /// Returns the projected row, or `None` when the log says there should not
    /// be one. That happens after a reset with no reading since: marking a book
    /// unread has always removed the row outright rather than zeroing it, and
    /// callers check for its absence.
    async fn refold<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Result<Option<read_progress::Model>> {
        // Only the current pass: the fold discards earlier ones, and loading
        // them would make every write cost more as a book is re-read.
        let sessions = ReadingSessionRepository::load_current_pass(db, user_id, book_id).await?;
        let folded = fold(&sessions);

        let Some(projected) = folded.progress else {
            Self::delete_row_in(db, user_id, book_id).await?;
            return Ok(None);
        };

        let existing = Self::get_in(db, user_id, book_id).await?;
        // Reuse the existing row's identity. Callers compare progress IDs across
        // writes to prove an update did not create a duplicate.
        let id = existing
            .as_ref()
            .map(|row| row.id)
            .unwrap_or_else(Uuid::new_v4);

        let model = read_progress::ActiveModel {
            id: Set(id),
            user_id: Set(user_id),
            book_id: Set(book_id),
            current_page: Set(projected.current_page),
            progress_percentage: Set(projected.progress_percentage),
            completed: Set(projected.completed),
            started_at: Set(projected.started_at),
            updated_at: Set(projected.updated_at),
            completed_at: Set(projected.completed_at),
            r2_progression: Set(projected.r2_progression),
        };

        let saved = if existing.is_some() {
            model.update(db).await?
        } else {
            model.insert(db).await?
        };

        if let Some(completion) = folded.completion {
            Self::record_completion_if_new(
                db,
                user_id,
                book_id,
                completion.started_at,
                completion.completed_at,
            )
            .await?;
        }

        Ok(Some(saved))
    }

    /// Append a reset, which opens a new pass and so drops the progress row.
    ///
    /// Recording this as an event rather than deleting the row is what makes the
    /// ordering between "I finished this" and "I am starting over" recorded
    /// data. Two offline clients replaying those in either order then reach the
    /// same completion count.
    async fn reset_txn(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
        device: &DeviceContext,
    ) -> Result<bool> {
        let txn = db.begin().await?;
        let existed = Self::reset_in(&txn, user_id, book_id, device, Utc::now()).await?;
        txn.commit().await?;
        Ok(existed)
    }

    /// The reset itself, over a caller-supplied connection.
    ///
    /// Separate from [`Self::reset_txn`] for the same reason as the upsert: a
    /// series of several hundred volumes should cost one commit, not one per
    /// book.
    async fn reset_in<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_id: Uuid,
        device: &DeviceContext,
        now: chrono::DateTime<Utc>,
    ) -> Result<bool> {
        let existed = Self::get_in(db, user_id, book_id).await?.is_some();

        let session =
            NewSession::from_legacy_write(user_id, book_id, device, SessionKind::Reset, now);
        ReadingSessionRepository::append(db, session, now).await?;
        Self::refold(db, user_id, book_id).await?;

        Ok(existed)
    }

    /// Remove the projection row itself, without touching the log.
    async fn delete_row_in<C: ConnectionTrait>(db: &C, user_id: Uuid, book_id: Uuid) -> Result<()> {
        ReadProgress::delete_many()
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::BookId.eq(book_id))
            .exec(db)
            .await?;

        Ok(())
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

    /// Delete reading progress, starting a new pass.
    ///
    /// This is the same operation as marking a book unread: it clears where the
    /// reader is without erasing that the book was read. The completion log
    /// survives, and the next completion counts as a fresh read-through.
    pub async fn delete(db: &DatabaseConnection, user_id: Uuid, book_id: Uuid) -> Result<()> {
        Self::delete_with_device(db, user_id, book_id, &DeviceContext::legacy()).await
    }

    /// [`Self::delete`] attributed to a device.
    pub async fn delete_with_device(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
        device: &DeviceContext,
    ) -> Result<()> {
        Self::reset_txn(db, user_id, book_id, device).await?;
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
        Self::mark_series_as_read_with_device(db, user_id, book_ids, &DeviceContext::legacy()).await
    }

    /// [`Self::mark_series_as_read`] attributed to a device.
    pub async fn mark_series_as_read_with_device(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_ids: Vec<(Uuid, i32)>,
        device: &DeviceContext,
    ) -> Result<usize> {
        match Self::mark_series_as_read_txn(db, user_id, &book_ids, device).await {
            Err(e) if Self::is_unique_violation(&e) => {
                // A concurrent writer poisoned the batch. The whole series rolled
                // back, so retrying is a clean re-run rather than a partial redo.
                Self::mark_series_as_read_txn(db, user_id, &book_ids, device).await
            }
            other => other,
        }
    }

    /// One transaction for the whole series.
    ///
    /// Each book still gets its own event in the log, because a later
    /// completion has to be recognisable as a genuine re-read per book. What is
    /// shared is the commit: a series can run to several hundred volumes, and a
    /// commit each turns one click into hundreds of round trips and fsyncs.
    ///
    /// All of them carry the same timestamp, which is also the more honest
    /// description of what happened: the reader marked the series in one go.
    async fn mark_series_as_read_txn(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_ids: &[(Uuid, i32)],
        device: &DeviceContext,
    ) -> Result<usize> {
        let txn = db.begin().await?;
        let now = Utc::now();
        let mut count = 0;

        // page_count is 1-indexed, so the last page is the page count itself.
        for &(book_id, page_count) in book_ids {
            Self::upsert_in(
                &txn, user_id, book_id, page_count, None, true, None, device, now,
            )
            .await?;
            count += 1;
        }

        txn.commit().await?;
        Ok(count)
    }

    /// Mark all books in a series as unread for a user
    /// Returns the number of books that had progress to clear
    ///
    /// Each book resets individually rather than in one bulk delete, because
    /// each needs its own reset event in the log so that a later completion is
    /// recognised as a genuine re-read. Books with no progress are still
    /// visited, but only those that had a row count toward the result.
    pub async fn mark_series_as_unread(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_ids: Vec<Uuid>,
    ) -> Result<u64> {
        Self::mark_series_as_unread_with_device(db, user_id, book_ids, &DeviceContext::legacy())
            .await
    }

    /// [`Self::mark_series_as_unread`] attributed to a device.
    pub async fn mark_series_as_unread_with_device(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_ids: Vec<Uuid>,
        device: &DeviceContext,
    ) -> Result<u64> {
        match Self::mark_series_as_unread_txn(db, user_id, &book_ids, device).await {
            Err(e) if Self::is_unique_violation(&e) => {
                Self::mark_series_as_unread_txn(db, user_id, &book_ids, device).await
            }
            other => other,
        }
    }

    /// One transaction for the whole series. See
    /// [`Self::mark_series_as_read_txn`] for why the commit is shared while the
    /// events are not.
    async fn mark_series_as_unread_txn(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_ids: &[Uuid],
        device: &DeviceContext,
    ) -> Result<u64> {
        let txn = db.begin().await?;
        let now = Utc::now();
        let mut cleared = 0;

        for &book_id in book_ids {
            if Self::reset_in(&txn, user_id, book_id, device, now).await? {
                cleared += 1;
            }
        }

        txn.commit().await?;
        Ok(cleared)
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

    // ========================================================================
    // The session log behind the projection
    //
    // `read_progress` is now derived, so these assert on the log itself: that
    // writes land in it, that page-by-page writes do not inflate it, and that
    // resets are recorded rather than erasing history.
    // ========================================================================

    async fn sessions_for(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Vec<crate::entities::reading_sessions::Model> {
        ReadingSessionRepository::load_for_book(db, user_id, book_id)
            .await
            .unwrap()
    }

    /// A write appends a session carrying the position it reported.
    #[tokio::test]
    async fn a_progress_write_appends_a_session() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        ReadProgressRepository::upsert(&db, user.id, book.id, 10, false)
            .await
            .unwrap();

        let sessions = sessions_for(&db, user.id, book.id).await;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].to_page, Some(10));
        assert_eq!(sessions[0].pass, 1);
        assert_eq!(sessions[0].device_id, "legacy");
    }

    /// Page-by-page writes from one device merge instead of accumulating a row
    /// per page. OPDS page streaming writes on every turn, so without this the
    /// log would grow without bound during a single sitting.
    #[tokio::test]
    async fn consecutive_page_writes_coalesce_into_one_session() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        for page in 1..=25 {
            ReadProgressRepository::upsert(&db, user.id, book.id, page, false)
                .await
                .unwrap();
        }

        let sessions = sessions_for(&db, user.id, book.id).await;
        assert_eq!(
            sessions.len(),
            1,
            "25 page turns in one sitting must be one session, not 25"
        );
        assert_eq!(sessions[0].to_page, Some(25));

        let progress = ReadProgressRepository::get_by_user_and_book(&db, user.id, book.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(progress.current_page, 25);
    }

    /// A completion is its own row. Folding it into neighbouring reading would
    /// erase the transition the fold needs to see.
    #[tokio::test]
    async fn a_completion_is_not_merged_into_reading() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        ReadProgressRepository::upsert(&db, user.id, book.id, 10, false)
            .await
            .unwrap();
        ReadProgressRepository::upsert(&db, user.id, book.id, 50, true)
            .await
            .unwrap();

        let sessions = sessions_for(&db, user.id, book.id).await;
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[1].session_kind(), SessionKind::Completed);
    }

    /// Marking unread records a reset and opens a new pass. The earlier
    /// sessions stay, so the history of what was read survives.
    #[tokio::test]
    async fn marking_unread_records_a_reset_and_keeps_history() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        ReadProgressRepository::upsert(&db, user.id, book.id, 50, true)
            .await
            .unwrap();
        ReadProgressRepository::mark_as_unread(&db, user.id, book.id)
            .await
            .unwrap();

        let sessions = sessions_for(&db, user.id, book.id).await;
        assert_eq!(sessions.len(), 2, "the reset is appended, not substituted");
        assert_eq!(sessions[1].session_kind(), SessionKind::Reset);
        assert_eq!(sessions[1].pass, 2, "a reset opens the next pass");

        // And the projection is gone, which is what callers check for.
        assert!(
            ReadProgressRepository::get_by_user_and_book(&db, user.id, book.id)
                .await
                .unwrap()
                .is_none()
        );
    }

    /// Reading after a reset belongs to the new pass, so its eventual
    /// completion is recognised as a separate read-through.
    #[tokio::test]
    async fn reading_after_a_reset_joins_the_new_pass() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        ReadProgressRepository::upsert(&db, user.id, book.id, 50, true)
            .await
            .unwrap();
        ReadProgressRepository::mark_as_unread(&db, user.id, book.id)
            .await
            .unwrap();
        ReadProgressRepository::upsert(&db, user.id, book.id, 5, false)
            .await
            .unwrap();

        let sessions = sessions_for(&db, user.id, book.id).await;
        let last = sessions.last().unwrap();
        assert_eq!(last.pass, 2);
        assert_eq!(last.to_page, Some(5));

        let progress = ReadProgressRepository::get_by_user_and_book(&db, user.id, book.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(progress.current_page, 5);
        assert!(!progress.completed);
    }

    /// Nothing measured reading time on these paths, so the log says so rather
    /// than recording a zero that statistics would later treat as real.
    #[tokio::test]
    async fn legacy_writes_report_no_measured_duration() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        ReadProgressRepository::upsert(&db, user.id, book.id, 10, false)
            .await
            .unwrap();

        let sessions = sessions_for(&db, user.id, book.id).await;
        assert_eq!(sessions[0].active_duration_ms, None);
        assert_eq!(
            sessions[0].duration_source(),
            crate::entities::reading_sessions::DurationSource::Unknown
        );
    }

    // ========================================================================
    // Device attribution and reconstructed reading time
    //
    // The compatibility surfaces report a position and nothing else. These
    // cover what can be rebuilt from that.
    // ========================================================================

    /// A device-attributed write is stored against that device, not the
    /// anonymous catch-all.
    #[tokio::test]
    async fn a_device_attributed_write_records_its_device() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        let device = DeviceContext::koreader("kobo-1", "Kobo Clara");
        ReadProgressRepository::upsert_with_device(&db, user.id, book.id, 10, false, &device)
            .await
            .unwrap();

        let sessions = sessions_for(&db, user.id, book.id).await;
        assert_eq!(sessions[0].device_id, "koreader:kobo-1");
        assert_eq!(sessions[0].device_name.as_deref(), Some("Kobo Clara"));
    }

    /// Successive writes from an inferring device accrue the gaps between them
    /// as reading time. This is what makes reading through Komga apps, OPDS
    /// readers and KOReader visible in the statistics at all.
    #[tokio::test]
    async fn successive_writes_from_a_compat_client_accrue_reading_time() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        let device = DeviceContext::user_agent("Komic/1.0");
        for page in 1..=5 {
            ReadProgressRepository::upsert_with_device(&db, user.id, book.id, page, false, &device)
                .await
                .unwrap();
        }

        let sessions = sessions_for(&db, user.id, book.id).await;
        assert_eq!(sessions.len(), 1, "the writes coalesce into one session");
        assert_eq!(
            sessions[0].duration_source(),
            crate::entities::reading_sessions::DurationSource::Inferred,
            "time rebuilt from request gaps must never masquerade as measured"
        );
        assert!(
            sessions[0].active_duration_ms.is_some(),
            "consecutive page turns must contribute some reconstructed time"
        );
    }

    /// Writes from a client that reports its own sessions never contribute
    /// inferred time. The web reader writes progress *and* posts measured
    /// sessions for the same reading; inferring here too would count it twice.
    #[tokio::test]
    async fn writes_from_a_session_reporting_client_infer_nothing() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        for page in 1..=5 {
            ReadProgressRepository::upsert(&db, user.id, book.id, page, false)
                .await
                .unwrap();
        }

        let sessions = sessions_for(&db, user.id, book.id).await;
        assert_eq!(sessions[0].active_duration_ms, None);
        assert_eq!(
            sessions[0].duration_source(),
            crate::entities::reading_sessions::DurationSource::Unknown
        );
    }

    /// Two clients reading the same book stay separable, which is the whole
    /// point of attributing writes to a device.
    #[tokio::test]
    async fn different_devices_produce_separate_sessions() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        ReadProgressRepository::upsert_with_device(
            &db,
            user.id,
            book.id,
            10,
            false,
            &DeviceContext::user_agent("Komic/1.0"),
        )
        .await
        .unwrap();
        ReadProgressRepository::upsert_with_device(
            &db,
            user.id,
            book.id,
            20,
            false,
            &DeviceContext::koreader("kobo-1", "Kobo"),
        )
        .await
        .unwrap();

        let sessions = sessions_for(&db, user.id, book.id).await;
        assert_eq!(sessions.len(), 2, "one session per device, not one merged");

        let progress = ReadProgressRepository::get_by_user_and_book(&db, user.id, book.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            progress.current_page, 20,
            "position still folds to one value"
        );
    }

    /// How long a write takes once a book has accumulated history.
    ///
    /// Every write refolds from the log, so the cost of writing grows with the
    /// size of the log unless something bounds what is loaded. Run with
    /// `cargo test -p codex-db --lib refold_cost -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "benchmark, not a correctness check"]
    async fn refold_cost_against_a_large_log() {
        use crate::entities::reading_sessions;
        use chrono::Duration as ChronoDuration;

        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;
        let base = Utc::now() - ChronoDuration::days(400);

        // A pathological but reachable shape: a book re-read many times, each
        // pass leaving sessions behind. Passes accumulate for the life of the
        // book, so this is what the log looks like years in.
        for pass in 1..=50i32 {
            let mut rows = Vec::new();
            for i in 0..40i32 {
                let at = base + ChronoDuration::minutes((pass * 100 + i) as i64);
                rows.push(reading_sessions::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    user_id: Set(user.id),
                    book_id: Set(book.id),
                    device_id: Set(format!("bench-device-{}", i % 3)),
                    device_name: Set(None),
                    pass: Set(pass),
                    kind: Set("progress".to_string()),
                    to_page: Set(Some(i)),
                    to_percentage: Set(None),
                    r2_progression: Set(None),
                    active_duration_ms: Set(Some(60_000)),
                    duration_source: Set("measured".to_string()),
                    pages_read: Set(Some(1)),
                    client_started_at: Set(at),
                    client_ended_at: Set(at),
                    server_recorded_at: Set(at),
                });
            }
            reading_sessions::Entity::insert_many(rows)
                .exec(&db)
                .await
                .unwrap();
        }

        let total = ReadingSessionRepository::load_for_book(&db, user.id, book.id)
            .await
            .unwrap()
            .len();

        let started = std::time::Instant::now();
        for page in 1..=20 {
            ReadProgressRepository::upsert(&db, user.id, book.id, page, false)
                .await
                .unwrap();
        }
        let per_write = started.elapsed() / 20;

        println!("sessions in log: {total}");
        println!("mean write (append + refold + persist): {per_write:?}");

        // Measured at ~28ms when the refold loaded every pass, and ~1.8ms once
        // it loaded only the current one, on a debug build. The ceiling is set
        // well above the latter so the test reports a regression in the loading
        // strategy rather than noise on a slow machine.
        assert!(
            per_write < std::time::Duration::from_millis(10),
            "a write took {per_write:?} against {total} sessions; \
             the refold is loading more than the current pass again"
        );
    }

    // ========================================================================
    // A measured session superseding the position writes it covers
    //
    // A client that measures its own sessions still writes progress as it
    // reads, because the projection has to stay live before the session
    // closes. These cover the arriving session absorbing those writes.
    // ========================================================================

    fn measured_session(
        user_id: Uuid,
        book_id: Uuid,
        device: &str,
        page: i32,
        started: chrono::DateTime<Utc>,
        ended: chrono::DateTime<Utc>,
    ) -> NewSession {
        NewSession::from_client(
            Uuid::new_v4(),
            user_id,
            book_id,
            device,
            Some("Test Browser".to_string()),
            SessionKind::Progress,
            Some(60_000),
            Some(10),
            started,
            ended,
        )
        .with_page(page)
    }

    /// The whole point: one sitting leaves one row, not one per page turn plus
    /// one for the session.
    #[tokio::test]
    async fn a_measured_session_absorbs_the_position_writes_it_covers() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        let device = DeviceContext::session_reporting_client("browser-1");
        let started = Utc::now();
        for page in 1..=20 {
            ReadProgressRepository::upsert_with_device(&db, user.id, book.id, page, false, &device)
                .await
                .unwrap();
        }
        assert!(!sessions_for(&db, user.id, book.id).await.is_empty());

        ReadProgressRepository::record_session(
            &db,
            measured_session(
                user.id,
                book.id,
                "browser-1",
                20,
                started - chrono::Duration::minutes(1),
                Utc::now() + chrono::Duration::minutes(1),
            ),
        )
        .await
        .unwrap();

        let sessions = sessions_for(&db, user.id, book.id).await;
        assert_eq!(sessions.len(), 1, "one sitting must leave one session");
        assert_eq!(
            sessions[0].active_duration_ms,
            Some(60_000),
            "the surviving row is the measured one"
        );
    }

    /// A page turn between the last position write and the session close must
    /// not be lost. Losing a redundant row is fine; losing the reader's place
    /// is not.
    #[tokio::test]
    async fn absorbing_never_moves_the_position_backwards() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        let device = DeviceContext::session_reporting_client("browser-1");
        let started = Utc::now();
        ReadProgressRepository::upsert_with_device(&db, user.id, book.id, 42, false, &device)
            .await
            .unwrap();

        // The measured session reports an earlier page than the last write.
        ReadProgressRepository::record_session(
            &db,
            measured_session(
                user.id,
                book.id,
                "browser-1",
                30,
                started - chrono::Duration::minutes(1),
                Utc::now() + chrono::Duration::minutes(1),
            ),
        )
        .await
        .unwrap();

        let progress = ReadProgressRepository::get_by_user_and_book(&db, user.id, book.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(progress.current_page, 42, "the furthest position survives");
    }

    /// Scoped to one device: another client's writes are not swept up.
    #[tokio::test]
    async fn absorbing_leaves_other_devices_alone() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        let started = Utc::now();
        ReadProgressRepository::upsert_with_device(
            &db,
            user.id,
            book.id,
            5,
            false,
            &DeviceContext::session_reporting_client("browser-1"),
        )
        .await
        .unwrap();
        ReadProgressRepository::upsert_with_device(
            &db,
            user.id,
            book.id,
            9,
            false,
            &DeviceContext::session_reporting_client("browser-2"),
        )
        .await
        .unwrap();

        ReadProgressRepository::record_session(
            &db,
            measured_session(
                user.id,
                book.id,
                "browser-1",
                9,
                started - chrono::Duration::minutes(1),
                Utc::now() + chrono::Duration::minutes(1),
            ),
        )
        .await
        .unwrap();

        let sessions = sessions_for(&db, user.id, book.id).await;
        assert_eq!(sessions.len(), 2, "browser-2's write must survive");
        assert!(sessions.iter().any(|s| s.device_id == "browser-2"));
    }

    /// Reconstructed time is real data, not a redundant position write, so it
    /// is never swept up even from the same device.
    #[tokio::test]
    async fn absorbing_leaves_reconstructed_sessions_alone() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        let started = Utc::now();
        let compat = DeviceContext {
            id: "browser-1".to_string(),
            name: None,
            infer_duration: true,
        };
        for page in 1..=3 {
            ReadProgressRepository::upsert_with_device(&db, user.id, book.id, page, false, &compat)
                .await
                .unwrap();
        }

        ReadProgressRepository::record_session(
            &db,
            measured_session(
                user.id,
                book.id,
                "browser-1",
                3,
                started - chrono::Duration::minutes(1),
                Utc::now() + chrono::Duration::minutes(1),
            ),
        )
        .await
        .unwrap();

        let sessions = sessions_for(&db, user.id, book.id).await;
        assert_eq!(sessions.len(), 2, "an inferred session is not redundant");
    }

    /// A write made after the session closed belongs to the next sitting and
    /// is outside the measured span, so it stays.
    #[tokio::test]
    async fn absorbing_ignores_writes_outside_the_session_span() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        let device = DeviceContext::session_reporting_client("browser-1");
        ReadProgressRepository::upsert_with_device(&db, user.id, book.id, 5, false, &device)
            .await
            .unwrap();

        // A session that ended before that write ever happened.
        ReadProgressRepository::record_session(
            &db,
            measured_session(
                user.id,
                book.id,
                "browser-1",
                4,
                Utc::now() - chrono::Duration::hours(3),
                Utc::now() - chrono::Duration::hours(2),
            ),
        )
        .await
        .unwrap();

        let sessions = sessions_for(&db, user.id, book.id).await;
        assert_eq!(sessions.len(), 2);
    }

    /// An EPUB locator only ever arrives on a position write, so a measured
    /// session that has none must inherit it rather than drop it.
    #[tokio::test]
    async fn absorbing_carries_forward_an_epub_locator() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        let device = DeviceContext::session_reporting_client("browser-1");
        let started = Utc::now();
        ReadProgressRepository::upsert_with_percentage_and_device(
            &db,
            user.id,
            book.id,
            5,
            Some(0.4),
            false,
            Some(r#"{"locator":"chapter-3"}"#.to_string()),
            &device,
        )
        .await
        .unwrap();

        ReadProgressRepository::record_session(
            &db,
            measured_session(
                user.id,
                book.id,
                "browser-1",
                5,
                started - chrono::Duration::minutes(1),
                Utc::now() + chrono::Duration::minutes(1),
            ),
        )
        .await
        .unwrap();

        let progress = ReadProgressRepository::get_by_user_and_book(&db, user.id, book.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            progress.r2_progression.as_deref(),
            Some(r#"{"locator":"chapter-3"}"#),
            "the reading position in an EPUB must survive the sweep"
        );
    }

    /// What marking a very long series costs.
    ///
    /// Sized from a real library whose longest series runs to 800 volumes.
    /// Both operations became per-book when progress moved onto the session
    /// log, so this is the check that the convenience of one click has not
    /// turned into a request that times out.
    ///
    /// Run with
    /// `cargo test -p codex-db --lib marking_a_long_series -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "benchmark, not a correctness check"]
    async fn marking_a_long_series_stays_responsive() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        let library =
            LibraryRepository::create(&db, "Bench Library", "/bench", ScanningStrategy::Default)
                .await
                .unwrap();
        let series = SeriesRepository::create(&db, library.id, "Long Series", None)
            .await
            .unwrap();

        let mut books = Vec::new();
        for _ in 0..800 {
            let book = books::Model {
                id: Uuid::new_v4(),
                series_id: series.id,
                library_id: library.id,
                path: format!("/bench/{}.cbz", Uuid::new_v4()),
                file_name: "v.cbz".to_string(),
                file_size: 1024,
                file_hash: format!("hash_{}", Uuid::new_v4()),
                partial_hash: String::new(),
                format: "cbz".to_string(),
                page_count: 20,
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
            books.push(BookRepository::create(&db, &book, None).await.unwrap());
        }

        let as_read: Vec<(Uuid, i32)> = books.iter().map(|b| (b.id, b.page_count)).collect();
        let ids: Vec<Uuid> = books.iter().map(|b| b.id).collect();

        let started = std::time::Instant::now();
        ReadProgressRepository::mark_series_as_read(&db, user.id, as_read)
            .await
            .unwrap();
        let read_elapsed = started.elapsed();

        let started = std::time::Instant::now();
        ReadProgressRepository::mark_series_as_unread(&db, user.id, ids)
            .await
            .unwrap();
        let unread_elapsed = started.elapsed();

        println!("books in series:      {}", books.len());
        println!("mark series read:     {read_elapsed:?}");
        println!("mark series unread:   {unread_elapsed:?}");
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
