//! Repository for series_covers table operations
//!
//! TODO: Remove allow(dead_code) when all cover features are fully integrated

#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::db::entities::{series_covers, series_covers::Entity as SeriesCovers};

/// Repository for series cover operations
pub struct SeriesCoversRepository;

impl SeriesCoversRepository {
    /// Get all covers for a series
    pub async fn list_by_series(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Vec<series_covers::Model>> {
        let results = SeriesCovers::find()
            .filter(series_covers::Column::SeriesId.eq(series_id))
            .order_by_asc(series_covers::Column::CreatedAt)
            .all(db)
            .await?;
        Ok(results)
    }

    /// Get the selected (primary) cover for a series
    pub async fn get_selected(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Option<series_covers::Model>> {
        let result = SeriesCovers::find()
            .filter(series_covers::Column::SeriesId.eq(series_id))
            .filter(series_covers::Column::IsSelected.eq(true))
            .one(db)
            .await?;
        Ok(result)
    }

    /// Get a cover by its ID
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<series_covers::Model>> {
        let result = SeriesCovers::find_by_id(id).one(db).await?;
        Ok(result)
    }

    /// Get a cover by series and source
    pub async fn get_by_source(
        db: &DatabaseConnection,
        series_id: Uuid,
        source: &str,
    ) -> Result<Option<series_covers::Model>> {
        let result = SeriesCovers::find()
            .filter(series_covers::Column::SeriesId.eq(series_id))
            .filter(series_covers::Column::Source.eq(source))
            .one(db)
            .await?;
        Ok(result)
    }

    /// Create a new cover for a series
    pub async fn create(
        db: &DatabaseConnection,
        series_id: Uuid,
        source: &str,
        path: &str,
        is_selected: bool,
        width: Option<i32>,
        height: Option<i32>,
    ) -> Result<series_covers::Model> {
        let now = Utc::now();

        // If this is being selected, deselect any existing selected covers
        if is_selected {
            Self::deselect_all(db, series_id).await?;
        }

        let active_model = series_covers::ActiveModel {
            id: Set(Uuid::new_v4()),
            series_id: Set(series_id),
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

    /// Set a specific cover as selected (and deselect others)
    pub async fn select_cover(
        db: &DatabaseConnection,
        series_id: Uuid,
        cover_id: Uuid,
    ) -> Result<series_covers::Model> {
        // First deselect all covers for this series
        Self::deselect_all(db, series_id).await?;

        // Now select the specified cover
        let cover = Self::get_by_id(db, cover_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Cover not found: {}", cover_id))?;

        if cover.series_id != series_id {
            return Err(anyhow::anyhow!(
                "Cover {} does not belong to series {}",
                cover_id,
                series_id
            ));
        }

        let mut active_model: series_covers::ActiveModel = cover.into();
        active_model.is_selected = Set(true);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Select a cover by source (e.g., "custom", "book:uuid")
    pub async fn select_by_source(
        db: &DatabaseConnection,
        series_id: Uuid,
        source: &str,
    ) -> Result<Option<series_covers::Model>> {
        // First check if the source exists
        let cover = match Self::get_by_source(db, series_id, source).await? {
            Some(c) => c,
            None => return Ok(None),
        };

        // Deselect all and select this one
        Self::deselect_all(db, series_id).await?;

        let mut active_model: series_covers::ActiveModel = cover.into();
        active_model.is_selected = Set(true);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(Some(model))
    }

    /// Deselect all covers for a series (resets to default thumbnail)
    pub async fn deselect_all(db: &DatabaseConnection, series_id: Uuid) -> Result<()> {
        use sea_orm::sea_query::Expr;

        SeriesCovers::update_many()
            .filter(series_covers::Column::SeriesId.eq(series_id))
            .filter(series_covers::Column::IsSelected.eq(true))
            .col_expr(series_covers::Column::IsSelected, Expr::value(false))
            .col_expr(series_covers::Column::UpdatedAt, Expr::value(Utc::now()))
            .exec(db)
            .await?;

        Ok(())
    }

    /// Update cover path
    pub async fn update_path(
        db: &DatabaseConnection,
        id: Uuid,
        path: &str,
    ) -> Result<series_covers::Model> {
        let cover = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Cover not found: {}", id))?;

        let mut active_model: series_covers::ActiveModel = cover.into();
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
    ) -> Result<series_covers::Model> {
        let cover = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Cover not found: {}", id))?;

        let mut active_model: series_covers::ActiveModel = cover.into();
        active_model.width = Set(width);
        active_model.height = Set(height);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Delete a cover by ID
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        SeriesCovers::delete_by_id(id).exec(db).await?;
        Ok(())
    }

    /// Delete all covers for a series
    pub async fn delete_by_series(db: &DatabaseConnection, series_id: Uuid) -> Result<()> {
        SeriesCovers::delete_many()
            .filter(series_covers::Column::SeriesId.eq(series_id))
            .exec(db)
            .await?;
        Ok(())
    }

    /// Delete cover by source (e.g., delete the "custom" cover)
    pub async fn delete_by_source(
        db: &DatabaseConnection,
        series_id: Uuid,
        source: &str,
    ) -> Result<()> {
        SeriesCovers::delete_many()
            .filter(series_covers::Column::SeriesId.eq(series_id))
            .filter(series_covers::Column::Source.eq(source))
            .exec(db)
            .await?;
        Ok(())
    }

    /// Check if a series has a custom cover
    pub async fn has_custom_cover(db: &DatabaseConnection, series_id: Uuid) -> Result<bool> {
        let result = Self::get_by_source(db, series_id, "custom").await?;
        Ok(result.is_some())
    }

    /// Get the selected cover source for a series (e.g., "custom", "book:uuid", or None)
    pub async fn get_selected_source(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Option<String>> {
        let selected = Self::get_selected(db, series_id).await?;
        Ok(selected.map(|c| c.source))
    }

    /// Get selected covers for multiple series by their IDs
    ///
    /// Returns a HashMap keyed by series_id for efficient lookups
    pub async fn get_selected_for_series_ids(
        db: &DatabaseConnection,
        series_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, series_covers::Model>> {
        if series_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let results = SeriesCovers::find()
            .filter(series_covers::Column::SeriesId.is_in(series_ids.to_vec()))
            .filter(series_covers::Column::IsSelected.eq(true))
            .all(db)
            .await?;

        Ok(results.into_iter().map(|c| (c.series_id, c)).collect())
    }

    /// Check if multiple series have custom covers
    ///
    /// Returns a HashMap keyed by series_id with boolean values
    pub async fn has_custom_cover_for_series_ids(
        db: &DatabaseConnection,
        series_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, bool>> {
        if series_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let results = SeriesCovers::find()
            .filter(series_covers::Column::SeriesId.is_in(series_ids.to_vec()))
            .filter(series_covers::Column::Source.eq("custom"))
            .all(db)
            .await?;

        let custom_cover_set: std::collections::HashSet<Uuid> =
            results.into_iter().map(|c| c.series_id).collect();

        Ok(series_ids
            .iter()
            .map(|id| (*id, custom_cover_set.contains(id)))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::repositories::{LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;

    #[tokio::test]
    async fn test_create_and_list_covers() {
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

        // Create first cover (selected)
        let cover1 = SeriesCoversRepository::create(
            db.sea_orm_connection(),
            series.id,
            "book:123",
            "/covers/123.jpg",
            true,
            Some(800),
            Some(1200),
        )
        .await
        .unwrap();

        assert!(cover1.is_selected);
        assert_eq!(cover1.source, "book:123");

        // Create second cover (not selected)
        let cover2 = SeriesCoversRepository::create(
            db.sea_orm_connection(),
            series.id,
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
        let covers = SeriesCoversRepository::list_by_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();

        assert_eq!(covers.len(), 2);
    }

    #[tokio::test]
    async fn test_select_cover() {
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

        // Create two covers
        let cover1 = SeriesCoversRepository::create(
            db.sea_orm_connection(),
            series.id,
            "book:123",
            "/covers/123.jpg",
            true,
            None,
            None,
        )
        .await
        .unwrap();

        let cover2 = SeriesCoversRepository::create(
            db.sea_orm_connection(),
            series.id,
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
            SeriesCoversRepository::select_cover(db.sea_orm_connection(), series.id, cover2.id)
                .await
                .unwrap();

        assert!(selected.is_selected);
        assert_eq!(selected.source, "custom");

        // Verify first cover is now deselected
        let cover1_updated = SeriesCoversRepository::get_by_id(db.sea_orm_connection(), cover1.id)
            .await
            .unwrap()
            .unwrap();

        assert!(!cover1_updated.is_selected);
    }

    #[tokio::test]
    async fn test_get_selected() {
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

        // No covers yet
        let selected = SeriesCoversRepository::get_selected(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert!(selected.is_none());

        // Create a selected cover
        SeriesCoversRepository::create(
            db.sea_orm_connection(),
            series.id,
            "custom",
            "/covers/custom.jpg",
            true,
            None,
            None,
        )
        .await
        .unwrap();

        let selected = SeriesCoversRepository::get_selected(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().source, "custom");
    }

    #[tokio::test]
    async fn test_has_custom_cover() {
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

        // No custom cover
        let has_custom =
            SeriesCoversRepository::has_custom_cover(db.sea_orm_connection(), series.id)
                .await
                .unwrap();
        assert!(!has_custom);

        // Add a custom cover
        SeriesCoversRepository::create(
            db.sea_orm_connection(),
            series.id,
            "custom",
            "/covers/custom.jpg",
            false,
            None,
            None,
        )
        .await
        .unwrap();

        let has_custom =
            SeriesCoversRepository::has_custom_cover(db.sea_orm_connection(), series.id)
                .await
                .unwrap();
        assert!(has_custom);
    }

    #[tokio::test]
    async fn test_delete_by_source() {
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

        // Create covers
        SeriesCoversRepository::create(
            db.sea_orm_connection(),
            series.id,
            "book:123",
            "/covers/123.jpg",
            true,
            None,
            None,
        )
        .await
        .unwrap();

        SeriesCoversRepository::create(
            db.sea_orm_connection(),
            series.id,
            "custom",
            "/covers/custom.jpg",
            false,
            None,
            None,
        )
        .await
        .unwrap();

        // Delete custom cover
        SeriesCoversRepository::delete_by_source(db.sea_orm_connection(), series.id, "custom")
            .await
            .unwrap();

        let covers = SeriesCoversRepository::list_by_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();

        assert_eq!(covers.len(), 1);
        assert_eq!(covers[0].source, "book:123");
    }
}
