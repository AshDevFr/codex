//! Repository for the per-user want-to-read queue.
//!
//! Each row flags exactly one series OR one book a user intends to read. The
//! queue is personal: every method scopes to a `user_id`.

#![allow(dead_code)]

use std::collections::HashSet;

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter,
    QueryOrder, Set,
};
use uuid::Uuid;

use crate::entities::{want_to_read, want_to_read::Entity as WantToRead};
use codex_models::sort::WantToReadSort;

/// Repository for want-to-read operations.
pub struct WantToReadRepository;

impl WantToReadRepository {
    /// Flag a series for a user. Idempotent: returns the existing row if already
    /// queued.
    pub async fn add_series(
        db: &DatabaseConnection,
        user_id: Uuid,
        series_id: Uuid,
    ) -> Result<want_to_read::Model> {
        if let Some(existing) = WantToRead::find()
            .filter(want_to_read::Column::UserId.eq(user_id))
            .filter(want_to_read::Column::SeriesId.eq(series_id))
            .one(db)
            .await?
        {
            return Ok(existing);
        }
        let model = want_to_read::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            series_id: Set(Some(series_id)),
            book_id: Set(None),
            added_at: Set(Utc::now()),
            position: Set(Self::next_position(db, user_id).await?),
        };
        Ok(model.insert(db).await?)
    }

    /// Flag a book for a user. Idempotent.
    pub async fn add_book(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Result<want_to_read::Model> {
        if let Some(existing) = WantToRead::find()
            .filter(want_to_read::Column::UserId.eq(user_id))
            .filter(want_to_read::Column::BookId.eq(book_id))
            .one(db)
            .await?
        {
            return Ok(existing);
        }
        let model = want_to_read::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            series_id: Set(None),
            book_id: Set(Some(book_id)),
            added_at: Set(Utc::now()),
            position: Set(Self::next_position(db, user_id).await?),
        };
        Ok(model.insert(db).await?)
    }

    /// Flag many series for a user in one batch. Idempotent: series already in
    /// the queue are skipped. Returns the number of rows newly inserted.
    ///
    /// Callers are responsible for ensuring the IDs reference existing series;
    /// invalid IDs would violate the foreign key and fail the whole batch.
    pub async fn add_series_bulk(
        db: &DatabaseConnection,
        user_id: Uuid,
        series_ids: &[Uuid],
    ) -> Result<usize> {
        if series_ids.is_empty() {
            return Ok(0);
        }
        let already = Self::series_ids_in_queue(db, user_id, series_ids).await?;
        let now = Utc::now();
        let next = Self::next_position(db, user_id).await?;
        let mut seen = HashSet::new();
        let models: Vec<want_to_read::ActiveModel> = series_ids
            .iter()
            .copied()
            .filter(|id| !already.contains(id) && seen.insert(*id))
            .enumerate()
            .map(|(offset, series_id)| want_to_read::ActiveModel {
                id: Set(Uuid::new_v4()),
                user_id: Set(user_id),
                series_id: Set(Some(series_id)),
                book_id: Set(None),
                added_at: Set(now),
                position: Set(next + offset as i32),
            })
            .collect();
        let added = models.len();
        if added > 0 {
            WantToRead::insert_many(models).exec(db).await?;
        }
        Ok(added)
    }

    /// Flag many books for a user in one batch. Idempotent: books already in the
    /// queue are skipped. Returns the number of rows newly inserted.
    ///
    /// Callers are responsible for ensuring the IDs reference existing books.
    pub async fn add_books_bulk(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_ids: &[Uuid],
    ) -> Result<usize> {
        if book_ids.is_empty() {
            return Ok(0);
        }
        let already = Self::book_ids_in_queue(db, user_id, book_ids).await?;
        let now = Utc::now();
        let next = Self::next_position(db, user_id).await?;
        let mut seen = HashSet::new();
        let models: Vec<want_to_read::ActiveModel> = book_ids
            .iter()
            .copied()
            .filter(|id| !already.contains(id) && seen.insert(*id))
            .enumerate()
            .map(|(offset, book_id)| want_to_read::ActiveModel {
                id: Set(Uuid::new_v4()),
                user_id: Set(user_id),
                series_id: Set(None),
                book_id: Set(Some(book_id)),
                added_at: Set(now),
                position: Set(next + offset as i32),
            })
            .collect();
        let added = models.len();
        if added > 0 {
            WantToRead::insert_many(models).exec(db).await?;
        }
        Ok(added)
    }

    /// Remove a series from a user's queue. Returns whether a row was removed.
    pub async fn remove_series(
        db: &DatabaseConnection,
        user_id: Uuid,
        series_id: Uuid,
    ) -> Result<bool> {
        let result = WantToRead::delete_many()
            .filter(want_to_read::Column::UserId.eq(user_id))
            .filter(want_to_read::Column::SeriesId.eq(series_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// Remove a book from a user's queue. Returns whether a row was removed.
    pub async fn remove_book(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Result<bool> {
        let result = WantToRead::delete_many()
            .filter(want_to_read::Column::UserId.eq(user_id))
            .filter(want_to_read::Column::BookId.eq(book_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// List a user's queue.
    ///
    /// `Newest`/`Oldest` order by add time; `Custom` orders by the manual
    /// `position` (with `added_at` as the tie-break for rows never reordered).
    pub async fn list(
        db: &DatabaseConnection,
        user_id: Uuid,
        sort: WantToReadSort,
    ) -> Result<Vec<want_to_read::Model>> {
        let query = WantToRead::find().filter(want_to_read::Column::UserId.eq(user_id));
        let query = match sort {
            WantToReadSort::Newest => query.order_by_desc(want_to_read::Column::AddedAt),
            WantToReadSort::Oldest => query.order_by_asc(want_to_read::Column::AddedAt),
            WantToReadSort::Custom => query
                .order_by_asc(want_to_read::Column::Position)
                .order_by_asc(want_to_read::Column::AddedAt)
                .order_by_asc(want_to_read::Column::Id),
        };
        Ok(query.all(db).await?)
    }

    /// Set explicit positions for the given entries in the order provided.
    /// Entries not in the user's queue are skipped (the user_id scope prevents
    /// touching another user's rows).
    pub async fn reorder(
        db: &DatabaseConnection,
        user_id: Uuid,
        ordered_entry_ids: &[Uuid],
    ) -> Result<()> {
        for (idx, entry_id) in ordered_entry_ids.iter().enumerate() {
            if let Some(entry) = WantToRead::find()
                .filter(want_to_read::Column::UserId.eq(user_id))
                .filter(want_to_read::Column::Id.eq(*entry_id))
                .one(db)
                .await?
            {
                let mut active = entry.into_active_model();
                active.position = Set(idx as i32);
                active.update(db).await?;
            }
        }
        Ok(())
    }

    /// Next position value for a new entry (max existing + 1, or 0 when empty).
    async fn next_position(db: &DatabaseConnection, user_id: Uuid) -> Result<i32> {
        let positions: Vec<i32> = WantToRead::find()
            .filter(want_to_read::Column::UserId.eq(user_id))
            .all(db)
            .await?
            .into_iter()
            .map(|e| e.position)
            .collect();
        Ok(positions.into_iter().max().map(|m| m + 1).unwrap_or(0))
    }

    /// Whether a series is in the user's queue.
    pub async fn is_series_in_queue(
        db: &DatabaseConnection,
        user_id: Uuid,
        series_id: Uuid,
    ) -> Result<bool> {
        Ok(WantToRead::find()
            .filter(want_to_read::Column::UserId.eq(user_id))
            .filter(want_to_read::Column::SeriesId.eq(series_id))
            .one(db)
            .await?
            .is_some())
    }

    /// Whether a book is in the user's queue.
    pub async fn is_book_in_queue(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Result<bool> {
        Ok(WantToRead::find()
            .filter(want_to_read::Column::UserId.eq(user_id))
            .filter(want_to_read::Column::BookId.eq(book_id))
            .one(db)
            .await?
            .is_some())
    }

    /// Of the given series IDs, return the subset in the user's queue. Batch
    /// helper for enriching series DTOs with a `wantToRead` flag.
    pub async fn series_ids_in_queue(
        db: &DatabaseConnection,
        user_id: Uuid,
        series_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>> {
        if series_ids.is_empty() {
            return Ok(HashSet::new());
        }
        Ok(WantToRead::find()
            .filter(want_to_read::Column::UserId.eq(user_id))
            .filter(want_to_read::Column::SeriesId.is_in(series_ids.to_vec()))
            .all(db)
            .await?
            .into_iter()
            .filter_map(|r| r.series_id)
            .collect())
    }

    /// Of the given book IDs, return the subset in the user's queue. Batch
    /// helper for enriching book DTOs with a `wantToRead` flag.
    pub async fn book_ids_in_queue(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>> {
        if book_ids.is_empty() {
            return Ok(HashSet::new());
        }
        Ok(WantToRead::find()
            .filter(want_to_read::Column::UserId.eq(user_id))
            .filter(want_to_read::Column::BookId.is_in(book_ids.to_vec()))
            .all(db)
            .await?
            .into_iter()
            .filter_map(|r| r.book_id)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScanningStrategy;
    use crate::entities::{books, users};
    use crate::repositories::{
        BookRepository, LibraryRepository, SeriesRepository, UserRepository,
    };
    use crate::test_helpers::create_test_db;

    async fn make_user(db: &DatabaseConnection, name: &str) -> users::Model {
        let now = Utc::now();
        let model = users::Model {
            id: Uuid::new_v4(),
            username: name.to_string(),
            email: format!("{name}@test.test"),
            password_hash: "hash".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: false,
            permissions: serde_json::json!([]),
            created_at: now,
            updated_at: now,
            last_login_at: None,
        };
        UserRepository::create(db, &model).await.unwrap()
    }

    async fn make_series_and_book(db: &DatabaseConnection) -> (Uuid, Uuid) {
        let library = LibraryRepository::create(db, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = SeriesRepository::create(db, library.id, "Series", None)
            .await
            .unwrap();
        let book = books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
            path: "/test/book.cbz".to_string(),
            file_name: "book.cbz".to_string(),
            file_size: 1024,
            file_hash: format!("hash_{}", Uuid::new_v4()),
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
            epub_spine_items: None,
        };
        let book = BookRepository::create(db, &book, None).await.unwrap();
        (series.id, book.id)
    }

    #[tokio::test]
    async fn test_add_remove_and_idempotency() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = make_user(conn, "alice").await;
        let (series_id, book_id) = make_series_and_book(conn).await;

        WantToReadRepository::add_series(conn, user.id, series_id)
            .await
            .unwrap();
        // Idempotent.
        WantToReadRepository::add_series(conn, user.id, series_id)
            .await
            .unwrap();
        WantToReadRepository::add_book(conn, user.id, book_id)
            .await
            .unwrap();

        let queue = WantToReadRepository::list(conn, user.id, WantToReadSort::Newest)
            .await
            .unwrap();
        assert_eq!(queue.len(), 2);

        assert!(
            WantToReadRepository::is_series_in_queue(conn, user.id, series_id)
                .await
                .unwrap()
        );
        assert!(
            WantToReadRepository::is_book_in_queue(conn, user.id, book_id)
                .await
                .unwrap()
        );

        assert!(
            WantToReadRepository::remove_series(conn, user.id, series_id)
                .await
                .unwrap()
        );
        assert!(
            !WantToReadRepository::is_series_in_queue(conn, user.id, series_id)
                .await
                .unwrap()
        );
        assert_eq!(
            WantToReadRepository::list(conn, user.id, WantToReadSort::Newest)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn test_add_series_bulk_inserts_new_and_skips_duplicates() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = make_user(conn, "alice").await;
        let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let s1 = SeriesRepository::create(conn, library.id, "S1", None)
            .await
            .unwrap()
            .id;
        let s2 = SeriesRepository::create(conn, library.id, "S2", None)
            .await
            .unwrap()
            .id;
        let s3 = SeriesRepository::create(conn, library.id, "S3", None)
            .await
            .unwrap()
            .id;

        // s1 is already queued; the batch carries a duplicate of s2.
        WantToReadRepository::add_series(conn, user.id, s1)
            .await
            .unwrap();

        let added = WantToReadRepository::add_series_bulk(conn, user.id, &[s1, s2, s2, s3])
            .await
            .unwrap();
        // s1 already present, s2 deduped to one insert, s3 new -> 2 newly added.
        assert_eq!(added, 2);
        assert_eq!(
            WantToReadRepository::list(conn, user.id, WantToReadSort::Newest)
                .await
                .unwrap()
                .len(),
            3
        );

        // Re-running the same batch adds nothing.
        let again = WantToReadRepository::add_series_bulk(conn, user.id, &[s1, s2, s3])
            .await
            .unwrap();
        assert_eq!(again, 0);

        // Empty input is a no-op.
        assert_eq!(
            WantToReadRepository::add_series_bulk(conn, user.id, &[])
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn test_custom_sort_and_reorder() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = make_user(conn, "alice").await;
        let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let mut series_ids = Vec::new();
        for name in ["S1", "S2", "S3"] {
            series_ids.push(
                SeriesRepository::create(conn, library.id, name, None)
                    .await
                    .unwrap()
                    .id,
            );
        }
        for id in &series_ids {
            WantToReadRepository::add_series(conn, user.id, *id)
                .await
                .unwrap();
        }

        // Before any reorder, custom falls back to insertion order (positions
        // are already max+1 per add, and added_at tie-breaks legacy zeros).
        let queue = WantToReadRepository::list(conn, user.id, WantToReadSort::Custom)
            .await
            .unwrap();
        let entry_ids: Vec<Uuid> = queue.iter().map(|e| e.id).collect();
        assert_eq!(
            queue
                .iter()
                .map(|e| e.series_id.unwrap())
                .collect::<Vec<_>>(),
            series_ids
        );

        // Reverse the queue and re-read in custom order.
        let reversed: Vec<Uuid> = entry_ids.iter().rev().copied().collect();
        WantToReadRepository::reorder(conn, user.id, &reversed)
            .await
            .unwrap();
        let queue = WantToReadRepository::list(conn, user.id, WantToReadSort::Custom)
            .await
            .unwrap();
        assert_eq!(queue[0].series_id.unwrap(), series_ids[2]);
        assert_eq!(queue[2].series_id.unwrap(), series_ids[0]);

        // A newly added entry appends at the end of the custom order.
        let s4 = SeriesRepository::create(conn, library.id, "S4", None)
            .await
            .unwrap()
            .id;
        WantToReadRepository::add_series(conn, user.id, s4)
            .await
            .unwrap();
        let queue = WantToReadRepository::list(conn, user.id, WantToReadSort::Custom)
            .await
            .unwrap();
        assert_eq!(queue.len(), 4);
        assert_eq!(queue[3].series_id.unwrap(), s4);

        // Another user's reorder cannot touch this queue.
        let mallory = make_user(conn, "mallory").await;
        WantToReadRepository::reorder(conn, mallory.id, &entry_ids)
            .await
            .unwrap();
        let queue_after = WantToReadRepository::list(conn, user.id, WantToReadSort::Custom)
            .await
            .unwrap();
        assert_eq!(
            queue_after.iter().map(|e| e.id).collect::<Vec<_>>(),
            queue.iter().map(|e| e.id).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn test_add_books_bulk_inserts_new_and_skips_duplicates() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = make_user(conn, "alice").await;
        let (_series_id, book_id) = make_series_and_book(conn).await;

        let added = WantToReadRepository::add_books_bulk(conn, user.id, &[book_id, book_id])
            .await
            .unwrap();
        assert_eq!(added, 1);
        assert!(
            WantToReadRepository::is_book_in_queue(conn, user.id, book_id)
                .await
                .unwrap()
        );

        let again = WantToReadRepository::add_books_bulk(conn, user.id, &[book_id])
            .await
            .unwrap();
        assert_eq!(again, 0);
    }

    #[tokio::test]
    async fn test_per_user_isolation_and_batch_lookup() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let alice = make_user(conn, "alice").await;
        let bob = make_user(conn, "bob").await;
        let (series_id, _book_id) = make_series_and_book(conn).await;

        WantToReadRepository::add_series(conn, alice.id, series_id)
            .await
            .unwrap();

        // Bob's queue is unaffected.
        assert!(
            !WantToReadRepository::is_series_in_queue(conn, bob.id, series_id)
                .await
                .unwrap()
        );

        let in_queue = WantToReadRepository::series_ids_in_queue(conn, alice.id, &[series_id])
            .await
            .unwrap();
        assert!(in_queue.contains(&series_id));

        let bob_in_queue = WantToReadRepository::series_ids_in_queue(conn, bob.id, &[series_id])
            .await
            .unwrap();
        assert!(bob_in_queue.is_empty());
    }
}
