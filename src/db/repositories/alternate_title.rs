//! Repository for series_alternate_titles table operations
//!
//! TODO: Remove allow(dead_code) when alternate title features are fully integrated

#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::db::entities::series_alternate_titles::{
    self, Entity as AlternateTitles, Model as AlternateTitle,
};

/// Repository for series alternate title operations
pub struct AlternateTitleRepository;

impl AlternateTitleRepository {
    /// Get an alternate title by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<AlternateTitle>> {
        let result = AlternateTitles::find_by_id(id).one(db).await?;
        Ok(result)
    }

    /// Get all alternate titles for a series
    pub async fn get_for_series(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Vec<AlternateTitle>> {
        let results = AlternateTitles::find()
            .filter(series_alternate_titles::Column::SeriesId.eq(series_id))
            .all(db)
            .await?;
        Ok(results)
    }

    /// Create a new alternate title for a series
    pub async fn create(
        db: &DatabaseConnection,
        series_id: Uuid,
        label: &str,
        title: &str,
    ) -> Result<AlternateTitle> {
        let now = Utc::now();
        let active_model = series_alternate_titles::ActiveModel {
            id: Set(Uuid::new_v4()),
            series_id: Set(series_id),
            label: Set(label.trim().to_string()),
            title: Set(title.trim().to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let model = active_model.insert(db).await?;
        Ok(model)
    }

    /// Update an alternate title
    pub async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        label: Option<&str>,
        title: Option<&str>,
    ) -> Result<Option<AlternateTitle>> {
        let existing = AlternateTitles::find_by_id(id).one(db).await?;

        let Some(existing) = existing else {
            return Ok(None);
        };

        let mut active_model: series_alternate_titles::ActiveModel = existing.into();
        active_model.updated_at = Set(Utc::now());

        if let Some(label) = label {
            active_model.label = Set(label.trim().to_string());
        }

        if let Some(title) = title {
            active_model.title = Set(title.trim().to_string());
        }

        let model = active_model.update(db).await?;
        Ok(Some(model))
    }

    /// Delete an alternate title by ID
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = AlternateTitles::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Delete all alternate titles for a series
    pub async fn delete_all_for_series(db: &DatabaseConnection, series_id: Uuid) -> Result<u64> {
        let result = AlternateTitles::delete_many()
            .filter(series_alternate_titles::Column::SeriesId.eq(series_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Check if an alternate title belongs to a specific series
    pub async fn belongs_to_series(
        db: &DatabaseConnection,
        id: Uuid,
        series_id: Uuid,
    ) -> Result<bool> {
        let result = AlternateTitles::find_by_id(id)
            .filter(series_alternate_titles::Column::SeriesId.eq(series_id))
            .one(db)
            .await?;
        Ok(result.is_some())
    }

    /// Get alternate titles for multiple series by their IDs
    ///
    /// Returns a HashMap keyed by series_id for efficient lookups
    pub async fn get_for_series_ids(
        db: &DatabaseConnection,
        series_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<AlternateTitle>>> {
        if series_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let results = AlternateTitles::find()
            .filter(series_alternate_titles::Column::SeriesId.is_in(series_ids.to_vec()))
            .all(db)
            .await?;

        let mut map: std::collections::HashMap<Uuid, Vec<AlternateTitle>> =
            std::collections::HashMap::new();

        for alt_title in results {
            map.entry(alt_title.series_id).or_default().push(alt_title);
        }

        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::repositories::{LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;

    #[tokio::test]
    async fn test_create_and_get_alternate_title() {
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

        let alt_title = AlternateTitleRepository::create(
            db.sea_orm_connection(),
            series.id,
            "Japanese",
            "テストシリーズ",
        )
        .await
        .unwrap();

        assert_eq!(alt_title.label, "Japanese");
        assert_eq!(alt_title.title, "テストシリーズ");
        assert_eq!(alt_title.series_id, series.id);

        let fetched = AlternateTitleRepository::get_by_id(db.sea_orm_connection(), alt_title.id)
            .await
            .unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().title, "テストシリーズ");
    }

    #[tokio::test]
    async fn test_get_for_series() {
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

        // Create multiple alternate titles
        AlternateTitleRepository::create(
            db.sea_orm_connection(),
            series.id,
            "Japanese",
            "日本語タイトル",
        )
        .await
        .unwrap();

        AlternateTitleRepository::create(
            db.sea_orm_connection(),
            series.id,
            "Romaji",
            "Nihongo Taitoru",
        )
        .await
        .unwrap();

        AlternateTitleRepository::create(
            db.sea_orm_connection(),
            series.id,
            "Korean",
            "한국어 제목",
        )
        .await
        .unwrap();

        let titles = AlternateTitleRepository::get_for_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();

        assert_eq!(titles.len(), 3);
    }

    #[tokio::test]
    async fn test_update_alternate_title() {
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

        let alt_title = AlternateTitleRepository::create(
            db.sea_orm_connection(),
            series.id,
            "Japanese",
            "Original Title",
        )
        .await
        .unwrap();

        // Update label only
        let updated = AlternateTitleRepository::update(
            db.sea_orm_connection(),
            alt_title.id,
            Some("Romaji"),
            None,
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(updated.label, "Romaji");
        assert_eq!(updated.title, "Original Title");

        // Update title only
        let updated = AlternateTitleRepository::update(
            db.sea_orm_connection(),
            alt_title.id,
            None,
            Some("Updated Title"),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(updated.label, "Romaji");
        assert_eq!(updated.title, "Updated Title");

        // Update both
        let updated = AlternateTitleRepository::update(
            db.sea_orm_connection(),
            alt_title.id,
            Some("English"),
            Some("Final Title"),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(updated.label, "English");
        assert_eq!(updated.title, "Final Title");
    }

    #[tokio::test]
    async fn test_update_nonexistent_title() {
        let (db, _temp_dir) = create_test_db().await;

        let result = AlternateTitleRepository::update(
            db.sea_orm_connection(),
            Uuid::new_v4(),
            Some("Label"),
            Some("Title"),
        )
        .await
        .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete_alternate_title() {
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

        let alt_title = AlternateTitleRepository::create(
            db.sea_orm_connection(),
            series.id,
            "Japanese",
            "タイトル",
        )
        .await
        .unwrap();

        let deleted = AlternateTitleRepository::delete(db.sea_orm_connection(), alt_title.id)
            .await
            .unwrap();
        assert!(deleted);

        let fetched = AlternateTitleRepository::get_by_id(db.sea_orm_connection(), alt_title.id)
            .await
            .unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_title() {
        let (db, _temp_dir) = create_test_db().await;

        let deleted = AlternateTitleRepository::delete(db.sea_orm_connection(), Uuid::new_v4())
            .await
            .unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_delete_all_for_series() {
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

        // Create multiple alternate titles
        for i in 1..=5 {
            AlternateTitleRepository::create(
                db.sea_orm_connection(),
                series.id,
                &format!("Label {}", i),
                &format!("Title {}", i),
            )
            .await
            .unwrap();
        }

        let count =
            AlternateTitleRepository::delete_all_for_series(db.sea_orm_connection(), series.id)
                .await
                .unwrap();

        assert_eq!(count, 5);

        let remaining =
            AlternateTitleRepository::get_for_series(db.sea_orm_connection(), series.id)
                .await
                .unwrap();
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn test_belongs_to_series() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let series1 =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 1", None)
                .await
                .unwrap();

        let series2 =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 2", None)
                .await
                .unwrap();

        let alt_title =
            AlternateTitleRepository::create(db.sea_orm_connection(), series1.id, "Label", "Title")
                .await
                .unwrap();

        let belongs = AlternateTitleRepository::belongs_to_series(
            db.sea_orm_connection(),
            alt_title.id,
            series1.id,
        )
        .await
        .unwrap();
        assert!(belongs);

        let belongs = AlternateTitleRepository::belongs_to_series(
            db.sea_orm_connection(),
            alt_title.id,
            series2.id,
        )
        .await
        .unwrap();
        assert!(!belongs);
    }

    #[tokio::test]
    async fn test_label_and_title_trimming() {
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

        let alt_title = AlternateTitleRepository::create(
            db.sea_orm_connection(),
            series.id,
            "  Spaced Label  ",
            "  Spaced Title  ",
        )
        .await
        .unwrap();

        assert_eq!(alt_title.label, "Spaced Label");
        assert_eq!(alt_title.title, "Spaced Title");
    }
}
