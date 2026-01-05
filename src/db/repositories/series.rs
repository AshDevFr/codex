use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use uuid::Uuid;

use crate::db::entities::{prelude::*, series};

/// Repository for Series operations
pub struct SeriesRepository;

impl SeriesRepository {
    /// Normalize name for searching (lowercase, alphanumeric only)
    fn normalize_name(name: &str) -> String {
        name.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Create a new series
    pub async fn create(
        db: &DatabaseConnection,
        library_id: Uuid,
        name: &str,
    ) -> Result<series::Model> {
        Self::create_with_fingerprint(db, library_id, name, None).await
    }

    /// Create a new series with optional fingerprint
    pub async fn create_with_fingerprint(
        db: &DatabaseConnection,
        library_id: Uuid,
        name: &str,
        fingerprint: Option<String>,
    ) -> Result<series::Model> {
        let now = Utc::now();
        let normalized_name = Self::normalize_name(name);

        let series = series::ActiveModel {
            id: Set(Uuid::new_v4()),
            library_id: Set(library_id),
            name: Set(name.to_string()),
            normalized_name: Set(normalized_name),
            sort_name: Set(None),
            summary: Set(None),
            publisher: Set(None),
            year: Set(None),
            book_count: Set(0),
            user_rating: Set(None),
            external_rating: Set(None),
            external_rating_count: Set(None),
            external_rating_source: Set(None),
            custom_metadata: Set(None),
            fingerprint: Set(fingerprint),
            created_at: Set(now),
            updated_at: Set(now),
        };

        series.insert(db).await.context("Failed to create series")
    }

    /// Get a series by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<series::Model>> {
        Series::find_by_id(id)
            .one(db)
            .await
            .context("Failed to get series by ID")
    }

    /// Get all series in a library
    pub async fn list_by_library(
        db: &DatabaseConnection,
        library_id: Uuid,
    ) -> Result<Vec<series::Model>> {
        Series::find()
            .filter(series::Column::LibraryId.eq(library_id))
            .order_by_asc(series::Column::SortName)
            .order_by_asc(series::Column::Name)
            .all(db)
            .await
            .context("Failed to list series by library")
    }

    /// Get all series across all libraries
    pub async fn list_all(db: &DatabaseConnection) -> Result<Vec<series::Model>> {
        Series::find()
            .order_by_asc(series::Column::SortName)
            .order_by_asc(series::Column::Name)
            .all(db)
            .await
            .context("Failed to list all series")
    }

    /// Search series by normalized name
    pub async fn search_by_name(
        db: &DatabaseConnection,
        query: &str,
    ) -> Result<Vec<series::Model>> {
        let pattern = format!("%{}%", query.to_lowercase());

        Series::find()
            .filter(series::Column::NormalizedName.contains(&pattern))
            .order_by_asc(series::Column::Name)
            .limit(50)
            .all(db)
            .await
            .context("Failed to search series by name")
    }

    /// Update series
    pub async fn update(db: &DatabaseConnection, series_model: &series::Model) -> Result<()> {
        let active = series::ActiveModel {
            id: Set(series_model.id),
            library_id: Set(series_model.library_id),
            name: Set(series_model.name.clone()),
            normalized_name: Set(series_model.normalized_name.clone()),
            sort_name: Set(series_model.sort_name.clone()),
            summary: Set(series_model.summary.clone()),
            publisher: Set(series_model.publisher.clone()),
            year: Set(series_model.year),
            book_count: Set(series_model.book_count),
            user_rating: Set(series_model.user_rating),
            external_rating: Set(series_model.external_rating),
            external_rating_count: Set(series_model.external_rating_count),
            external_rating_source: Set(series_model.external_rating_source.clone()),
            custom_metadata: Set(series_model.custom_metadata.clone()),
            fingerprint: Set(series_model.fingerprint.clone()),
            created_at: Set(series_model.created_at),
            updated_at: Set(Utc::now()),
        };

        active.update(db).await.context("Failed to update series")?;

        Ok(())
    }

