//! Repository for book_covers table operations
//!
//! Provides CRUD operations for managing book cover images.
//! Supports multiple covers per book with one selected as primary.
//!
//! Mirrors the series_covers repository pattern.

#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::db::entities::{book_covers, book_covers::Entity as BookCovers};

/// Repository for book cover operations
pub struct BookCoversRepository;

impl BookCoversRepository {
    /// Get all covers for a book
    pub async fn list_by_book(
        db: &DatabaseConnection,
        book_id: Uuid,
    ) -> Result<Vec<book_covers::Model>> {
        let results = BookCovers::find()
            .filter(book_covers::Column::BookId.eq(book_id))
            .order_by_asc(book_covers::Column::CreatedAt)
            .all(db)
            .await?;
        Ok(results)
    }

    /// Get the selected (primary) cover for a book
    pub async fn get_selected(
        db: &DatabaseConnection,
        book_id: Uuid,
    ) -> Result<Option<book_covers::Model>> {
        let result = BookCovers::find()
            .filter(book_covers::Column::BookId.eq(book_id))
            .filter(book_covers::Column::IsSelected.eq(true))
            .one(db)
            .await?;
        Ok(result)
    }

    /// Get a cover by its ID
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<book_covers::Model>> {
        let result = BookCovers::find_by_id(id).one(db).await?;
        Ok(result)
    }

    /// Get a cover by book and source
    pub async fn get_by_source(
        db: &DatabaseConnection,
        book_id: Uuid,
        source: &str,
    ) -> Result<Option<book_covers::Model>> {
        let result = BookCovers::find()
            .filter(book_covers::Column::BookId.eq(book_id))
            .filter(book_covers::Column::Source.eq(source))
            .one(db)
            .await?;
        Ok(result)
    }

