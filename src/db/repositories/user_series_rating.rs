//! Repository for user_series_ratings table operations
// TODO: Remove once all repository methods are used by API handlers
#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use uuid::Uuid;

use crate::db::entities::{user_series_ratings, user_series_ratings::Entity as UserSeriesRatings};

/// Repository for user series rating operations
pub struct UserSeriesRatingRepository;

impl UserSeriesRatingRepository {
    /// Get a rating by ID
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<user_series_ratings::Model>> {
        let result = UserSeriesRatings::find_by_id(id).one(db).await?;
        Ok(result)
    }

    /// Get a user's rating for a specific series
    pub async fn get_by_user_and_series(
        db: &DatabaseConnection,
        user_id: Uuid,
        series_id: Uuid,
    ) -> Result<Option<user_series_ratings::Model>> {
        let result = UserSeriesRatings::find()
            .filter(user_series_ratings::Column::UserId.eq(user_id))
            .filter(user_series_ratings::Column::SeriesId.eq(series_id))
            .one(db)
            .await?;
        Ok(result)
    }

    /// Get all ratings for a user, ordered by most recent
    pub async fn get_all_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Vec<user_series_ratings::Model>> {
        let results = UserSeriesRatings::find()
            .filter(user_series_ratings::Column::UserId.eq(user_id))
            .order_by_desc(user_series_ratings::Column::UpdatedAt)
            .all(db)
            .await?;
        Ok(results)
    }

