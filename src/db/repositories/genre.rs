//! Repository for genres and series_genres table operations

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use uuid::Uuid;

use crate::db::entities::{genres, genres::Entity as Genres, series_genres};

/// Repository for genre operations
pub struct GenreRepository;

impl GenreRepository {
    /// Get a genre by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<genres::Model>> {
        let result = Genres::find_by_id(id).one(db).await?;
        Ok(result)
    }

    /// Get a genre by normalized name
    pub async fn get_by_name(db: &DatabaseConnection, name: &str) -> Result<Option<genres::Model>> {
        let normalized = name.to_lowercase().trim().to_string();
        let result = Genres::find()
            .filter(genres::Column::NormalizedName.eq(&normalized))
            .one(db)
            .await?;
        Ok(result)
    }

    /// List all genres sorted by name
    pub async fn list_all(db: &DatabaseConnection) -> Result<Vec<genres::Model>> {
        let results = Genres::find()
            .order_by_asc(genres::Column::Name)
            .all(db)
            .await?;
        Ok(results)
    }

    /// Create a new genre
    pub async fn create(db: &DatabaseConnection, name: &str) -> Result<genres::Model> {
        let normalized = name.to_lowercase().trim().to_string();
        let trimmed_name = name.trim().to_string();

        let active_model = genres::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(trimmed_name),
            normalized_name: Set(normalized),
            created_at: Set(Utc::now()),
        };

        let model = active_model.insert(db).await?;
        Ok(model)
    }

    /// Find or create a genre by name
    /// Returns the existing genre if found, otherwise creates a new one
    pub async fn find_or_create(db: &DatabaseConnection, name: &str) -> Result<genres::Model> {
        if let Some(existing) = Self::get_by_name(db, name).await? {
            return Ok(existing);
        }
        Self::create(db, name).await
    }

    /// Delete a genre by ID
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = Genres::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Get all genres for a series
    pub async fn get_genres_for_series(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Vec<genres::Model>> {
        use crate::db::entities::series_genres::Entity as SeriesGenres;

        let genre_ids: Vec<Uuid> = SeriesGenres::find()
            .filter(series_genres::Column::SeriesId.eq(series_id))
            .all(db)
            .await?
            .into_iter()
            .map(|sg| sg.genre_id)
            .collect();

        if genre_ids.is_empty() {
            return Ok(vec![]);
        }

        let genres = Genres::find()
            .filter(genres::Column::Id.is_in(genre_ids))
            .order_by_asc(genres::Column::Name)
            .all(db)
            .await?;

        Ok(genres)
    }

    /// Set genres for a series (replaces existing)
    /// Takes a list of genre names, finds or creates each, then links them
    pub async fn set_genres_for_series(
        db: &DatabaseConnection,
        series_id: Uuid,
        genre_names: Vec<String>,
    ) -> Result<Vec<genres::Model>> {
        use crate::db::entities::series_genres::Entity as SeriesGenres;

        // Remove existing genre links for this series
        SeriesGenres::delete_many()
            .filter(series_genres::Column::SeriesId.eq(series_id))
            .exec(db)
            .await?;

        if genre_names.is_empty() {
            return Ok(vec![]);
        }

        // Find or create each genre and link it
        let mut genres = Vec::new();
        for name in genre_names {
            let genre = Self::find_or_create(db, &name).await?;

            // Create the link
            let link = series_genres::ActiveModel {
                series_id: Set(series_id),
                genre_id: Set(genre.id),
            };
            link.insert(db).await?;

            genres.push(genre);
        }

        // Sort by name before returning
        genres.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(genres)
    }

    /// Add a single genre to a series
    pub async fn add_genre_to_series(
        db: &DatabaseConnection,
        series_id: Uuid,
        genre_name: &str,
    ) -> Result<genres::Model> {
        let genre = Self::find_or_create(db, genre_name).await?;

        // Check if already linked
        use crate::db::entities::series_genres::Entity as SeriesGenres;
        let existing = SeriesGenres::find()
            .filter(series_genres::Column::SeriesId.eq(series_id))
            .filter(series_genres::Column::GenreId.eq(genre.id))
            .one(db)
            .await?;

        if existing.is_none() {
            let link = series_genres::ActiveModel {
                series_id: Set(series_id),
                genre_id: Set(genre.id),
            };
            link.insert(db).await?;
        }

        Ok(genre)
    }

    /// Remove a genre from a series
    pub async fn remove_genre_from_series(
        db: &DatabaseConnection,
        series_id: Uuid,
        genre_id: Uuid,
    ) -> Result<bool> {
        use crate::db::entities::series_genres::Entity as SeriesGenres;

        let result = SeriesGenres::delete_many()
            .filter(series_genres::Column::SeriesId.eq(series_id))
            .filter(series_genres::Column::GenreId.eq(genre_id))
            .exec(db)
            .await?;

        Ok(result.rows_affected > 0)
    }

    /// Count series using a genre
    pub async fn count_series_with_genre(db: &DatabaseConnection, genre_id: Uuid) -> Result<u64> {
        use crate::db::entities::series_genres::Entity as SeriesGenres;

        let count = SeriesGenres::find()
            .filter(series_genres::Column::GenreId.eq(genre_id))
            .count(db)
            .await?;

        Ok(count)
    }

    /// Delete all unused genres (genres with no series linked)
    /// Returns the names of deleted genres
    pub async fn delete_unused(db: &DatabaseConnection) -> Result<Vec<String>> {
        use crate::db::entities::series_genres::Entity as SeriesGenres;

        // Get all genres
        let all_genres = Self::list_all(db).await?;
        let mut deleted_names = Vec::new();

        for genre in all_genres {
            // Check if genre has any series
            let count = SeriesGenres::find()
                .filter(series_genres::Column::GenreId.eq(genre.id))
                .count(db)
                .await?;

            if count == 0 {
                // Delete the unused genre
                Genres::delete_by_id(genre.id).exec(db).await?;
                deleted_names.push(genre.name);
            }
        }

        Ok(deleted_names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::{LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;
    use crate::db::ScanningStrategy;

    #[tokio::test]
    async fn test_create_and_get_genre() {
        let (db, _temp_dir) = create_test_db().await;

        let genre = GenreRepository::create(db.sea_orm_connection(), "Action")
            .await
            .unwrap();

        assert_eq!(genre.name, "Action");
        assert_eq!(genre.normalized_name, "action");

        let fetched = GenreRepository::get_by_id(db.sea_orm_connection(), genre.id)
            .await
            .unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "Action");
    }

    #[tokio::test]
    async fn test_find_or_create_genre() {
        let (db, _temp_dir) = create_test_db().await;

        // First call creates
        let genre1 = GenreRepository::find_or_create(db.sea_orm_connection(), "Comedy")
            .await
            .unwrap();
        assert_eq!(genre1.name, "Comedy");

        // Second call finds existing (case insensitive)
        let genre2 = GenreRepository::find_or_create(db.sea_orm_connection(), "COMEDY")
            .await
            .unwrap();
        assert_eq!(genre1.id, genre2.id);

        // Third call with different name creates new
        let genre3 = GenreRepository::find_or_create(db.sea_orm_connection(), "Drama")
            .await
            .unwrap();
        assert_ne!(genre1.id, genre3.id);
    }

    #[tokio::test]
    async fn test_list_all_genres() {
        let (db, _temp_dir) = create_test_db().await;

        GenreRepository::create(db.sea_orm_connection(), "Zulu")
            .await
            .unwrap();
        GenreRepository::create(db.sea_orm_connection(), "Alpha")
            .await
            .unwrap();
        GenreRepository::create(db.sea_orm_connection(), "Beta")
            .await
            .unwrap();

        let genres = GenreRepository::list_all(db.sea_orm_connection())
            .await
            .unwrap();

        assert_eq!(genres.len(), 3);
        // Should be sorted by name
        assert_eq!(genres[0].name, "Alpha");
        assert_eq!(genres[1].name, "Beta");
        assert_eq!(genres[2].name, "Zulu");
    }

    #[tokio::test]
    async fn test_set_genres_for_series() {
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

        // Set initial genres
        let genres = GenreRepository::set_genres_for_series(
            db.sea_orm_connection(),
            series.id,
            vec!["Action".to_string(), "Comedy".to_string()],
        )
        .await
        .unwrap();

        assert_eq!(genres.len(), 2);

        // Verify they're linked
        let fetched = GenreRepository::get_genres_for_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 2);

        // Replace with different genres
        let new_genres = GenreRepository::set_genres_for_series(
            db.sea_orm_connection(),
            series.id,
            vec!["Drama".to_string()],
        )
        .await
        .unwrap();

        assert_eq!(new_genres.len(), 1);
        assert_eq!(new_genres[0].name, "Drama");

        // Verify old genres are unlinked
        let fetched = GenreRepository::get_genres_for_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].name, "Drama");
    }

    #[tokio::test]
    async fn test_add_and_remove_genre_from_series() {
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

        // Add a genre
        let genre =
            GenreRepository::add_genre_to_series(db.sea_orm_connection(), series.id, "Horror")
                .await
                .unwrap();

        let fetched = GenreRepository::get_genres_for_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 1);

        // Adding same genre again should not duplicate
        GenreRepository::add_genre_to_series(db.sea_orm_connection(), series.id, "Horror")
            .await
            .unwrap();

        let fetched = GenreRepository::get_genres_for_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 1);

        // Remove the genre
        let removed =
            GenreRepository::remove_genre_from_series(db.sea_orm_connection(), series.id, genre.id)
                .await
                .unwrap();
        assert!(removed);

        let fetched = GenreRepository::get_genres_for_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 0);
    }

    #[tokio::test]
    async fn test_count_series_with_genre() {
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

        let genre = GenreRepository::create(db.sea_orm_connection(), "SciFi")
            .await
            .unwrap();

        // Initially no series have this genre
        let count = GenreRepository::count_series_with_genre(db.sea_orm_connection(), genre.id)
            .await
            .unwrap();
        assert_eq!(count, 0);

        // Add genre to series1
        GenreRepository::add_genre_to_series(db.sea_orm_connection(), series1.id, "SciFi")
            .await
            .unwrap();

        let count = GenreRepository::count_series_with_genre(db.sea_orm_connection(), genre.id)
            .await
            .unwrap();
        assert_eq!(count, 1);

        // Add genre to series2
        GenreRepository::add_genre_to_series(db.sea_orm_connection(), series2.id, "SciFi")
            .await
            .unwrap();

        let count = GenreRepository::count_series_with_genre(db.sea_orm_connection(), genre.id)
            .await
            .unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_delete_genre() {
        let (db, _temp_dir) = create_test_db().await;

        let genre = GenreRepository::create(db.sea_orm_connection(), "ToDelete")
            .await
            .unwrap();

        let deleted = GenreRepository::delete(db.sea_orm_connection(), genre.id)
            .await
            .unwrap();
        assert!(deleted);

        let fetched = GenreRepository::get_by_id(db.sea_orm_connection(), genre.id)
            .await
            .unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_genre_name_trimming() {
        let (db, _temp_dir) = create_test_db().await;

        let genre = GenreRepository::create(db.sea_orm_connection(), "  Spaced  ")
            .await
            .unwrap();

        assert_eq!(genre.name, "Spaced");
        assert_eq!(genre.normalized_name, "spaced");

        // Should find by original name with spaces
        let found = GenreRepository::get_by_name(db.sea_orm_connection(), "  SPACED  ")
            .await
            .unwrap();
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn test_delete_unused_genres() {
        let (db, _temp_dir) = create_test_db().await;

        // Create a library and series for testing
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

        // Create some genres - one used, two unused
        let used_genre = GenreRepository::create(db.sea_orm_connection(), "UsedGenre")
            .await
            .unwrap();
        GenreRepository::create(db.sea_orm_connection(), "UnusedGenre1")
            .await
            .unwrap();
        GenreRepository::create(db.sea_orm_connection(), "UnusedGenre2")
            .await
            .unwrap();

        // Link one genre to a series
        GenreRepository::add_genre_to_series(db.sea_orm_connection(), series.id, "UsedGenre")
            .await
            .unwrap();

        // Verify we have 3 genres
        let all_genres = GenreRepository::list_all(db.sea_orm_connection())
            .await
            .unwrap();
        assert_eq!(all_genres.len(), 3);

        // Delete unused genres
        let deleted_names = GenreRepository::delete_unused(db.sea_orm_connection())
            .await
            .unwrap();

        // Should have deleted 2 unused genres
        assert_eq!(deleted_names.len(), 2);
        assert!(deleted_names.contains(&"UnusedGenre1".to_string()));
        assert!(deleted_names.contains(&"UnusedGenre2".to_string()));

        // Verify only 1 genre remains
        let remaining_genres = GenreRepository::list_all(db.sea_orm_connection())
            .await
            .unwrap();
        assert_eq!(remaining_genres.len(), 1);
        assert_eq!(remaining_genres[0].id, used_genre.id);
    }

    #[tokio::test]
    async fn test_delete_unused_genres_empty() {
        let (db, _temp_dir) = create_test_db().await;

        // Delete unused when no genres exist
        let deleted_names = GenreRepository::delete_unused(db.sea_orm_connection())
            .await
            .unwrap();

        assert!(deleted_names.is_empty());
    }
}