    /// Create a new cover for a book
    pub async fn create(
        db: &DatabaseConnection,
        book_id: Uuid,
        source: &str,
        path: &str,
        is_selected: bool,
        width: Option<i32>,
        height: Option<i32>,
    ) -> Result<book_covers::Model> {
        let now = Utc::now();

        // If this is being selected, deselect any existing selected covers
        if is_selected {
            Self::deselect_all(db, book_id).await?;
        }

        let active_model = book_covers::ActiveModel {
            id: Set(Uuid::new_v4()),
            book_id: Set(book_id),
            source: Set(source.to_string()),
            path: Set(path.to_string()),
            is_selected: Set(is_selected),
            width: Set(width),
            height: Set(height),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let model = active_model.insert(db).await?;
        Ok(model)
    }

    /// Create a cover from extracted book content (EPUB cover, CBZ first page, etc.)
    pub async fn create_extracted(
        db: &DatabaseConnection,
        book_id: Uuid,
        path: &str,
        is_selected: bool,
        width: Option<i32>,
        height: Option<i32>,
    ) -> Result<book_covers::Model> {
        Self::create(db, book_id, "extracted", path, is_selected, width, height).await
    }

    /// Create a cover from a plugin source
    pub async fn create_for_plugin(
        db: &DatabaseConnection,
        book_id: Uuid,
        plugin_name: &str,
        path: &str,
        is_selected: bool,
        width: Option<i32>,
        height: Option<i32>,
    ) -> Result<book_covers::Model> {
        let source = book_covers::Model::plugin_source(plugin_name);
        Self::create(db, book_id, &source, path, is_selected, width, height).await
    }

    /// Set a specific cover as selected (and deselect others)
    pub async fn select_cover(
        db: &DatabaseConnection,
        book_id: Uuid,
        cover_id: Uuid,
    ) -> Result<book_covers::Model> {
        // First deselect all covers for this book
        Self::deselect_all(db, book_id).await?;

        // Now select the specified cover
        let cover = Self::get_by_id(db, cover_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Cover not found: {}", cover_id))?;

        if cover.book_id != book_id {
            return Err(anyhow::anyhow!(
                "Cover {} does not belong to book {}",
                cover_id,
                book_id
            ));
        }

        let mut active_model: book_covers::ActiveModel = cover.into();
        active_model.is_selected = Set(true);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Select a cover by source (e.g., "custom", "extracted", "plugin:openlibrary")
    pub async fn select_by_source(
        db: &DatabaseConnection,
        book_id: Uuid,
        source: &str,
    ) -> Result<Option<book_covers::Model>> {
        // First check if the source exists
        let cover = match Self::get_by_source(db, book_id, source).await? {
            Some(c) => c,
            None => return Ok(None),
        };

        // Deselect all and select this one
        Self::deselect_all(db, book_id).await?;

        let mut active_model: book_covers::ActiveModel = cover.into();
        active_model.is_selected = Set(true);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(Some(model))
    }

    /// Deselect all covers for a book (resets to default thumbnail)
    pub async fn deselect_all(db: &DatabaseConnection, book_id: Uuid) -> Result<()> {
        use sea_orm::sea_query::Expr;

        BookCovers::update_many()
            .filter(book_covers::Column::BookId.eq(book_id))
            .filter(book_covers::Column::IsSelected.eq(true))
            .col_expr(book_covers::Column::IsSelected, Expr::value(false))
            .col_expr(book_covers::Column::UpdatedAt, Expr::value(Utc::now()))
            .exec(db)
            .await?;

        Ok(())
    }

    /// Update cover path
    pub async fn update_path(
        db: &DatabaseConnection,
        id: Uuid,
        path: &str,
    ) -> Result<book_covers::Model> {
        let cover = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Cover not found: {}", id))?;

        let mut active_model: book_covers::ActiveModel = cover.into();
        active_model.path = Set(path.to_string());
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Update cover dimensions
    pub async fn update_dimensions(
        db: &DatabaseConnection,
        id: Uuid,
        width: Option<i32>,
        height: Option<i32>,
    ) -> Result<book_covers::Model> {
        let cover = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Cover not found: {}", id))?;

        let mut active_model: book_covers::ActiveModel = cover.into();
        active_model.width = Set(width);
        active_model.height = Set(height);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Delete a cover by ID
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        BookCovers::delete_by_id(id).exec(db).await?;
        Ok(())
    }

    /// Delete all covers for a book
    pub async fn delete_by_book(db: &DatabaseConnection, book_id: Uuid) -> Result<()> {
        BookCovers::delete_many()
            .filter(book_covers::Column::BookId.eq(book_id))
            .exec(db)
            .await?;
        Ok(())
    }

    /// Delete cover by source (e.g., delete the "custom" cover)
    pub async fn delete_by_source(
        db: &DatabaseConnection,
        book_id: Uuid,
        source: &str,
    ) -> Result<()> {
        BookCovers::delete_many()
            .filter(book_covers::Column::BookId.eq(book_id))
            .filter(book_covers::Column::Source.eq(source))
            .exec(db)
            .await?;
        Ok(())
    }

    /// Check if a book has a custom cover
    pub async fn has_custom_cover(db: &DatabaseConnection, book_id: Uuid) -> Result<bool> {
        let result = Self::get_by_source(db, book_id, "custom").await?;
        Ok(result.is_some())
    }

    /// Get the selected cover source for a book (e.g., "custom", "extracted", or None)
    pub async fn get_selected_source(
        db: &DatabaseConnection,
        book_id: Uuid,
    ) -> Result<Option<String>> {
        let selected = Self::get_selected(db, book_id).await?;
        Ok(selected.map(|c| c.source))
    }

    /// Get selected covers for multiple books by their IDs
    ///
    /// Returns a HashMap keyed by book_id for efficient lookups
    pub async fn get_selected_for_book_ids(
        db: &DatabaseConnection,
        book_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, book_covers::Model>> {
        if book_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let results = BookCovers::find()
            .filter(book_covers::Column::BookId.is_in(book_ids.to_vec()))
            .filter(book_covers::Column::IsSelected.eq(true))
            .all(db)
            .await?;

        Ok(results.into_iter().map(|c| (c.book_id, c)).collect())
    }

    /// Check if multiple books have custom covers
    ///
    /// Returns a HashMap keyed by book_id with boolean values
    pub async fn has_custom_cover_for_book_ids(
        db: &DatabaseConnection,
        book_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, bool>> {
        if book_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let results = BookCovers::find()
            .filter(book_covers::Column::BookId.is_in(book_ids.to_vec()))
            .filter(book_covers::Column::Source.eq("custom"))
            .all(db)
            .await?;

        let custom_cover_set: std::collections::HashSet<Uuid> =
            results.into_iter().map(|c| c.book_id).collect();

        Ok(book_ids
            .iter()
            .map(|id| (*id, custom_cover_set.contains(id)))
            .collect())
    }

    /// Upsert a cover by source
    ///
    /// Creates if not exists, updates if exists
    pub async fn upsert_by_source(
        db: &DatabaseConnection,
        book_id: Uuid,
        source: &str,
        path: &str,
        is_selected: bool,
        width: Option<i32>,
        height: Option<i32>,
    ) -> Result<book_covers::Model> {
        let existing = Self::get_by_source(db, book_id, source).await?;

        match existing {
            Some(existing) => {
                // Update existing
                let mut active_model: book_covers::ActiveModel = existing.into();
                active_model.path = Set(path.to_string());
                active_model.width = Set(width);
                active_model.height = Set(height);
                active_model.updated_at = Set(Utc::now());

                if is_selected {
                    Self::deselect_all(db, book_id).await?;
                    active_model.is_selected = Set(true);
                }

                let model = active_model.update(db).await?;
                Ok(model)
            }
            None => {
                // Create new
                Self::create(db, book_id, source, path, is_selected, width, height).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::entities::books;
    use crate::db::repositories::{BookRepository, LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;
    use chrono::Utc;

    async fn setup_test_book(db: &DatabaseConnection) -> (Uuid, Uuid) {
        let library =
            LibraryRepository::create(db, "Test Library", "/test/path", ScanningStrategy::Default)
                .await
                .unwrap();

        let series = SeriesRepository::create(db, library.id, "Test Series", None)
            .await
            .unwrap();

        let book_model = books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
            file_path: "/test/path/test.cbz".to_string(),
            file_name: "test.cbz".to_string(),
            file_size: 1024,
            file_hash: "test_hash".to_string(),
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

        let book = BookRepository::create(db, &book_model, None).await.unwrap();

        (series.id, book.id)
    }

    #[tokio::test]
    async fn test_create_and_list_covers() {
        let (db, _temp_dir) = create_test_db().await;
        let (_, book_id) = setup_test_book(db.sea_orm_connection()).await;

        // Create first cover (selected)
        let cover1 = BookCoversRepository::create(
            db.sea_orm_connection(),
            book_id,
            "extracted",
            "/covers/extracted.jpg",
            true,
            Some(800),
            Some(1200),
        )
        .await
        .unwrap();

        assert!(cover1.is_selected);
        assert_eq!(cover1.source, "extracted");

        // Create second cover (not selected)
        let cover2 = BookCoversRepository::create(
            db.sea_orm_connection(),
            book_id,
            "custom",
            "/covers/custom.jpg",
            false,
            None,
            None,
        )
        .await
        .unwrap();

        assert!(!cover2.is_selected);

        // List all covers
        let covers = BookCoversRepository::list_by_book(db.sea_orm_connection(), book_id)
            .await
            .unwrap();

        assert_eq!(covers.len(), 2);
    }

    #[tokio::test]
    async fn test_select_cover() {
        let (db, _temp_dir) = create_test_db().await;
        let (_, book_id) = setup_test_book(db.sea_orm_connection()).await;

        // Create two covers
        let cover1 = BookCoversRepository::create(
            db.sea_orm_connection(),
            book_id,
            "extracted",
            "/covers/extracted.jpg",
            true,
            None,
            None,
        )
        .await
        .unwrap();

        let cover2 = BookCoversRepository::create(
            db.sea_orm_connection(),
            book_id,
            "custom",
            "/covers/custom.jpg",
            false,
            None,
            None,
        )
        .await
        .unwrap();

        // Select the second cover
        let selected =
            BookCoversRepository::select_cover(db.sea_orm_connection(), book_id, cover2.id)
                .await
                .unwrap();

        assert!(selected.is_selected);
        assert_eq!(selected.source, "custom");

        // Verify first cover is now deselected
        let cover1_updated = BookCoversRepository::get_by_id(db.sea_orm_connection(), cover1.id)
            .await
            .unwrap()
            .unwrap();

        assert!(!cover1_updated.is_selected);
    }

    #[tokio::test]
    async fn test_get_selected() {
        let (db, _temp_dir) = create_test_db().await;
        let (_, book_id) = setup_test_book(db.sea_orm_connection()).await;

        // No covers yet
        let selected = BookCoversRepository::get_selected(db.sea_orm_connection(), book_id)
            .await
            .unwrap();
        assert!(selected.is_none());

        // Create a selected cover
        BookCoversRepository::create(
            db.sea_orm_connection(),
            book_id,
            "custom",
            "/covers/custom.jpg",
            true,
            None,
            None,
        )
        .await
        .unwrap();

        let selected = BookCoversRepository::get_selected(db.sea_orm_connection(), book_id)
            .await
            .unwrap();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().source, "custom");
    }

    #[tokio::test]
    async fn test_has_custom_cover() {
        let (db, _temp_dir) = create_test_db().await;
        let (_, book_id) = setup_test_book(db.sea_orm_connection()).await;

        // No custom cover
        let has_custom = BookCoversRepository::has_custom_cover(db.sea_orm_connection(), book_id)
            .await
            .unwrap();
        assert!(!has_custom);

        // Add a custom cover
        BookCoversRepository::create(
            db.sea_orm_connection(),
            book_id,
            "custom",
            "/covers/custom.jpg",
            false,
            None,
            None,
        )
        .await
        .unwrap();

        let has_custom = BookCoversRepository::has_custom_cover(db.sea_orm_connection(), book_id)
            .await
            .unwrap();
        assert!(has_custom);
    }

    #[tokio::test]
    async fn test_delete_by_source() {
        let (db, _temp_dir) = create_test_db().await;
        let (_, book_id) = setup_test_book(db.sea_orm_connection()).await;

        // Create covers
        BookCoversRepository::create(
            db.sea_orm_connection(),
            book_id,
            "extracted",
            "/covers/extracted.jpg",
            true,
            None,
            None,
        )
        .await
        .unwrap();

        BookCoversRepository::create(
            db.sea_orm_connection(),
            book_id,
            "custom",
            "/covers/custom.jpg",
            false,
            None,
            None,
        )
        .await
        .unwrap();

        // Delete custom cover
        BookCoversRepository::delete_by_source(db.sea_orm_connection(), book_id, "custom")
            .await
            .unwrap();

        let covers = BookCoversRepository::list_by_book(db.sea_orm_connection(), book_id)
            .await
            .unwrap();

        assert_eq!(covers.len(), 1);
        assert_eq!(covers[0].source, "extracted");
    }

    #[tokio::test]
    async fn test_create_extracted() {
        let (db, _temp_dir) = create_test_db().await;
        let (_, book_id) = setup_test_book(db.sea_orm_connection()).await;

        let cover = BookCoversRepository::create_extracted(
            db.sea_orm_connection(),
            book_id,
            "/covers/extracted.jpg",
            true,
            Some(800),
            Some(1200),
        )
        .await
        .unwrap();

        assert_eq!(cover.source, "extracted");
        assert!(cover.is_extracted());
        assert!(cover.is_selected);
    }

    #[tokio::test]
    async fn test_create_for_plugin() {
        let (db, _temp_dir) = create_test_db().await;
        let (_, book_id) = setup_test_book(db.sea_orm_connection()).await;

        let cover = BookCoversRepository::create_for_plugin(
            db.sea_orm_connection(),
            book_id,
            "openlibrary",
            "/covers/openlibrary.jpg",
            false,
            None,
            None,
        )
        .await
        .unwrap();

        assert_eq!(cover.source, "plugin:openlibrary");
        assert!(cover.is_plugin_source());
        assert_eq!(cover.plugin_name(), Some("openlibrary"));
    }

    #[tokio::test]
    async fn test_upsert_by_source() {
        let (db, _temp_dir) = create_test_db().await;
        let (_, book_id) = setup_test_book(db.sea_orm_connection()).await;

        // First upsert creates
        let cover1 = BookCoversRepository::upsert_by_source(
            db.sea_orm_connection(),
            book_id,
            "extracted",
            "/covers/old.jpg",
            true,
            Some(400),
            Some(600),
        )
        .await
        .unwrap();

        assert_eq!(cover1.path, "/covers/old.jpg");

        // Second upsert updates
        let cover2 = BookCoversRepository::upsert_by_source(
            db.sea_orm_connection(),
            book_id,
            "extracted",
            "/covers/new.jpg",
            false,
            Some(800),
            Some(1200),
        )
        .await
        .unwrap();

        assert_eq!(cover1.id, cover2.id);
        assert_eq!(cover2.path, "/covers/new.jpg");

        // Verify only one record exists
        let covers = BookCoversRepository::list_by_book(db.sea_orm_connection(), book_id)
            .await
            .unwrap();
        assert_eq!(covers.len(), 1);
    }
}
