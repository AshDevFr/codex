//! Repository for read lists and the read_list_books junction.
//!
//! Read lists are shared, ordered groupings of books across series. Membership
//! order is held by the `position` column on the junction; whether it is honored
//! (vs. a release-date fallback) is decided by the caller based on the read
//! list's `ordered` flag.

#![allow(dead_code)]

use std::collections::HashMap;

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use uuid::Uuid;

use crate::entities::{
    books, books::Entity as Books, read_list_books, read_list_books::Entity as ReadListBooks,
    read_lists, read_lists::Entity as ReadLists,
};
use crate::repositories::visibility::{SeriesVisibility, apply_book_visibility};

/// Repository for read list operations.
pub struct ReadListRepository;

impl ReadListRepository {
    /// Get a read list by ID.
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<read_lists::Model>> {
        Ok(ReadLists::find_by_id(id).one(db).await?)
    }

    /// Get a read list by (case-insensitive) name.
    pub async fn get_by_name(
        db: &DatabaseConnection,
        name: &str,
    ) -> Result<Option<read_lists::Model>> {
        let normalized = name.trim().to_lowercase();
        Ok(ReadLists::find()
            .filter(read_lists::Column::NormalizedName.eq(normalized))
            .one(db)
            .await?)
    }

    /// List all read lists sorted by name.
    pub async fn list_all(db: &DatabaseConnection) -> Result<Vec<read_lists::Model>> {
        Ok(ReadLists::find()
            .order_by_asc(read_lists::Column::Name)
            .all(db)
            .await?)
    }

    /// Total number of read lists.
    pub async fn count(db: &DatabaseConnection) -> Result<u64> {
        Ok(ReadLists::find().count(db).await?)
    }

    /// Get the set of book IDs that belong to at least one read list.
    ///
    /// Used by the filter service to evaluate the "in read list" membership
    /// filter. Returns distinct book IDs across all read lists.
    pub async fn all_member_book_ids(
        db: &DatabaseConnection,
    ) -> Result<std::collections::HashSet<Uuid>> {
        let ids: Vec<Uuid> = ReadListBooks::find()
            .select_only()
            .column(read_list_books::Column::BookId)
            .distinct()
            .into_tuple()
            .all(db)
            .await?;
        Ok(ids.into_iter().collect())
    }

