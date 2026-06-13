//! Repository for the per-user want-to-read queue.
//!
//! Each row flags exactly one series OR one book a user intends to read. The
//! queue is personal: every method scopes to a `user_id`.

#![allow(dead_code)]

use std::collections::HashSet;

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::entities::{want_to_read, want_to_read::Entity as WantToRead};

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
        };
        Ok(model.insert(db).await?)
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

    /// List a user's queue ordered by when each entry was added.
    pub async fn list(
        db: &DatabaseConnection,
        user_id: Uuid,
        ascending: bool,
    ) -> Result<Vec<want_to_read::Model>> {
        let query = WantToRead::find().filter(want_to_read::Column::UserId.eq(user_id));
        let query = if ascending {
            query.order_by_asc(want_to_read::Column::AddedAt)
        } else {
            query.order_by_desc(want_to_read::Column::AddedAt)
        };
        Ok(query.all(db).await?)
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

        let queue = WantToReadRepository::list(conn, user.id, false)
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
            WantToReadRepository::list(conn, user.id, false)
                .await
                .unwrap()
                .len(),
            1
        );
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