    /// Get all ratings for a series, ordered by most recent
    pub async fn get_all_for_series(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Vec<user_series_ratings::Model>> {
        let results = UserSeriesRatings::find()
            .filter(user_series_ratings::Column::SeriesId.eq(series_id))
            .order_by_desc(user_series_ratings::Column::UpdatedAt)
            .all(db)
            .await?;
        Ok(results)
    }

    /// Create a new rating
    pub async fn create(
        db: &DatabaseConnection,
        user_id: Uuid,
        series_id: Uuid,
        rating: i32,
        notes: Option<String>,
    ) -> Result<user_series_ratings::Model> {
        // Validate rating is in range 1-100
        if !(1..=100).contains(&rating) {
            return Err(anyhow::anyhow!(
                "Rating must be between 1 and 100, got {}",
                rating
            ));
        }

        let now = Utc::now();
        let active_model = user_series_ratings::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            series_id: Set(series_id),
            rating: Set(rating),
            notes: Set(notes),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let model = active_model.insert(db).await?;
        Ok(model)
    }

    /// Update an existing rating
    pub async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        rating: i32,
        notes: Option<String>,
    ) -> Result<user_series_ratings::Model> {
        // Validate rating is in range 1-100
        if !(1..=100).contains(&rating) {
            return Err(anyhow::anyhow!(
                "Rating must be between 1 and 100, got {}",
                rating
            ));
        }

        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Rating not found: {}", id))?;

        let mut active_model: user_series_ratings::ActiveModel = existing.into();
        active_model.rating = Set(rating);
        active_model.notes = Set(notes);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Upsert a rating (create or update)
    /// If a rating already exists for user/series, updates it; otherwise creates new
    pub async fn upsert(
        db: &DatabaseConnection,
        user_id: Uuid,
        series_id: Uuid,
        rating: i32,
        notes: Option<String>,
    ) -> Result<user_series_ratings::Model> {
        if let Some(existing) = Self::get_by_user_and_series(db, user_id, series_id).await? {
            Self::update(db, existing.id, rating, notes).await
        } else {
            Self::create(db, user_id, series_id, rating, notes).await
        }
    }

    /// Delete a rating by ID
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = UserSeriesRatings::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Delete a user's rating for a specific series
    pub async fn delete_by_user_and_series(
        db: &DatabaseConnection,
        user_id: Uuid,
        series_id: Uuid,
    ) -> Result<bool> {
        let result = UserSeriesRatings::delete_many()
            .filter(user_series_ratings::Column::UserId.eq(user_id))
            .filter(user_series_ratings::Column::SeriesId.eq(series_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// Calculate average rating for a series
    pub async fn calculate_average_for_series(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Option<f64>> {
        let ratings = Self::get_all_for_series(db, series_id).await?;
        if ratings.is_empty() {
            return Ok(None);
        }

        let sum: i32 = ratings.iter().map(|r| r.rating).sum();
        let count = ratings.len() as f64;
        Ok(Some(sum as f64 / count))
    }

    /// Count ratings for a series
    pub async fn count_for_series(db: &DatabaseConnection, series_id: Uuid) -> Result<u64> {
        let count = UserSeriesRatings::find()
            .filter(user_series_ratings::Column::SeriesId.eq(series_id))
            .count(db)
            .await?;
        Ok(count)
    }

    /// Count ratings by a user
    pub async fn count_for_user(db: &DatabaseConnection, user_id: Uuid) -> Result<u64> {
        let count = UserSeriesRatings::find()
            .filter(user_series_ratings::Column::UserId.eq(user_id))
            .count(db)
            .await?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::users;
    use crate::db::repositories::{LibraryRepository, SeriesRepository, UserRepository};
    use crate::db::test_helpers::create_test_db;
    use crate::db::ScanningStrategy;

    fn create_user_model(email: &str) -> users::Model {
        users::Model {
            id: Uuid::new_v4(),
            username: email.split('@').next().unwrap_or("testuser").to_string(),
            email: email.to_string(),
            password_hash: "hashedpassword".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: true,
            permissions: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        }
    }

    async fn create_test_user(
        db: &DatabaseConnection,
        email: &str,
    ) -> crate::db::entities::users::Model {
        let user_model = create_user_model(email);
        UserRepository::create(db, &user_model).await.unwrap()
    }

    #[tokio::test]
    async fn test_create_and_get_rating() {
        let (db, _temp_dir) = create_test_db().await;

        let user = create_test_user(db.sea_orm_connection(), "test@example.com").await;

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

        let rating = UserSeriesRatingRepository::create(
            db.sea_orm_connection(),
            user.id,
            series.id,
            85,
            Some("Great series!".to_string()),
        )
        .await
        .unwrap();

        assert_eq!(rating.user_id, user.id);
        assert_eq!(rating.series_id, series.id);
        assert_eq!(rating.rating, 85);
        assert_eq!(rating.notes, Some("Great series!".to_string()));

        let fetched = UserSeriesRatingRepository::get_by_id(db.sea_orm_connection(), rating.id)
            .await
            .unwrap();
        assert!(fetched.is_some());
    }

    #[tokio::test]
    async fn test_get_by_user_and_series() {
        let (db, _temp_dir) = create_test_db().await;

        let user = create_test_user(db.sea_orm_connection(), "test@example.com").await;

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

        // No rating yet
        let fetched = UserSeriesRatingRepository::get_by_user_and_series(
            db.sea_orm_connection(),
            user.id,
            series.id,
        )
        .await
        .unwrap();
        assert!(fetched.is_none());

        // Create rating
        UserSeriesRatingRepository::create(db.sea_orm_connection(), user.id, series.id, 75, None)
            .await
            .unwrap();

        let fetched = UserSeriesRatingRepository::get_by_user_and_series(
            db.sea_orm_connection(),
            user.id,
            series.id,
        )
        .await
        .unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().rating, 75);
    }

    #[tokio::test]
    async fn test_update_rating() {
        let (db, _temp_dir) = create_test_db().await;

        let user = create_test_user(db.sea_orm_connection(), "test@example.com").await;

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

        let rating = UserSeriesRatingRepository::create(
            db.sea_orm_connection(),
            user.id,
            series.id,
            50,
            None,
        )
        .await
        .unwrap();

        let updated = UserSeriesRatingRepository::update(
            db.sea_orm_connection(),
            rating.id,
            90,
            Some("Changed my mind!".to_string()),
        )
        .await
        .unwrap();

        assert_eq!(updated.rating, 90);
        assert_eq!(updated.notes, Some("Changed my mind!".to_string()));
    }

    #[tokio::test]
    async fn test_upsert_rating() {
        let (db, _temp_dir) = create_test_db().await;

        let user = create_test_user(db.sea_orm_connection(), "test@example.com").await;

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
        let rating1 = UserSeriesRatingRepository::upsert(
            db.sea_orm_connection(),
            user.id,
            series.id,
            60,
            None,
        )
        .await
        .unwrap();

        assert_eq!(rating1.rating, 60);

        // Second upsert updates
        let rating2 = UserSeriesRatingRepository::upsert(
            db.sea_orm_connection(),
            user.id,
            series.id,
            80,
            Some("Updated notes".to_string()),
        )
        .await
        .unwrap();

        assert_eq!(rating2.id, rating1.id); // Same ID
        assert_eq!(rating2.rating, 80);
        assert_eq!(rating2.notes, Some("Updated notes".to_string()));
    }

    #[tokio::test]
    async fn test_rating_validation() {
        let (db, _temp_dir) = create_test_db().await;

        let user = create_test_user(db.sea_orm_connection(), "test@example.com").await;

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

        // Rating too low
        let result = UserSeriesRatingRepository::create(
            db.sea_orm_connection(),
            user.id,
            series.id,
            0,
            None,
        )
        .await;
        assert!(result.is_err());

        // Rating too high
        let result = UserSeriesRatingRepository::create(
            db.sea_orm_connection(),
            user.id,
            series.id,
            101,
            None,
        )
        .await;
        assert!(result.is_err());

        // Valid ratings at boundaries
        let rating = UserSeriesRatingRepository::create(
            db.sea_orm_connection(),
            user.id,
            series.id,
            1,
            None,
        )
        .await
        .unwrap();
        assert_eq!(rating.rating, 1);

        // Delete and try 100
        UserSeriesRatingRepository::delete(db.sea_orm_connection(), rating.id)
            .await
            .unwrap();

        let rating = UserSeriesRatingRepository::create(
            db.sea_orm_connection(),
            user.id,
            series.id,
            100,
            None,
        )
        .await
        .unwrap();
        assert_eq!(rating.rating, 100);
    }

    #[tokio::test]
    async fn test_delete_rating() {
        let (db, _temp_dir) = create_test_db().await;

        let user = create_test_user(db.sea_orm_connection(), "test@example.com").await;

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

        let rating = UserSeriesRatingRepository::create(
            db.sea_orm_connection(),
            user.id,
            series.id,
            70,
            None,
        )
        .await
        .unwrap();

        // Delete by ID
        let deleted = UserSeriesRatingRepository::delete(db.sea_orm_connection(), rating.id)
            .await
            .unwrap();
        assert!(deleted);

        let fetched = UserSeriesRatingRepository::get_by_id(db.sea_orm_connection(), rating.id)
            .await
            .unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_delete_by_user_and_series() {
        let (db, _temp_dir) = create_test_db().await;

        let user = create_test_user(db.sea_orm_connection(), "test@example.com").await;

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

        UserSeriesRatingRepository::create(db.sea_orm_connection(), user.id, series.id, 70, None)
            .await
            .unwrap();

        let deleted = UserSeriesRatingRepository::delete_by_user_and_series(
            db.sea_orm_connection(),
            user.id,
            series.id,
        )
        .await
        .unwrap();
        assert!(deleted);

        let fetched = UserSeriesRatingRepository::get_by_user_and_series(
            db.sea_orm_connection(),
            user.id,
            series.id,
        )
        .await
        .unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_get_all_for_user() {
        let (db, _temp_dir) = create_test_db().await;

        let user = create_test_user(db.sea_orm_connection(), "test@example.com").await;

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

        UserSeriesRatingRepository::create(db.sea_orm_connection(), user.id, series1.id, 80, None)
            .await
            .unwrap();

        UserSeriesRatingRepository::create(db.sea_orm_connection(), user.id, series2.id, 90, None)
            .await
            .unwrap();

        let ratings =
            UserSeriesRatingRepository::get_all_for_user(db.sea_orm_connection(), user.id)
                .await
                .unwrap();
        assert_eq!(ratings.len(), 2);
    }

    #[tokio::test]
    async fn test_calculate_average() {
        let (db, _temp_dir) = create_test_db().await;

        let user1 = create_test_user(db.sea_orm_connection(), "user1@example.com").await;
        let user2 = create_test_user(db.sea_orm_connection(), "user2@example.com").await;

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

        // No ratings yet
        let avg = UserSeriesRatingRepository::calculate_average_for_series(
            db.sea_orm_connection(),
            series.id,
        )
        .await
        .unwrap();
        assert!(avg.is_none());

        // Add ratings
        UserSeriesRatingRepository::create(db.sea_orm_connection(), user1.id, series.id, 80, None)
            .await
            .unwrap();

        UserSeriesRatingRepository::create(db.sea_orm_connection(), user2.id, series.id, 60, None)
            .await
            .unwrap();

        let avg = UserSeriesRatingRepository::calculate_average_for_series(
            db.sea_orm_connection(),
            series.id,
        )
        .await
        .unwrap();
        assert!(avg.is_some());
        assert!((avg.unwrap() - 70.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_count_for_series_and_user() {
        let (db, _temp_dir) = create_test_db().await;

        let user = create_test_user(db.sea_orm_connection(), "test@example.com").await;

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

        assert_eq!(
            UserSeriesRatingRepository::count_for_user(db.sea_orm_connection(), user.id)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            UserSeriesRatingRepository::count_for_series(db.sea_orm_connection(), series1.id)
                .await
                .unwrap(),
            0
        );

        UserSeriesRatingRepository::create(db.sea_orm_connection(), user.id, series1.id, 80, None)
            .await
            .unwrap();

        UserSeriesRatingRepository::create(db.sea_orm_connection(), user.id, series2.id, 90, None)
            .await
            .unwrap();

        assert_eq!(
            UserSeriesRatingRepository::count_for_user(db.sea_orm_connection(), user.id)
                .await
                .unwrap(),
            2
        );
        assert_eq!(
            UserSeriesRatingRepository::count_for_series(db.sea_orm_connection(), series1.id)
                .await
                .unwrap(),
            1
        );
    }
}