    /// Create a new read list. Fails if the (normalized) name already exists.
    pub async fn create(
        db: &DatabaseConnection,
        name: &str,
        summary: Option<&str>,
        ordered: bool,
    ) -> Result<read_lists::Model> {
        let now = Utc::now();
        let model = read_lists::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(name.trim().to_string()),
            normalized_name: Set(name.trim().to_lowercase()),
            summary: Set(summary.map(|s| s.to_string())),
            ordered: Set(ordered),
            created_at: Set(now),
            updated_at: Set(now),
        };
        Ok(model.insert(db).await?)
    }

    /// Update a read list's name, summary, and/or ordered flag. Returns `None`
    /// if the read list does not exist. `summary = Some(None)` clears it.
    pub async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        name: Option<&str>,
        summary: Option<Option<&str>>,
        ordered: Option<bool>,
    ) -> Result<Option<read_lists::Model>> {
        let Some(existing) = ReadLists::find_by_id(id).one(db).await? else {
            return Ok(None);
        };
        let mut active = existing.into_active_model();
        if let Some(name) = name {
            active.name = Set(name.trim().to_string());
            active.normalized_name = Set(name.trim().to_lowercase());
        }
        if let Some(summary) = summary {
            active.summary = Set(summary.map(|s| s.to_string()));
        }
        if let Some(ordered) = ordered {
            active.ordered = Set(ordered);
        }
        active.updated_at = Set(Utc::now());
        Ok(Some(active.update(db).await?))
    }

    /// Delete a read list (cascades its membership rows). Returns whether a row
    /// was removed.
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = ReadLists::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Add a book to a read list at the end of the order. Idempotent.
    pub async fn add_book(
        db: &DatabaseConnection,
        read_list_id: Uuid,
        book_id: Uuid,
    ) -> Result<read_list_books::Model> {
        if let Some(existing) = ReadListBooks::find()
            .filter(read_list_books::Column::ReadListId.eq(read_list_id))
            .filter(read_list_books::Column::BookId.eq(book_id))
            .one(db)
            .await?
        {
            return Ok(existing);
        }

        let position = Self::next_position(db, read_list_id).await?;
        let link = read_list_books::ActiveModel {
            id: Set(Uuid::new_v4()),
            read_list_id: Set(read_list_id),
            book_id: Set(book_id),
            position: Set(position),
            created_at: Set(Utc::now()),
        };
        Ok(link.insert(db).await?)
    }

    /// Remove a book from a read list. Returns whether a row was removed.
    pub async fn remove_book(
        db: &DatabaseConnection,
        read_list_id: Uuid,
        book_id: Uuid,
    ) -> Result<bool> {
        let result = ReadListBooks::delete_many()
            .filter(read_list_books::Column::ReadListId.eq(read_list_id))
            .filter(read_list_books::Column::BookId.eq(book_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// Set explicit positions for the given books in the order provided. Books
    /// not currently members are skipped.
    pub async fn reorder(
        db: &DatabaseConnection,
        read_list_id: Uuid,
        ordered_book_ids: &[Uuid],
    ) -> Result<()> {
        for (idx, book_id) in ordered_book_ids.iter().enumerate() {
            if let Some(link) = ReadListBooks::find()
                .filter(read_list_books::Column::ReadListId.eq(read_list_id))
                .filter(read_list_books::Column::BookId.eq(*book_id))
                .one(db)
                .await?
            {
                let mut active = link.into_active_model();
                active.position = Set(idx as i32);
                active.update(db).await?;
            }
        }
        Ok(())
    }

    /// Get the member books of a read list in stored order (by position, then
    /// insertion time), filtered by the caller's (series-based) visibility.
    pub async fn get_books(
        db: &DatabaseConnection,
        read_list_id: Uuid,
        vis: Option<&SeriesVisibility>,
    ) -> Result<Vec<books::Model>> {
        if matches!(vis, Some(v) if v.is_empty_whitelist()) {
            return Ok(vec![]);
        }

        let ordered_ids: Vec<Uuid> = ReadListBooks::find()
            .filter(read_list_books::Column::ReadListId.eq(read_list_id))
            .order_by_asc(read_list_books::Column::Position)
            .order_by_asc(read_list_books::Column::CreatedAt)
            .all(db)
            .await?
            .into_iter()
            .map(|l| l.book_id)
            .collect();
        if ordered_ids.is_empty() {
            return Ok(vec![]);
        }

        // Visibility is series-based; apply it to the books query.
        let query = apply_book_visibility(
            Books::find().filter(books::Column::Id.is_in(ordered_ids.clone())),
            vis,
        );
        let by_id: HashMap<Uuid, books::Model> = query
            .all(db)
            .await?
            .into_iter()
            .map(|b| (b.id, b))
            .collect();

        Ok(ordered_ids
            .iter()
            .filter_map(|id| by_id.get(id).cloned())
            .collect())
    }

    /// Count the visible member books of a read list.
    pub async fn count_books(
        db: &DatabaseConnection,
        read_list_id: Uuid,
        vis: Option<&SeriesVisibility>,
    ) -> Result<u64> {
        if matches!(vis, Some(v) if v.is_empty_whitelist()) {
            return Ok(0);
        }
        let ids: Vec<Uuid> = ReadListBooks::find()
            .filter(read_list_books::Column::ReadListId.eq(read_list_id))
            .all(db)
            .await?
            .into_iter()
            .map(|l| l.book_id)
            .collect();
        if ids.is_empty() {
            return Ok(0);
        }
        let query = apply_book_visibility(Books::find().filter(books::Column::Id.is_in(ids)), vis);
        Ok(query.count(db).await?)
    }

    /// Get the read lists that contain a given book, sorted by name.
    pub async fn get_read_lists_for_book(
        db: &DatabaseConnection,
        book_id: Uuid,
    ) -> Result<Vec<read_lists::Model>> {
        let read_list_ids: Vec<Uuid> = ReadListBooks::find()
            .filter(read_list_books::Column::BookId.eq(book_id))
            .all(db)
            .await?
            .into_iter()
            .map(|l| l.read_list_id)
            .collect();
        if read_list_ids.is_empty() {
            return Ok(vec![]);
        }
        Ok(ReadLists::find()
            .filter(read_lists::Column::Id.is_in(read_list_ids))
            .order_by_asc(read_lists::Column::Name)
            .all(db)
            .await?)
    }

    /// Next position value for a new member (max existing + 1, or 0 when empty).
    async fn next_position(db: &DatabaseConnection, read_list_id: Uuid) -> Result<i32> {
        let positions: Vec<i32> = ReadListBooks::find()
            .filter(read_list_books::Column::ReadListId.eq(read_list_id))
            .all(db)
            .await?
            .into_iter()
            .map(|l| l.position)
            .collect();
        Ok(positions.into_iter().max().map(|m| m + 1).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScanningStrategy;
    use crate::entities::series;
    use crate::repositories::{BookRepository, LibraryRepository, SeriesRepository};
    use crate::test_helpers::create_test_db;

    async fn make_book(db: &DatabaseConnection, series_id: Uuid, library_id: Uuid) -> books::Model {
        let book = books::Model {
            id: Uuid::new_v4(),
            series_id,
            library_id,
            path: format!("/test/{}.cbz", Uuid::new_v4()),
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
        BookRepository::create(db, &book, None).await.unwrap()
    }

    async fn setup(db: &DatabaseConnection) -> (series::Model, Vec<books::Model>) {
        let library = LibraryRepository::create(db, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = SeriesRepository::create(db, library.id, "Series", None)
            .await
            .unwrap();
        let mut books = Vec::new();
        for _ in 0..3 {
            books.push(make_book(db, series.id, library.id).await);
        }
        (series, books)
    }

    #[tokio::test]
    async fn test_create_with_summary_and_update() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let rl = ReadListRepository::create(conn, "Civil War", Some("Crossover"), true)
            .await
            .unwrap();
        assert_eq!(rl.summary.as_deref(), Some("Crossover"));
        assert!(rl.ordered);

        let updated = ReadListRepository::update(conn, rl.id, None, Some(None), Some(false))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.summary, None);
        assert!(!updated.ordered);

        assert!(ReadListRepository::delete(conn, rl.id).await.unwrap());
    }

    #[tokio::test]
    async fn test_all_member_book_ids() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (_series, books) = setup(conn).await;

        // No read lists yet => empty set.
        let members = ReadListRepository::all_member_book_ids(conn).await.unwrap();
        assert!(members.is_empty());

        // Two read lists, with one book shared between them.
        let rl_a = ReadListRepository::create(conn, "A", None, false)
            .await
            .unwrap();
        let rl_b = ReadListRepository::create(conn, "B", None, false)
            .await
            .unwrap();
        ReadListRepository::add_book(conn, rl_a.id, books[0].id)
            .await
            .unwrap();
        ReadListRepository::add_book(conn, rl_a.id, books[1].id)
            .await
            .unwrap();
        // books[1] also belongs to B => must be de-duplicated.
        ReadListRepository::add_book(conn, rl_b.id, books[1].id)
            .await
            .unwrap();

        let members = ReadListRepository::all_member_book_ids(conn).await.unwrap();
        assert_eq!(members.len(), 2);
        assert!(members.contains(&books[0].id));
        assert!(members.contains(&books[1].id));
        // books[2] is in no read list.
        assert!(!members.contains(&books[2].id));
    }

    #[tokio::test]
    async fn test_membership_order_and_reorder() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (_series, books) = setup(conn).await;

        let rl = ReadListRepository::create(conn, "List", None, true)
            .await
            .unwrap();
        for b in &books {
            ReadListRepository::add_book(conn, rl.id, b.id)
                .await
                .unwrap();
        }
        // Idempotent re-add.
        ReadListRepository::add_book(conn, rl.id, books[0].id)
            .await
            .unwrap();

        let members = ReadListRepository::get_books(conn, rl.id, None)
            .await
            .unwrap();
        assert_eq!(members.len(), 3);
        assert_eq!(members[0].id, books[0].id);

        let reversed: Vec<Uuid> = books.iter().rev().map(|b| b.id).collect();
        ReadListRepository::reorder(conn, rl.id, &reversed)
            .await
            .unwrap();
        let members = ReadListRepository::get_books(conn, rl.id, None)
            .await
            .unwrap();
        assert_eq!(members[0].id, books[2].id);

        // Containers-for-book lookup.
        let lists = ReadListRepository::get_read_lists_for_book(conn, books[0].id)
            .await
            .unwrap();
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].id, rl.id);
    }

    #[tokio::test]
    async fn test_book_visibility_filtering() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series, books) = setup(conn).await;

        let rl = ReadListRepository::create(conn, "List", None, true)
            .await
            .unwrap();
        for b in &books {
            ReadListRepository::add_book(conn, rl.id, b.id)
                .await
                .unwrap();
        }

        // Hiding the whole series hides all its books from this viewer.
        let vis = SeriesVisibility {
            excluded_series_ids: vec![series.id],
            allowed_series_ids: None,
        };
        assert!(
            ReadListRepository::get_books(conn, rl.id, Some(&vis))
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            ReadListRepository::count_books(conn, rl.id, Some(&vis))
                .await
                .unwrap(),
            0
        );
        // Without the filter, all three are visible.
        assert_eq!(
            ReadListRepository::count_books(conn, rl.id, None)
                .await
                .unwrap(),
            3
        );
    }
}