    /// Update series name (useful when folder is renamed but fingerprint matches)
    pub async fn update_name(db: &DatabaseConnection, id: Uuid, name: &str) -> Result<()> {
        let series = Series::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Series not found"))?;

        let normalized_name = Self::normalize_name(name);

        let mut active: series::ActiveModel = series.into();
        active.name = Set(name.to_string());
        active.normalized_name = Set(normalized_name);
        active.updated_at = Set(Utc::now());

        active
            .update(db)
            .await
            .context("Failed to update series name")?;

        Ok(())
    }

    /// Update series fingerprint
    pub async fn update_fingerprint(
        db: &DatabaseConnection,
        id: Uuid,
        fingerprint: Option<String>,
    ) -> Result<()> {
        let series = Series::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Series not found"))?;

        let mut active: series::ActiveModel = series.into();
        active.fingerprint = Set(fingerprint);
        active.updated_at = Set(Utc::now());

        active
            .update(db)
            .await
            .context("Failed to update series fingerprint")?;

        Ok(())
    }

    /// Increment book count for a series
    pub async fn increment_book_count(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        let series_model = Series::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Series not found"))?;

        let mut active: series::ActiveModel = series_model.into();
        active.book_count = Set(active.book_count.unwrap() + 1);
        active.updated_at = Set(Utc::now());

        active
            .update(db)
            .await
            .context("Failed to increment book count")?;

        Ok(())
    }

    /// Delete a series
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        Series::delete_by_id(id)
            .exec(db)
            .await
            .context("Failed to delete series")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::LibraryRepository;
    use crate::db::test_helpers::create_test_db;
    use crate::db::ScanningStrategy;

    #[tokio::test]
    async fn test_create_series() {
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

        assert_eq!(series.name, "Test Series");
        assert_eq!(series.library_id, library.id);
        assert_eq!(series.book_count, 0);
        assert_eq!(series.normalized_name, "test series");
    }

    #[tokio::test]
    async fn test_get_series_by_id() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let created = SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series")
            .await
            .unwrap();

        let retrieved = SeriesRepository::get_by_id(db.sea_orm_connection(), created.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.name, "Test Series");
    }

    #[tokio::test]
    async fn test_list_series_by_library() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 1")
            .await
            .unwrap();
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 2")
            .await
            .unwrap();

        let series = SeriesRepository::list_by_library(db.sea_orm_connection(), library.id)
            .await
            .unwrap();

        assert_eq!(series.len(), 2);
    }

    #[tokio::test]
    async fn test_search_series_by_name() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        SeriesRepository::create(db.sea_orm_connection(), library.id, "One Piece")
            .await
            .unwrap();
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Naruto")
            .await
            .unwrap();

        let results = SeriesRepository::search_by_name(db.sea_orm_connection(), "piece")
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "One Piece");
    }

    #[tokio::test]
    async fn test_update_series() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let mut series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Original Name")
                .await
                .unwrap();

        series.name = "Updated Name".to_string();
        series.normalized_name = SeriesRepository::normalize_name(&series.name);
        series.summary = Some("Updated summary".to_string());

        SeriesRepository::update(db.sea_orm_connection(), &series)
            .await
            .unwrap();

        let retrieved = SeriesRepository::get_by_id(db.sea_orm_connection(), series.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.name, "Updated Name");
        assert_eq!(retrieved.summary, Some("Updated summary".to_string()));
    }

    #[tokio::test]
    async fn test_increment_book_count() {
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

        assert_eq!(series.book_count, 0);

        SeriesRepository::increment_book_count(db.sea_orm_connection(), series.id)
            .await
            .unwrap();

        let retrieved = SeriesRepository::get_by_id(db.sea_orm_connection(), series.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.book_count, 1);
    }

    #[tokio::test]
    async fn test_delete_series() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let series = SeriesRepository::create(db.sea_orm_connection(), library.id, "To Delete")
            .await
            .unwrap();

        SeriesRepository::delete(db.sea_orm_connection(), series.id)
            .await
            .unwrap();

        let result = SeriesRepository::get_by_id(db.sea_orm_connection(), series.id)
            .await
            .unwrap();

        assert!(result.is_none());
    }
}
