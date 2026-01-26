//! Repository for series_external_ratings table operations
//!
//! TODO: Remove allow(dead_code) when external rating features are fully integrated

#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    prelude::Decimal, ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use uuid::Uuid;

use crate::db::entities::series_external_ratings::{
    self, Entity as ExternalRatings, Model as ExternalRating,
};

/// Repository for series external rating operations
pub struct ExternalRatingRepository;

impl ExternalRatingRepository {
    /// Get an external rating by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<ExternalRating>> {
        let result = ExternalRatings::find_by_id(id).one(db).await?;
        Ok(result)
    }

    /// Get all external ratings for a series
    pub async fn get_for_series(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Vec<ExternalRating>> {
        let results = ExternalRatings::find()
            .filter(series_external_ratings::Column::SeriesId.eq(series_id))
            .all(db)
            .await?;
        Ok(results)
    }

    /// Get an external rating by series ID and source name
    pub async fn get_by_source(
        db: &DatabaseConnection,
        series_id: Uuid,
        source_name: &str,
    ) -> Result<Option<ExternalRating>> {
        let normalized = source_name.to_lowercase().trim().to_string();
        let result = ExternalRatings::find()
            .filter(series_external_ratings::Column::SeriesId.eq(series_id))
            .filter(series_external_ratings::Column::SourceName.eq(&normalized))
            .one(db)
            .await?;
        Ok(result)
    }

    /// Create a new external rating for a series
    pub async fn create(
        db: &DatabaseConnection,
        series_id: Uuid,
        source_name: &str,
        rating: Decimal,
        vote_count: Option<i32>,
    ) -> Result<ExternalRating> {
        let now = Utc::now();
        let normalized_source = source_name.to_lowercase().trim().to_string();

        let active_model = series_external_ratings::ActiveModel {
            id: Set(Uuid::new_v4()),
            series_id: Set(series_id),
            source_name: Set(normalized_source),
            rating: Set(rating),
            vote_count: Set(vote_count),
            fetched_at: Set(now),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let model = active_model.insert(db).await?;
        Ok(model)
    }

    /// Create or update an external rating (upsert by series_id + source_name)
    pub async fn upsert(
        db: &DatabaseConnection,
        series_id: Uuid,
        source_name: &str,
        rating: Decimal,
        vote_count: Option<i32>,
    ) -> Result<ExternalRating> {
        let existing = Self::get_by_source(db, series_id, source_name).await?;

        match existing {
            Some(existing) => {
                let mut active_model: series_external_ratings::ActiveModel = existing.into();
                let now = Utc::now();
                active_model.rating = Set(rating);
                active_model.vote_count = Set(vote_count);
                active_model.fetched_at = Set(now);
                active_model.updated_at = Set(now);

                let model = active_model.update(db).await?;
                Ok(model)
            }
            None => Self::create(db, series_id, source_name, rating, vote_count).await,
        }
    }

    /// Update an external rating by ID
    pub async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        rating: Option<Decimal>,
        vote_count: Option<Option<i32>>,
    ) -> Result<Option<ExternalRating>> {
        let existing = ExternalRatings::find_by_id(id).one(db).await?;

        let Some(existing) = existing else {
            return Ok(None);
        };

        let mut active_model: series_external_ratings::ActiveModel = existing.into();
        let now = Utc::now();
        active_model.updated_at = Set(now);
        active_model.fetched_at = Set(now);

        if let Some(rating) = rating {
            active_model.rating = Set(rating);
        }

        if let Some(vote_count) = vote_count {
            active_model.vote_count = Set(vote_count);
        }

        let model = active_model.update(db).await?;
        Ok(Some(model))
    }

    /// Delete an external rating by ID
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = ExternalRatings::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Delete an external rating by series ID and source name
    pub async fn delete_by_source(
        db: &DatabaseConnection,
        series_id: Uuid,
        source_name: &str,
    ) -> Result<bool> {
        let normalized = source_name.to_lowercase().trim().to_string();
        let result = ExternalRatings::delete_many()
            .filter(series_external_ratings::Column::SeriesId.eq(series_id))
            .filter(series_external_ratings::Column::SourceName.eq(&normalized))
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// Delete all external ratings for a series
    pub async fn delete_all_for_series(db: &DatabaseConnection, series_id: Uuid) -> Result<u64> {
        let result = ExternalRatings::delete_many()
            .filter(series_external_ratings::Column::SeriesId.eq(series_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Check if an external rating belongs to a specific series
    pub async fn belongs_to_series(
        db: &DatabaseConnection,
        id: Uuid,
        series_id: Uuid,
    ) -> Result<bool> {
        let result = ExternalRatings::find_by_id(id)
            .filter(series_external_ratings::Column::SeriesId.eq(series_id))
            .one(db)
            .await?;
        Ok(result.is_some())
    }

    /// Get external ratings for multiple series by their IDs
    ///
    /// Returns a HashMap keyed by series_id for efficient lookups
    pub async fn get_for_series_ids(
        db: &DatabaseConnection,
        series_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<ExternalRating>>> {
        if series_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let results = ExternalRatings::find()
            .filter(series_external_ratings::Column::SeriesId.is_in(series_ids.to_vec()))
            .all(db)
            .await?;

        let mut map: std::collections::HashMap<Uuid, Vec<ExternalRating>> =
            std::collections::HashMap::new();

        for rating in results {
            map.entry(rating.series_id).or_default().push(rating);
        }

        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::{LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;
    use crate::db::ScanningStrategy;

    fn dec(value: f64) -> Decimal {
        Decimal::from_f64_retain(value).unwrap()
    }

    #[tokio::test]
    async fn test_create_and_get_external_rating() {
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

        let rating = ExternalRatingRepository::create(
            db.sea_orm_connection(),
            series.id,
            "myanimelist",
            dec(85.5),
            Some(1000),
        )
        .await
        .unwrap();

        assert_eq!(rating.source_name, "myanimelist");
        assert_eq!(rating.rating, dec(85.5));
        assert_eq!(rating.vote_count, Some(1000));
        assert_eq!(rating.series_id, series.id);

        let fetched = ExternalRatingRepository::get_by_id(db.sea_orm_connection(), rating.id)
            .await
            .unwrap();
        assert!(fetched.is_some());
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

        ExternalRatingRepository::create(
            db.sea_orm_connection(),
            series.id,
            "myanimelist",
            dec(85.0),
            Some(1000),
        )
        .await
        .unwrap();

        ExternalRatingRepository::create(
            db.sea_orm_connection(),
            series.id,
            "anilist",
            dec(90.0),
            Some(500),
        )
        .await
        .unwrap();

        ExternalRatingRepository::create(
            db.sea_orm_connection(),
            series.id,
            "mangabaka",
            dec(78.5),
            None,
        )
        .await
        .unwrap();

        let ratings = ExternalRatingRepository::get_for_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();

        assert_eq!(ratings.len(), 3);
    }

    #[tokio::test]
    async fn test_get_by_source() {
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

        ExternalRatingRepository::create(
            db.sea_orm_connection(),
            series.id,
            "myanimelist",
            dec(85.0),
            Some(1000),
        )
        .await
        .unwrap();

        let rating = ExternalRatingRepository::get_by_source(
            db.sea_orm_connection(),
            series.id,
            "MyAnimeList",
        )
        .await
        .unwrap();

        assert!(rating.is_some());
        assert_eq!(rating.unwrap().source_name, "myanimelist");

        let not_found =
            ExternalRatingRepository::get_by_source(db.sea_orm_connection(), series.id, "anilist")
                .await
                .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_upsert_external_rating() {
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

        // First upsert creates
        let rating1 = ExternalRatingRepository::upsert(
            db.sea_orm_connection(),
            series.id,
            "anilist",
            dec(80.0),
            Some(500),
        )
        .await
        .unwrap();

        assert_eq!(rating1.rating, dec(80.0));
        assert_eq!(rating1.vote_count, Some(500));

        // Second upsert updates
        let rating2 = ExternalRatingRepository::upsert(
            db.sea_orm_connection(),
            series.id,
            "anilist",
            dec(85.0),
            Some(600),
        )
        .await
        .unwrap();

        assert_eq!(rating1.id, rating2.id);
        assert_eq!(rating2.rating, dec(85.0));
        assert_eq!(rating2.vote_count, Some(600));

        // Verify only one rating exists
        let ratings = ExternalRatingRepository::get_for_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert_eq!(ratings.len(), 1);
    }

    #[tokio::test]
    async fn test_update_external_rating() {
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

        let rating = ExternalRatingRepository::create(
            db.sea_orm_connection(),
            series.id,
            "anilist",
            dec(80.0),
            Some(500),
        )
        .await
        .unwrap();

        // Update rating only
        let updated = ExternalRatingRepository::update(
            db.sea_orm_connection(),
            rating.id,
            Some(dec(90.0)),
            None,
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(updated.rating, dec(90.0));
        assert_eq!(updated.vote_count, Some(500));

        // Update vote count only
        let updated = ExternalRatingRepository::update(
            db.sea_orm_connection(),
            rating.id,
            None,
            Some(Some(700)),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(updated.rating, dec(90.0));
        assert_eq!(updated.vote_count, Some(700));

        // Update vote count to None
        let updated =
            ExternalRatingRepository::update(db.sea_orm_connection(), rating.id, None, Some(None))
                .await
                .unwrap()
                .unwrap();

        assert_eq!(updated.vote_count, None);
    }

    #[tokio::test]
    async fn test_delete_external_rating() {
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

        let rating = ExternalRatingRepository::create(
            db.sea_orm_connection(),
            series.id,
            "anilist",
            dec(80.0),
            None,
        )
        .await
        .unwrap();

        let deleted = ExternalRatingRepository::delete(db.sea_orm_connection(), rating.id)
            .await
            .unwrap();
        assert!(deleted);

        let fetched = ExternalRatingRepository::get_by_id(db.sea_orm_connection(), rating.id)
            .await
            .unwrap();
        assert!(fetched.is_none());
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

        ExternalRatingRepository::create(
            db.sea_orm_connection(),
            series.id,
            "anilist",
            dec(80.0),
            None,
        )
        .await
        .unwrap();

        let deleted = ExternalRatingRepository::delete_by_source(
            db.sea_orm_connection(),
            series.id,
            "AniList",
        )
        .await
        .unwrap();
        assert!(deleted);

        let fetched =
            ExternalRatingRepository::get_by_source(db.sea_orm_connection(), series.id, "anilist")
                .await
                .unwrap();
        assert!(fetched.is_none());
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

        for source in ["mal", "anilist", "mangabaka"] {
            ExternalRatingRepository::create(
                db.sea_orm_connection(),
                series.id,
                source,
                dec(80.0),
                None,
            )
            .await
            .unwrap();
        }

        let count =
            ExternalRatingRepository::delete_all_for_series(db.sea_orm_connection(), series.id)
                .await
                .unwrap();

        assert_eq!(count, 3);

        let remaining =
            ExternalRatingRepository::get_for_series(db.sea_orm_connection(), series.id)
                .await
                .unwrap();
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn test_source_name_normalization() {
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

        let rating = ExternalRatingRepository::create(
            db.sea_orm_connection(),
            series.id,
            "  MyAnimeList  ",
            dec(85.0),
            None,
        )
        .await
        .unwrap();

        assert_eq!(rating.source_name, "myanimelist");

        // Should find with different case
        let found = ExternalRatingRepository::get_by_source(
            db.sea_orm_connection(),
            series.id,
            "MYANIMELIST",
        )
        .await
        .unwrap();
        assert!(found.is_some());
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

        let rating = ExternalRatingRepository::create(
            db.sea_orm_connection(),
            series1.id,
            "mal",
            dec(80.0),
            None,
        )
        .await
        .unwrap();

        let belongs = ExternalRatingRepository::belongs_to_series(
            db.sea_orm_connection(),
            rating.id,
            series1.id,
        )
        .await
        .unwrap();
        assert!(belongs);

        let belongs = ExternalRatingRepository::belongs_to_series(
            db.sea_orm_connection(),
            rating.id,
            series2.id,
        )
        .await
        .unwrap();
        assert!(!belongs);
    }
}
