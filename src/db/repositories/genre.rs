//! Repository for genres, series_genres, and book_genres table operations
//!
//! TODO: Remove allow(dead_code) when genre features are fully integrated

#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use uuid::Uuid;

use crate::db::entities::{
    book_genres, book_genres::Entity as BookGenres, genres, genres::Entity as Genres, series_genres,
};

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

    /// Get all series IDs that have a specific genre (by normalized name)
    pub async fn get_series_ids_by_genre_name(
        db: &DatabaseConnection,
        genre_name: &str,
    ) -> Result<Vec<Uuid>> {
        use crate::db::entities::series_genres::Entity as SeriesGenres;

        let normalized = genre_name.to_lowercase().trim().to_string();

        // First find the genre
        let genre = Genres::find()
            .filter(genres::Column::NormalizedName.eq(&normalized))
            .one(db)
            .await?;

        match genre {
            Some(g) => {
                let series_ids: Vec<Uuid> = SeriesGenres::find()
                    .filter(series_genres::Column::GenreId.eq(g.id))
                    .all(db)
                    .await?
                    .into_iter()
                    .map(|sg| sg.series_id)
                    .collect();

                Ok(series_ids)
            }
            None => Ok(vec![]),
        }
    }

    /// Get all series IDs that have ALL of the specified genres (AND logic)
    pub async fn get_series_ids_by_genre_names(
        db: &DatabaseConnection,
        genre_names: &[String],
    ) -> Result<Vec<Uuid>> {
        if genre_names.is_empty() {
            return Ok(vec![]);
        }

        // Get series IDs for the first genre
        let mut result_ids = Self::get_series_ids_by_genre_name(db, &genre_names[0]).await?;

        // Intersect with series IDs for remaining genres
        for name in &genre_names[1..] {
            let ids = Self::get_series_ids_by_genre_name(db, name).await?;
            result_ids.retain(|id| ids.contains(id));

            // Early exit if no matches
            if result_ids.is_empty() {
                break;
            }
        }

        Ok(result_ids)
    }

    /// Get all series IDs that have a specific genre (alias for get_series_ids_by_genre_name)
    pub async fn get_series_with_genre(
        db: &DatabaseConnection,
        genre_name: &str,
    ) -> Result<Vec<Uuid>> {
        Self::get_series_ids_by_genre_name(db, genre_name).await
    }

    /// Get all series IDs that have any genre containing the given substring (case-insensitive)
    pub async fn get_series_with_genre_containing(
        db: &DatabaseConnection,
        substring: &str,
    ) -> Result<Vec<Uuid>> {
        use crate::db::entities::series_genres::Entity as SeriesGenres;

        let normalized = substring.to_lowercase();

        // Find all genres containing the substring
        let matching_genres: Vec<genres::Model> = Genres::find()
            .filter(genres::Column::NormalizedName.contains(&normalized))
            .all(db)
            .await?;

        if matching_genres.is_empty() {
            return Ok(vec![]);
        }

        let genre_ids: Vec<Uuid> = matching_genres.iter().map(|g| g.id).collect();

        // Get all series with any of these genres
        let series_ids: Vec<Uuid> = SeriesGenres::find()
            .filter(series_genres::Column::GenreId.is_in(genre_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|sg| sg.series_id)
            .collect();

        // Deduplicate
        let unique: std::collections::HashSet<Uuid> = series_ids.into_iter().collect();
        Ok(unique.into_iter().collect())
    }

    /// Get all series IDs that have any genre starting with the given prefix (case-insensitive)
    pub async fn get_series_with_genre_starting_with(
        db: &DatabaseConnection,
        prefix: &str,
    ) -> Result<Vec<Uuid>> {
        use crate::db::entities::series_genres::Entity as SeriesGenres;

        let normalized = prefix.to_lowercase();

        let matching_genres: Vec<genres::Model> = Genres::find()
            .filter(genres::Column::NormalizedName.starts_with(&normalized))
            .all(db)
            .await?;

        if matching_genres.is_empty() {
            return Ok(vec![]);
        }

        let genre_ids: Vec<Uuid> = matching_genres.iter().map(|g| g.id).collect();

        let series_ids: Vec<Uuid> = SeriesGenres::find()
            .filter(series_genres::Column::GenreId.is_in(genre_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|sg| sg.series_id)
            .collect();

        let unique: std::collections::HashSet<Uuid> = series_ids.into_iter().collect();
        Ok(unique.into_iter().collect())
    }

    /// Get all series IDs that have any genre ending with the given suffix (case-insensitive)
    pub async fn get_series_with_genre_ending_with(
        db: &DatabaseConnection,
        suffix: &str,
    ) -> Result<Vec<Uuid>> {
        use crate::db::entities::series_genres::Entity as SeriesGenres;

        let normalized = suffix.to_lowercase();

        let matching_genres: Vec<genres::Model> = Genres::find()
            .filter(genres::Column::NormalizedName.ends_with(&normalized))
            .all(db)
            .await?;

        if matching_genres.is_empty() {
            return Ok(vec![]);
        }

        let genre_ids: Vec<Uuid> = matching_genres.iter().map(|g| g.id).collect();

        let series_ids: Vec<Uuid> = SeriesGenres::find()
            .filter(series_genres::Column::GenreId.is_in(genre_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|sg| sg.series_id)
            .collect();

        let unique: std::collections::HashSet<Uuid> = series_ids.into_iter().collect();
        Ok(unique.into_iter().collect())
    }

    /// Get all series IDs that have at least one genre
    pub async fn get_all_series_with_genres(db: &DatabaseConnection) -> Result<Vec<Uuid>> {
        use crate::db::entities::series_genres::Entity as SeriesGenres;

        let series_ids: Vec<Uuid> = SeriesGenres::find()
            .all(db)
            .await?
            .into_iter()
            .map(|sg| sg.series_id)
            .collect();

        // Deduplicate
        let unique: std::collections::HashSet<Uuid> = series_ids.into_iter().collect();
        Ok(unique.into_iter().collect())
    }

    /// Get genres for multiple series by their IDs
    ///
    /// Returns a HashMap keyed by series_id for efficient lookups
    pub async fn get_genres_for_series_ids(
        db: &DatabaseConnection,
        series_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<genres::Model>>> {
        use crate::db::entities::series_genres::Entity as SeriesGenres;

        if series_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        // Get all series_genres mappings for the given series
        let series_genre_links: Vec<series_genres::Model> = SeriesGenres::find()
            .filter(series_genres::Column::SeriesId.is_in(series_ids.to_vec()))
            .all(db)
            .await?;

        if series_genre_links.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        // Collect unique genre IDs
        let genre_ids: Vec<Uuid> = series_genre_links
            .iter()
            .map(|sg| sg.genre_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Fetch all genres at once
        let all_genres: Vec<genres::Model> = Genres::find()
            .filter(genres::Column::Id.is_in(genre_ids))
            .all(db)
            .await?;

        // Create genre lookup map
        let genre_map: std::collections::HashMap<Uuid, genres::Model> =
            all_genres.into_iter().map(|g| (g.id, g)).collect();

        // Build result map
        let mut result: std::collections::HashMap<Uuid, Vec<genres::Model>> =
            std::collections::HashMap::new();

        for link in series_genre_links {
            if let Some(genre) = genre_map.get(&link.genre_id) {
                result
                    .entry(link.series_id)
                    .or_default()
                    .push(genre.clone());
            }
        }

        // Sort genres by name within each series
        for genres in result.values_mut() {
            genres.sort_by(|a, b| a.name.cmp(&b.name));
        }

        Ok(result)
    }

    /// Get all genres for a book
    pub async fn get_genres_for_book(
        db: &DatabaseConnection,
        book_id: Uuid,
    ) -> Result<Vec<genres::Model>> {
        let genre_ids: Vec<Uuid> = BookGenres::find()
            .filter(book_genres::Column::BookId.eq(book_id))
            .all(db)
            .await?
            .into_iter()
            .map(|bg| bg.genre_id)
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

    /// Set genres for a book (replaces existing)
    /// Takes a list of genre names, finds or creates each, then links them
    pub async fn set_genres_for_book(
        db: &DatabaseConnection,
        book_id: Uuid,
        genre_names: Vec<String>,
    ) -> Result<Vec<genres::Model>> {
        // Remove existing genre links for this book
        BookGenres::delete_many()
            .filter(book_genres::Column::BookId.eq(book_id))
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
            let link = book_genres::ActiveModel {
                book_id: Set(book_id),
                genre_id: Set(genre.id),
            };
            link.insert(db).await?;

            genres.push(genre);
        }

        // Sort by name before returning
        genres.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(genres)
    }

    /// Add a single genre to a book
    pub async fn add_genre_to_book(
        db: &DatabaseConnection,
        book_id: Uuid,
        genre_name: &str,
    ) -> Result<genres::Model> {
        let genre = Self::find_or_create(db, genre_name).await?;

        // Check if already linked
        let existing = BookGenres::find()
            .filter(book_genres::Column::BookId.eq(book_id))
            .filter(book_genres::Column::GenreId.eq(genre.id))
            .one(db)
            .await?;

        if existing.is_none() {
            let link = book_genres::ActiveModel {
                book_id: Set(book_id),
                genre_id: Set(genre.id),
            };
            link.insert(db).await?;
        }

        Ok(genre)
    }

    /// Remove a genre from a book
    pub async fn remove_genre_from_book(
        db: &DatabaseConnection,
        book_id: Uuid,
        genre_id: Uuid,
    ) -> Result<bool> {
        let result = BookGenres::delete_many()
            .filter(book_genres::Column::BookId.eq(book_id))
            .filter(book_genres::Column::GenreId.eq(genre_id))
            .exec(db)
            .await?;

        Ok(result.rows_affected > 0)
    }

    /// Get genres for multiple books by their IDs
    ///
    /// Returns a HashMap keyed by book_id for efficient lookups
    pub async fn get_genres_for_book_ids(
        db: &DatabaseConnection,
        book_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<genres::Model>>> {
        if book_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        // Get all book_genres mappings for the given books
        let book_genre_links: Vec<book_genres::Model> = BookGenres::find()
            .filter(book_genres::Column::BookId.is_in(book_ids.to_vec()))
            .all(db)
            .await?;

        if book_genre_links.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        // Collect unique genre IDs
        let genre_ids: Vec<Uuid> = book_genre_links
            .iter()
            .map(|bg| bg.genre_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Fetch all genres at once
        let all_genres: Vec<genres::Model> = Genres::find()
            .filter(genres::Column::Id.is_in(genre_ids))
            .all(db)
            .await?;

        // Create genre lookup map
        let genre_map: std::collections::HashMap<Uuid, genres::Model> =
            all_genres.into_iter().map(|g| (g.id, g)).collect();

        // Build result map
        let mut result: std::collections::HashMap<Uuid, Vec<genres::Model>> =
            std::collections::HashMap::new();

        for link in book_genre_links {
            if let Some(genre) = genre_map.get(&link.genre_id) {
                result.entry(link.book_id).or_default().push(genre.clone());
            }
        }

        // Sort genres by name within each book
        for genres in result.values_mut() {
            genres.sort_by(|a, b| a.name.cmp(&b.name));
        }

        Ok(result)
    }

    /// Delete all unused genres (genres with no series or books linked)
    /// Returns the names of deleted genres
    pub async fn delete_unused(db: &DatabaseConnection) -> Result<Vec<String>> {
        use crate::db::entities::series_genres::Entity as SeriesGenres;

        // Get all genres
        let all_genres = Self::list_all(db).await?;
        let mut deleted_names = Vec::new();

        for genre in all_genres {
            // Check if genre has any series
            let series_count = SeriesGenres::find()
                .filter(series_genres::Column::GenreId.eq(genre.id))
                .count(db)
                .await?;

            // Check if genre has any books
            let book_count = BookGenres::find()
                .filter(book_genres::Column::GenreId.eq(genre.id))
                .count(db)
                .await?;

            if series_count == 0 && book_count == 0 {
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
    use crate::db::ScanningStrategy;
    use crate::db::repositories::{LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;

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

    /// Helper to create a test book for genre tests
    async fn create_test_book_for_genre(
        db: &crate::db::Database,
        series_id: Uuid,
        library_id: Uuid,
    ) -> crate::db::entities::books::Model {
        use crate::db::entities::books;
        use crate::db::repositories::BookRepository;
        use chrono::Utc;

        let book = books::Model {
            id: Uuid::new_v4(),
            series_id,
            library_id,
            file_path: "/test/book.cbz".to_string(),
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
        };

        BookRepository::create(db.sea_orm_connection(), &book, None)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_set_genres_for_book() {
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

        let book = create_test_book_for_genre(&db, series.id, library.id).await;

        // Set initial genres
        let genres = GenreRepository::set_genres_for_book(
            db.sea_orm_connection(),
            book.id,
            vec!["Action".to_string(), "Comedy".to_string()],
        )
        .await
        .unwrap();

        assert_eq!(genres.len(), 2);

        // Verify they're linked
        let fetched = GenreRepository::get_genres_for_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 2);

        // Replace with different genres
        let new_genres = GenreRepository::set_genres_for_book(
            db.sea_orm_connection(),
            book.id,
            vec!["Drama".to_string()],
        )
        .await
        .unwrap();

        assert_eq!(new_genres.len(), 1);
        assert_eq!(new_genres[0].name, "Drama");

        // Verify old genres are unlinked
        let fetched = GenreRepository::get_genres_for_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].name, "Drama");
    }

    #[tokio::test]
    async fn test_add_and_remove_genre_from_book() {
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

        let book = create_test_book_for_genre(&db, series.id, library.id).await;

        // Add a genre
        let genre = GenreRepository::add_genre_to_book(db.sea_orm_connection(), book.id, "Horror")
            .await
            .unwrap();

        let fetched = GenreRepository::get_genres_for_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 1);

        // Adding same genre again should not duplicate
        GenreRepository::add_genre_to_book(db.sea_orm_connection(), book.id, "Horror")
            .await
            .unwrap();

        let fetched = GenreRepository::get_genres_for_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 1);

        // Remove the genre
        let removed =
            GenreRepository::remove_genre_from_book(db.sea_orm_connection(), book.id, genre.id)
                .await
                .unwrap();
        assert!(removed);

        let fetched = GenreRepository::get_genres_for_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 0);
    }

    #[tokio::test]
    async fn test_delete_unused_genres_with_book_links() {
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

        let book = create_test_book_for_genre(&db, series.id, library.id).await;

        // Create genres: one linked to book only, one linked to series only, one unused
        let book_genre = GenreRepository::create(db.sea_orm_connection(), "BookOnlyGenre")
            .await
            .unwrap();
        GenreRepository::create(db.sea_orm_connection(), "UnusedGenre")
            .await
            .unwrap();

        // Link one genre to a book (not a series)
        GenreRepository::add_genre_to_book(db.sea_orm_connection(), book.id, "BookOnlyGenre")
            .await
            .unwrap();

        // Verify we have 2 genres
        let all_genres = GenreRepository::list_all(db.sea_orm_connection())
            .await
            .unwrap();
        assert_eq!(all_genres.len(), 2);

        // Delete unused genres — should only delete the truly unused one
        let deleted_names = GenreRepository::delete_unused(db.sea_orm_connection())
            .await
            .unwrap();

        assert_eq!(deleted_names.len(), 1);
        assert!(deleted_names.contains(&"UnusedGenre".to_string()));

        // Genre linked to book should still exist
        let remaining = GenreRepository::list_all(db.sea_orm_connection())
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, book_genre.id);
    }
}
