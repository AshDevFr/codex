//! Repository for the append-only read-completion log.
//!
//! Every row is one finished read-through. Nothing here updates a row: the only
//! operations are recording a completion, reading history back, and deleting it
//! at one of three scopes.
//!
//! Most methods are generic over [`ConnectionTrait`] rather than taking a
//! `&DatabaseConnection`, so a completion can be recorded inside the same
//! transaction as the progress update that triggered it.

#![allow(dead_code)]

use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, JoinType, Order, QueryFilter,
    QueryOrder, QuerySelect, RelationTrait, Set,
};
use uuid::Uuid;

use crate::entities::{books, read_completions, read_completions::Entity as ReadCompletions};

/// One completed pass: `(started_at, completed_at)`.
pub type CompletionSpan = (DateTime<Utc>, DateTime<Utc>);

/// A series' completion history, derived from its books.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeriesHistory {
    /// Minimum completion count across the series' books.
    pub read_count: i64,
    /// When the series was last completed as a whole.
    pub last_completed_at: Option<DateTime<Utc>>,
    /// `(started_at, completed_at)` per pass, newest first.
    pub passes: Vec<CompletionSpan>,
}

pub struct ReadCompletionRepository;

impl ReadCompletionRepository {
    /// Record one finished read-through.
    ///
    /// Callers are responsible for deciding whether a completion is a new pass;
    /// this method always inserts. See the duplicate guard on the write path.
    pub async fn record<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_id: Uuid,
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
    ) -> Result<read_completions::Model> {
        let row = read_completions::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            book_id: Set(book_id),
            started_at: Set(started_at),
            completed_at: Set(completed_at),
        };
        Ok(row.insert(db).await?)
    }

    /// Whether a completion has already been banked for the current pass.
    ///
    /// The comparison is against the *progress row's* `started_at`, not against
    /// `completed` or `completed_at`. Those two are both cleared when a book is
    /// un-completed: tapping back one page from the end sets `completed = false`
    /// and wipes `completed_at`, so tapping forward again is indistinguishable
    /// from a first-ever completion and a guard reading either column would bank
    /// a second row for the same read-through.
    ///
    /// `started_at` is untouched by that bounce, so it delimits the pass
    /// correctly. Marking a book unread deletes the progress row entirely, so
    /// the next read starts a genuinely new pass with a later `started_at` and
    /// its completion records normally.
    pub async fn has_completion_since<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_id: Uuid,
        pass_started_at: DateTime<Utc>,
    ) -> Result<bool> {
        let existing = ReadCompletions::find()
            .filter(read_completions::Column::UserId.eq(user_id))
            .filter(read_completions::Column::BookId.eq(book_id))
            .filter(read_completions::Column::CompletedAt.gte(pass_started_at))
            .one(db)
            .await?;
        Ok(existing.is_some())
    }

    /// Completions for one book, newest first.
    pub async fn list_for_book<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Result<Vec<read_completions::Model>> {
        Ok(ReadCompletions::find()
            .filter(read_completions::Column::UserId.eq(user_id))
            .filter(read_completions::Column::BookId.eq(book_id))
            .order_by_desc(read_completions::Column::CompletedAt)
            // Stable tie-break so equal timestamps (possible when a series is
            // bulk-marked read) don't return in arbitrary order.
            .order_by(read_completions::Column::Id, Order::Asc)
            .all(db)
            .await?)
    }

    /// Completions for every book in one series, newest first.
    ///
    /// Joined through `books` so callers don't have to fetch the book list
    /// first. Ordering is done in the database.
    pub async fn list_for_series<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        series_id: Uuid,
    ) -> Result<Vec<read_completions::Model>> {
        Ok(ReadCompletions::find()
            .join(JoinType::InnerJoin, read_completions::Relation::Books.def())
            .filter(read_completions::Column::UserId.eq(user_id))
            .filter(books::Column::SeriesId.eq(series_id))
            .order_by_desc(read_completions::Column::CompletedAt)
            .order_by(read_completions::Column::Id, Order::Asc)
            .all(db)
            .await?)
    }

    /// How many times each of the given books has been completed.
    ///
    /// Books with no completions are absent from the map rather than present
    /// with a zero, so callers must treat a missing key as zero. That keeps the
    /// query to the rows that exist.
    pub async fn counts_for_books<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, i64>> {
        if book_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows: Vec<(Uuid, i64)> = ReadCompletions::find()
            .select_only()
            .column(read_completions::Column::BookId)
            .column_as(read_completions::Column::Id.count(), "count")
            .filter(read_completions::Column::UserId.eq(user_id))
            .filter(read_completions::Column::BookId.is_in(book_ids.to_vec()))
            .group_by(read_completions::Column::BookId)
            .into_tuple()
            .all(db)
            .await?;

        Ok(rows.into_iter().collect())
    }

    /// Completion count for one book.
    pub async fn count_for_book<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Result<i64> {
        Ok(Self::counts_for_books(db, user_id, &[book_id])
            .await?
            .get(&book_id)
            .copied()
            .unwrap_or(0))
    }

    /// Most recent completion date per book.
    ///
    /// Books with no completions are absent, same convention as
    /// [`Self::counts_for_books`].
    pub async fn last_completed_for_books<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, DateTime<Utc>>> {
        if book_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows: Vec<(Uuid, DateTime<Utc>)> = ReadCompletions::find()
            .select_only()
            .column(read_completions::Column::BookId)
            .column_as(
                read_completions::Column::CompletedAt.max(),
                "last_completed_at",
            )
            .filter(read_completions::Column::UserId.eq(user_id))
            .filter(read_completions::Column::BookId.is_in(book_ids.to_vec()))
            .group_by(read_completions::Column::BookId)
            .into_tuple()
            .all(db)
            .await?;

        Ok(rows.into_iter().collect())
    }

    /// How many times a whole series has been read, and the span of each pass.
    ///
    /// A series counts as read N times only once *every* book in it has been read
    /// N times, so the count is the minimum across its books. A series with no
    /// books, or with any book never completed, reports 0. Soft-deleted books are
    /// excluded: a file disappearing from disk should not make a finished series
    /// look unfinished forever.
    ///
    /// Pass N of the series runs from the earliest of its books' Nth start to the
    /// latest of their Nth finish, so the returned spans describe when the series
    /// as a whole was being read. Entries come back newest first, matching
    /// [`Self::list_for_book`].
    ///
    /// The completions are fetched with a single ordered query and folded in
    /// memory. This is one series' worth of rows rather than a paginated result
    /// set, so it does not conflict with the rule that pagination sorts in the
    /// database.
    pub async fn series_history<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        series_id: Uuid,
    ) -> Result<SeriesHistory> {
        let book_ids: Vec<Uuid> = books::Entity::find()
            .select_only()
            .column(books::Column::Id)
            .filter(books::Column::SeriesId.eq(series_id))
            .filter(books::Column::Deleted.eq(false))
            .into_tuple()
            .all(db)
            .await?;

        if book_ids.is_empty() {
            return Ok(SeriesHistory::default());
        }

        // Oldest-first per book, so index N is that book's Nth pass.
        let mut by_book: HashMap<Uuid, Vec<CompletionSpan>> = HashMap::new();
        let rows = ReadCompletions::find()
            .filter(read_completions::Column::UserId.eq(user_id))
            .filter(read_completions::Column::BookId.is_in(book_ids.clone()))
            .order_by_asc(read_completions::Column::CompletedAt)
            .order_by(read_completions::Column::Id, Order::Asc)
            .all(db)
            .await?;
        for row in rows {
            by_book
                .entry(row.book_id)
                .or_default()
                .push((row.started_at, row.completed_at));
        }

        // A book with no completions means the series has never been finished.
        let read_count = book_ids
            .iter()
            .map(|id| by_book.get(id).map_or(0, |passes| passes.len()))
            .min()
            .unwrap_or(0);

        let mut passes = Vec::with_capacity(read_count);
        for pass in 0..read_count {
            let nth: Vec<CompletionSpan> = book_ids
                .iter()
                .filter_map(|id| by_book.get(id).and_then(|list| list.get(pass)).copied())
                .collect();
            let started_at = nth.iter().map(|(s, _)| *s).min();
            let completed_at = nth.iter().map(|(_, c)| *c).max();
            if let (Some(started_at), Some(completed_at)) = (started_at, completed_at) {
                passes.push((started_at, completed_at));
            }
        }

        let last_completed_at = passes.last().map(|(_, c)| *c);
        // Newest first.
        passes.reverse();

        Ok(SeriesHistory {
            read_count: read_count as i64,
            last_completed_at,
            passes,
        })
    }

    /// Clear one book's history for one user. Returns the number of rows removed.
    pub async fn delete_for_book<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Result<u64> {
        let result = ReadCompletions::delete_many()
            .filter(read_completions::Column::UserId.eq(user_id))
            .filter(read_completions::Column::BookId.eq(book_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Remove one recorded read-through.
    ///
    /// For correcting a single wrong entry without discarding the rest of a
    /// book's history, which the wholesale clear cannot do.
    ///
    /// Scoped to the user and the book as well as the id: the id alone would be
    /// enough to find the row, but matching all three means a caller cannot
    /// reach another user's history even by guessing, and cannot delete from a
    /// book other than the one the request addressed.
    ///
    /// Returns whether a row was removed, so a caller can answer 404 for an id
    /// that does not exist rather than reporting a silent success.
    pub async fn delete_entry<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_id: Uuid,
        completion_id: Uuid,
    ) -> Result<bool> {
        let result = ReadCompletions::delete_many()
            .filter(read_completions::Column::Id.eq(completion_id))
            .filter(read_completions::Column::UserId.eq(user_id))
            .filter(read_completions::Column::BookId.eq(book_id))
            .exec(db)
            .await?;

        Ok(result.rows_affected > 0)
    }

    /// Clear the history of every book in a series for one user.
    ///
    /// The book ids are resolved first rather than joined: `DELETE ... JOIN` is
    /// not portable, and the id list for a single series is small.
    pub async fn delete_for_series<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        series_id: Uuid,
    ) -> Result<u64> {
        let book_ids: Vec<Uuid> = books::Entity::find()
            .select_only()
            .column(books::Column::Id)
            .filter(books::Column::SeriesId.eq(series_id))
            .into_tuple()
            .all(db)
            .await?;

        if book_ids.is_empty() {
            return Ok(0);
        }

        let result = ReadCompletions::delete_many()
            .filter(read_completions::Column::UserId.eq(user_id))
            .filter(read_completions::Column::BookId.is_in(book_ids))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Clear a user's entire completion history.
    pub async fn delete_all_for_user<C: ConnectionTrait>(db: &C, user_id: Uuid) -> Result<u64> {
        let result = ReadCompletions::delete_many()
            .filter(read_completions::Column::UserId.eq(user_id))
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
    use chrono::TimeDelta;
    use codex_models::ScanningStrategy;
    use codex_utils::password;
    use sea_orm::DatabaseConnection;

    async fn create_user(db: &DatabaseConnection, name: &str) -> users::Model {
        let password_hash = password::hash_password("password").unwrap();
        let user = users::Model {
            id: Uuid::new_v4(),
            username: name.to_string(),
            email: format!("{name}@example.com"),
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

    /// A library with one series, returned so tests can add books to it.
    async fn create_series(db: &DatabaseConnection, name: &str) -> (Uuid, Uuid) {
        let library = LibraryRepository::create(
            db,
            &format!("Library {name}"),
            &format!("/test/{name}"),
            ScanningStrategy::Default,
        )
        .await
        .unwrap();
        let series = SeriesRepository::create(db, library.id, name, None)
            .await
            .unwrap();
        (library.id, series.id)
    }

    async fn create_book(
        db: &DatabaseConnection,
        library_id: Uuid,
        series_id: Uuid,
    ) -> books::Model {
        let book = books::Model {
            id: Uuid::new_v4(),
            series_id,
            library_id,
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

    fn at(minutes: i64) -> DateTime<Utc> {
        Utc::now() + TimeDelta::minutes(minutes)
    }

    #[tokio::test]
    async fn record_and_count() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let (library_id, series_id) = create_series(&db, "Series").await;
        let book = create_book(&db, library_id, series_id).await;

        assert_eq!(
            ReadCompletionRepository::count_for_book(&db, user.id, book.id)
                .await
                .unwrap(),
            0,
            "a book with no completions counts zero"
        );

        ReadCompletionRepository::record(&db, user.id, book.id, at(-60), at(0))
            .await
            .unwrap();
        assert_eq!(
            ReadCompletionRepository::count_for_book(&db, user.id, book.id)
                .await
                .unwrap(),
            1
        );

        // Recording is unconditional: the caller owns the duplicate decision.
        ReadCompletionRepository::record(&db, user.id, book.id, at(10), at(20))
            .await
            .unwrap();
        assert_eq!(
            ReadCompletionRepository::count_for_book(&db, user.id, book.id)
                .await
                .unwrap(),
            2
        );
    }

    #[tokio::test]
    async fn list_for_book_is_newest_first() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let (library_id, series_id) = create_series(&db, "Series").await;
        let book = create_book(&db, library_id, series_id).await;

        // Inserted oldest-first so the ordering assertion can't pass by accident.
        ReadCompletionRepository::record(&db, user.id, book.id, at(-120), at(-100))
            .await
            .unwrap();
        ReadCompletionRepository::record(&db, user.id, book.id, at(-60), at(-50))
            .await
            .unwrap();
        ReadCompletionRepository::record(&db, user.id, book.id, at(-10), at(0))
            .await
            .unwrap();

        let entries = ReadCompletionRepository::list_for_book(&db, user.id, book.id)
            .await
            .unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries[0].completed_at > entries[1].completed_at);
        assert!(entries[1].completed_at > entries[2].completed_at);
    }

    #[tokio::test]
    async fn history_is_per_user() {
        let db = setup_test_db().await;
        let alice = create_user(&db, "alice").await;
        let bob = create_user(&db, "bob").await;
        let (library_id, series_id) = create_series(&db, "Series").await;
        let book = create_book(&db, library_id, series_id).await;

        ReadCompletionRepository::record(&db, alice.id, book.id, at(-60), at(0))
            .await
            .unwrap();

        assert_eq!(
            ReadCompletionRepository::count_for_book(&db, alice.id, book.id)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            ReadCompletionRepository::count_for_book(&db, bob.id, book.id)
                .await
                .unwrap(),
            0,
            "one user's completion must not appear in another's history"
        );
        assert!(
            ReadCompletionRepository::list_for_book(&db, bob.id, book.id)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn counts_for_books_omits_books_with_no_completions() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let (library_id, series_id) = create_series(&db, "Series").await;
        let read_twice = create_book(&db, library_id, series_id).await;
        let read_once = create_book(&db, library_id, series_id).await;
        let unread = create_book(&db, library_id, series_id).await;

        ReadCompletionRepository::record(&db, user.id, read_twice.id, at(-120), at(-100))
            .await
            .unwrap();
        ReadCompletionRepository::record(&db, user.id, read_twice.id, at(-60), at(-50))
            .await
            .unwrap();
        ReadCompletionRepository::record(&db, user.id, read_once.id, at(-60), at(-50))
            .await
            .unwrap();

        let counts = ReadCompletionRepository::counts_for_books(
            &db,
            user.id,
            &[read_twice.id, read_once.id, unread.id],
        )
        .await
        .unwrap();

        assert_eq!(counts.get(&read_twice.id), Some(&2));
        assert_eq!(counts.get(&read_once.id), Some(&1));
        assert_eq!(
            counts.get(&unread.id),
            None,
            "books with no completions are absent, not zero"
        );
    }

    #[tokio::test]
    async fn counts_for_books_handles_an_empty_id_list() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        assert!(
            ReadCompletionRepository::counts_for_books(&db, user.id, &[])
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn list_for_series_spans_its_books_only() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let (library_id, series_id) = create_series(&db, "Wanted").await;
        let (other_library, other_series) = create_series(&db, "Other").await;

        let first = create_book(&db, library_id, series_id).await;
        let second = create_book(&db, library_id, series_id).await;
        let unrelated = create_book(&db, other_library, other_series).await;

        ReadCompletionRepository::record(&db, user.id, first.id, at(-120), at(-100))
            .await
            .unwrap();
        ReadCompletionRepository::record(&db, user.id, second.id, at(-60), at(-50))
            .await
            .unwrap();
        ReadCompletionRepository::record(&db, user.id, unrelated.id, at(-10), at(0))
            .await
            .unwrap();

        let entries = ReadCompletionRepository::list_for_series(&db, user.id, series_id)
            .await
            .unwrap();
        assert_eq!(entries.len(), 2, "the unrelated series must not leak in");
        // Newest first.
        assert_eq!(entries[0].book_id, second.id);
        assert_eq!(entries[1].book_id, first.id);
    }

    #[tokio::test]
    async fn delete_for_book_is_scoped_to_that_book_and_user() {
        let db = setup_test_db().await;
        let alice = create_user(&db, "alice").await;
        let bob = create_user(&db, "bob").await;
        let (library_id, series_id) = create_series(&db, "Series").await;
        let target = create_book(&db, library_id, series_id).await;
        let sibling = create_book(&db, library_id, series_id).await;

        for book in [&target, &sibling] {
            ReadCompletionRepository::record(&db, alice.id, book.id, at(-60), at(0))
                .await
                .unwrap();
            ReadCompletionRepository::record(&db, bob.id, book.id, at(-60), at(0))
                .await
                .unwrap();
        }

        let removed = ReadCompletionRepository::delete_for_book(&db, alice.id, target.id)
            .await
            .unwrap();
        assert_eq!(removed, 1);

        assert_eq!(
            ReadCompletionRepository::count_for_book(&db, alice.id, target.id)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            ReadCompletionRepository::count_for_book(&db, alice.id, sibling.id)
                .await
                .unwrap(),
            1,
            "a sibling book keeps its history"
        );
        assert_eq!(
            ReadCompletionRepository::count_for_book(&db, bob.id, target.id)
                .await
                .unwrap(),
            1,
            "another user keeps their history for the same book"
        );
    }

    #[tokio::test]
    async fn delete_for_series_clears_every_book_in_it() {
        let db = setup_test_db().await;
        let alice = create_user(&db, "alice").await;
        let bob = create_user(&db, "bob").await;
        let (library_id, series_id) = create_series(&db, "Target").await;
        let (other_library, other_series) = create_series(&db, "Other").await;

        let first = create_book(&db, library_id, series_id).await;
        let second = create_book(&db, library_id, series_id).await;
        let unrelated = create_book(&db, other_library, other_series).await;

        for book in [&first, &second, &unrelated] {
            ReadCompletionRepository::record(&db, alice.id, book.id, at(-60), at(0))
                .await
                .unwrap();
            ReadCompletionRepository::record(&db, bob.id, book.id, at(-60), at(0))
                .await
                .unwrap();
        }

        let removed = ReadCompletionRepository::delete_for_series(&db, alice.id, series_id)
            .await
            .unwrap();
        assert_eq!(removed, 2);

        assert!(
            ReadCompletionRepository::list_for_series(&db, alice.id, series_id)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            ReadCompletionRepository::count_for_book(&db, alice.id, unrelated.id)
                .await
                .unwrap(),
            1,
            "another series is untouched"
        );
        assert_eq!(
            ReadCompletionRepository::list_for_series(&db, bob.id, series_id)
                .await
                .unwrap()
                .len(),
            2,
            "another user's history for the same series survives"
        );
    }

    #[tokio::test]
    async fn delete_for_series_on_a_series_with_no_books_is_a_noop() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let (_library_id, series_id) = create_series(&db, "Empty").await;

        assert_eq!(
            ReadCompletionRepository::delete_for_series(&db, user.id, series_id)
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn delete_all_for_user_spares_other_users() {
        let db = setup_test_db().await;
        let alice = create_user(&db, "alice").await;
        let bob = create_user(&db, "bob").await;
        let (library_id, series_id) = create_series(&db, "Series").await;
        let first = create_book(&db, library_id, series_id).await;
        let second = create_book(&db, library_id, series_id).await;

        for book in [&first, &second] {
            ReadCompletionRepository::record(&db, alice.id, book.id, at(-60), at(0))
                .await
                .unwrap();
            ReadCompletionRepository::record(&db, bob.id, book.id, at(-60), at(0))
                .await
                .unwrap();
        }

        let removed = ReadCompletionRepository::delete_all_for_user(&db, alice.id)
            .await
            .unwrap();
        assert_eq!(removed, 2);

        assert_eq!(
            ReadCompletionRepository::count_for_book(&db, alice.id, first.id)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            ReadCompletionRepository::count_for_book(&db, bob.id, first.id)
                .await
                .unwrap(),
            1
        );
    }

    /// The guard predicate the write path relies on: a completion recorded
    /// during the current pass suppresses a second one, while one from an
    /// earlier pass does not.
    #[tokio::test]
    async fn has_completion_since_delimits_the_pass() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let (library_id, series_id) = create_series(&db, "Series").await;
        let book = create_book(&db, library_id, series_id).await;

        let pass_started = at(0);

        assert!(
            !ReadCompletionRepository::has_completion_since(&db, user.id, book.id, pass_started)
                .await
                .unwrap(),
            "nothing recorded yet"
        );

        // A completion from an *earlier* pass must not suppress this one.
        ReadCompletionRepository::record(&db, user.id, book.id, at(-120), at(-100))
            .await
            .unwrap();
        assert!(
            !ReadCompletionRepository::has_completion_since(&db, user.id, book.id, pass_started)
                .await
                .unwrap(),
            "an older completion belongs to a previous read-through"
        );

        // One recorded during this pass must suppress it.
        ReadCompletionRepository::record(&db, user.id, book.id, pass_started, at(10))
            .await
            .unwrap();
        assert!(
            ReadCompletionRepository::has_completion_since(&db, user.id, book.id, pass_started)
                .await
                .unwrap()
        );
    }

    /// Deleting a book cascades its completion rows away.
    #[tokio::test]
    async fn completions_cascade_when_the_book_is_deleted() {
        let db = setup_test_db().await;
        let user = create_user(&db, "reader").await;
        let (library_id, series_id) = create_series(&db, "Series").await;
        let book = create_book(&db, library_id, series_id).await;

        ReadCompletionRepository::record(&db, user.id, book.id, at(-60), at(0))
            .await
            .unwrap();

        books::Entity::delete_by_id(book.id)
            .exec(&db)
            .await
            .unwrap();

        assert_eq!(
            ReadCompletionRepository::count_for_book(&db, user.id, book.id)
                .await
                .unwrap(),
            0
        );
    }
}
