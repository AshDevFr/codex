use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, RelationTrait, Set,
};
use uuid::Uuid;

use crate::db::entities::{prelude::*, series};
use crate::events::{EntityChangeEvent, EntityEvent, EventBroadcaster};
use std::sync::Arc;

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
        event_broadcaster: Option<&Arc<EventBroadcaster>>,
    ) -> Result<series::Model> {
        Self::create_with_fingerprint(db, library_id, name, None, None, event_broadcaster).await
    }

    /// Create a new series with optional fingerprint
    pub async fn create_with_fingerprint(
        db: &DatabaseConnection,
        library_id: Uuid,
        name: &str,
        fingerprint: Option<String>,
        path: Option<String>,
        event_broadcaster: Option<&Arc<EventBroadcaster>>,
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
            path: Set(path),
            reading_direction: Set(None),
            custom_cover_path: Set(None),
            selected_cover_source: Set(None),
            metadata_populated_from_book: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let created_series = series.insert(db).await.context("Failed to create series")?;

        // Emit SeriesCreated event if broadcaster is available
        if let Some(broadcaster) = event_broadcaster {
            let event = EntityChangeEvent::new(
                EntityEvent::SeriesCreated {
                    series_id: created_series.id,
                    library_id,
                },
                None, // System-triggered, no user_id
            );
            let _ = broadcaster.emit(event);
        }

        Ok(created_series)
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

    /// Count series in a library
    pub async fn count_by_library(db: &DatabaseConnection, library_id: Uuid) -> Result<i64> {
        use sea_orm::PaginatorTrait;

        let count = Series::find()
            .filter(series::Column::LibraryId.eq(library_id))
            .paginate(db, 1)
            .num_items()
            .await
            .context("Failed to count series")?;

        Ok(count as i64)
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

    /// Get series with in-progress books (series that have at least one book with reading progress that is not completed)
    pub async fn list_in_progress(
        db: &DatabaseConnection,
        user_id: Uuid,
        library_id: Option<Uuid>,
    ) -> Result<Vec<series::Model>> {
        use crate::db::entities::{books, read_progress};
        use sea_orm::JoinType;

        let mut query = Series::find()
            .join(JoinType::InnerJoin, series::Relation::Books.def())
            .join(JoinType::InnerJoin, books::Relation::ReadProgress.def())
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::Completed.eq(false)); // Only in-progress books

        // Filter by library if specified
        if let Some(lib_id) = library_id {
            query = query.filter(series::Column::LibraryId.eq(lib_id));
        }

        // Group by series to avoid duplicates
        query
            .group_by(series::Column::Id)
            .order_by_asc(series::Column::SortName)
            .order_by_asc(series::Column::Name)
            .all(db)
            .await
            .context("Failed to list in-progress series")
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
    pub async fn update(
        db: &DatabaseConnection,
        series_model: &series::Model,
        event_broadcaster: Option<&Arc<EventBroadcaster>>,
    ) -> Result<()> {
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
            path: Set(series_model.path.clone()),
            reading_direction: Set(series_model.reading_direction.clone()),
            custom_cover_path: Set(series_model.custom_cover_path.clone()),
            selected_cover_source: Set(series_model.selected_cover_source.clone()),
            metadata_populated_from_book: Set(series_model.metadata_populated_from_book),
            created_at: Set(series_model.created_at),
            updated_at: Set(Utc::now()),
        };

        active.update(db).await.context("Failed to update series")?;

        // Emit SeriesUpdated event if broadcaster is available
        if let Some(broadcaster) = event_broadcaster {
            let event = EntityChangeEvent::new(
                EntityEvent::SeriesUpdated {
                    series_id: series_model.id,
                    library_id: series_model.library_id,
                    fields: None, // Could track specific fields if needed
                },
                None, // System-triggered, no user_id
            );
            let _ = broadcaster.emit(event);
        }

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

    /// Update series custom cover path
    pub async fn update_custom_cover(
        db: &DatabaseConnection,
        id: Uuid,
        cover_path: Option<String>,
    ) -> Result<()> {
        let series = Series::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Series not found"))?;

        let mut active: series::ActiveModel = series.into();
        active.custom_cover_path = Set(cover_path);
        active.updated_at = Set(Utc::now());

        active
            .update(db)
            .await
            .context("Failed to update custom cover path")?;

        Ok(())
    }

    /// Update which cover source is selected (default, custom, etc.)
    pub async fn update_selected_cover_source(
        db: &DatabaseConnection,
        id: Uuid,
        source: Option<String>,
    ) -> Result<()> {
        let series = Series::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Series not found"))?;

        let mut active: series::ActiveModel = series.into();
        active.selected_cover_source = Set(source);
        active.updated_at = Set(Utc::now());

        active
            .update(db)
            .await
            .context("Failed to update selected cover source")?;

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

    /// Find and delete all series in a library that have no books
    pub async fn purge_empty_series_in_library(
        db: &DatabaseConnection,
        library_id: Uuid,
        event_broadcaster: Option<&Arc<crate::events::EventBroadcaster>>,
    ) -> Result<u64> {
        use crate::db::entities::{books, prelude::*};

        // Find all series in the library
        let all_series = Series::find()
            .filter(series::Column::LibraryId.eq(library_id))
            .all(db)
            .await
            .context("Failed to find series in library")?;

        let mut deleted_count = 0u64;

        // Check each series and delete if empty
        for series_model in all_series {
            let book_count = books::Entity::find()
                .filter(books::Column::SeriesId.eq(series_model.id))
                .count(db)
                .await
                .context("Failed to count books in series")?;

            if book_count == 0 {
                let series_id = series_model.id;

                Series::delete_by_id(series_id)
                    .exec(db)
                    .await
                    .context(format!("Failed to delete empty series {}", series_id))?;
                deleted_count += 1;

                // Emit SeriesDeleted event
                if let Some(broadcaster) = event_broadcaster {
                    use crate::events::{EntityChangeEvent, EntityEvent};
                    use tracing::warn;

                    let event = EntityChangeEvent {
                        event: EntityEvent::SeriesDeleted {
                            library_id,
                            series_id,
                        },
                        user_id: None,
                        timestamp: chrono::Utc::now(),
                    };

                    if let Err(e) = broadcaster.emit(event) {
                        warn!(
                            "Failed to emit SeriesDeleted event for series {}: {:?}",
                            series_id, e
                        );
                    }
                }
            }
        }

        Ok(deleted_count)
    }

    /// Check if a series has any books and delete it if empty
    pub async fn purge_if_empty(
        db: &DatabaseConnection,
        series_id: Uuid,
        event_broadcaster: Option<&Arc<crate::events::EventBroadcaster>>,
    ) -> Result<bool> {
        use crate::db::entities::books;

        // First get series info for library_id before deletion
        let series = Self::get_by_id(db, series_id)
            .await?
            .context("Series not found")?;
        let library_id = series.library_id;

        // Check if series has any books
        let book_count = books::Entity::find()
            .filter(books::Column::SeriesId.eq(series_id))
            .count(db)
            .await
            .context("Failed to count books in series")?;

        if book_count == 0 {
            // Series is empty, delete it
            Series::delete_by_id(series_id)
                .exec(db)
                .await
                .context("Failed to delete empty series")?;

            // Emit SeriesDeleted event
            if let Some(broadcaster) = event_broadcaster {
                use crate::events::{EntityChangeEvent, EntityEvent};
                use tracing::warn;

                let event = EntityChangeEvent {
                    event: EntityEvent::SeriesDeleted {
                        library_id,
                        series_id,
                    },
                    user_id: None,
                    timestamp: chrono::Utc::now(),
                };

                if let Err(e) = broadcaster.emit(event) {
                    warn!(
                        "Failed to emit SeriesDeleted event for series {}: {:?}",
                        series_id, e
                    );
                }
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::books;
    use crate::db::repositories::{BookRepository, LibraryRepository};
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

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
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

        let created =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
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

        SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 1", None)
            .await
            .unwrap();
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 2", None)
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

        SeriesRepository::create(db.sea_orm_connection(), library.id, "One Piece", None)
            .await
            .unwrap();
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Naruto", None)
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Original Name", None)
                .await
                .unwrap();

        series.name = "Updated Name".to_string();
        series.normalized_name = SeriesRepository::normalize_name(&series.name);
        series.summary = Some("Updated summary".to_string());

        SeriesRepository::update(db.sea_orm_connection(), &series, None)
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

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
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

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "To Delete", None)
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

    #[tokio::test]
    async fn test_series_reading_direction_defaults_to_none() {
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

        // reading_direction should default to None (inherits from library)
        assert_eq!(series.reading_direction, None);
    }

    #[tokio::test]
    async fn test_series_update_reading_direction() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Manga Series", None)
                .await
                .unwrap();

        // Override reading direction for this specific series
        series.reading_direction = Some("RIGHT_TO_LEFT".to_string());
        SeriesRepository::update(db.sea_orm_connection(), &series, None)
            .await
            .unwrap();

        let retrieved = SeriesRepository::get_by_id(db.sea_orm_connection(), series.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            retrieved.reading_direction,
            Some("RIGHT_TO_LEFT".to_string())
        );
    }

    #[tokio::test]
    async fn test_series_clear_reading_direction() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        // Set a reading direction
        series.reading_direction = Some("TOP_TO_BOTTOM".to_string());
        SeriesRepository::update(db.sea_orm_connection(), &series, None)
            .await
            .unwrap();

        // Clear it to revert to library default
        series.reading_direction = None;
        SeriesRepository::update(db.sea_orm_connection(), &series, None)
            .await
            .unwrap();

        let retrieved = SeriesRepository::get_by_id(db.sea_orm_connection(), series.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.reading_direction, None);
    }

    #[tokio::test]
    async fn test_series_reading_direction_inheritance_concept() {
        let (db, _temp_dir) = create_test_db().await;

        // Create library with RIGHT_TO_LEFT default (manga library)
        let mut library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Manga Library",
            "/manga/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();
        library.default_reading_direction = "RIGHT_TO_LEFT".to_string();
        LibraryRepository::update(db.sea_orm_connection(), &library)
            .await
            .unwrap();

        // Create series without reading direction (should inherit library default)
        let series1 =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Manga 1", None)
                .await
                .unwrap();

        // Create series with explicit override
        let mut series2 =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Webtoon", None)
                .await
                .unwrap();
        series2.reading_direction = Some("TOP_TO_BOTTOM".to_string());
        SeriesRepository::update(db.sea_orm_connection(), &series2, None)
            .await
            .unwrap();

        // Verify inheritance concept
        let retrieved_library = LibraryRepository::get_by_id(db.sea_orm_connection(), library.id)
            .await
            .unwrap()
            .unwrap();
        let retrieved_series1 = SeriesRepository::get_by_id(db.sea_orm_connection(), series1.id)
            .await
            .unwrap()
            .unwrap();
        let retrieved_series2 = SeriesRepository::get_by_id(db.sea_orm_connection(), series2.id)
            .await
            .unwrap()
            .unwrap();

        // Library has RIGHT_TO_LEFT
        assert_eq!(retrieved_library.default_reading_direction, "RIGHT_TO_LEFT");

        // Series1 has None, meaning it inherits library's RIGHT_TO_LEFT
        assert_eq!(retrieved_series1.reading_direction, None);

        // Series2 has explicit override to TOP_TO_BOTTOM
        assert_eq!(
            retrieved_series2.reading_direction,
            Some("TOP_TO_BOTTOM".to_string())
        );
    }

    #[tokio::test]
    async fn test_update_custom_cover() {
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

        // Initially no custom cover
        assert_eq!(series.custom_cover_path, None);

        // Set custom cover path
        SeriesRepository::update_custom_cover(
            db.sea_orm_connection(),
            series.id,
            Some("data/covers/test.jpg".to_string()),
        )
        .await
        .unwrap();

        let retrieved = SeriesRepository::get_by_id(db.sea_orm_connection(), series.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            retrieved.custom_cover_path,
            Some("data/covers/test.jpg".to_string())
        );

        // Clear custom cover
        SeriesRepository::update_custom_cover(db.sea_orm_connection(), series.id, None)
            .await
            .unwrap();

        let retrieved = SeriesRepository::get_by_id(db.sea_orm_connection(), series.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.custom_cover_path, None);
    }

    #[tokio::test]
    async fn test_update_selected_cover_source() {
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

        // Initially no selected cover source (defaults to first book cover)
        assert_eq!(series.selected_cover_source, None);

        // Set to custom
        SeriesRepository::update_selected_cover_source(
            db.sea_orm_connection(),
            series.id,
            Some("custom".to_string()),
        )
        .await
        .unwrap();

        let retrieved = SeriesRepository::get_by_id(db.sea_orm_connection(), series.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.selected_cover_source, Some("custom".to_string()));

        // Set to default
        SeriesRepository::update_selected_cover_source(
            db.sea_orm_connection(),
            series.id,
            Some("default".to_string()),
        )
        .await
        .unwrap();

        let retrieved = SeriesRepository::get_by_id(db.sea_orm_connection(), series.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.selected_cover_source, Some("default".to_string()));

        // Clear to use default behavior
        SeriesRepository::update_selected_cover_source(db.sea_orm_connection(), series.id, None)
            .await
            .unwrap();

        let retrieved = SeriesRepository::get_by_id(db.sea_orm_connection(), series.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.selected_cover_source, None);
    }

    #[tokio::test]
    async fn test_custom_cover_workflow() {
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

        // Simulate uploading a custom cover
        let cover_path = format!("data/covers/{}.jpg", series.id);

        SeriesRepository::update_custom_cover(
            db.sea_orm_connection(),
            series.id,
            Some(cover_path.clone()),
        )
        .await
        .unwrap();

        SeriesRepository::update_selected_cover_source(
            db.sea_orm_connection(),
            series.id,
            Some("custom".to_string()),
        )
        .await
        .unwrap();

        let retrieved = SeriesRepository::get_by_id(db.sea_orm_connection(), series.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.custom_cover_path, Some(cover_path));
        assert_eq!(retrieved.selected_cover_source, Some("custom".to_string()));

        // Switch back to default (first book cover)
        SeriesRepository::update_selected_cover_source(
            db.sea_orm_connection(),
            series.id,
            Some("default".to_string()),
        )
        .await
        .unwrap();

        let retrieved = SeriesRepository::get_by_id(db.sea_orm_connection(), series.id)
            .await
            .unwrap()
            .unwrap();

        // Cover path is still there, just not being used
        assert_eq!(
            retrieved.custom_cover_path,
            Some(format!("data/covers/{}.jpg", series.id))
        );
        assert_eq!(retrieved.selected_cover_source, Some("default".to_string()));
    }

    #[tokio::test]
    async fn test_list_in_progress() {
        let (db, _temp_dir) = create_test_db().await;

        // Create library
        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Create multiple series
        let series1 =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 1", None)
                .await
                .unwrap();
        let series2 =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 2", None)
                .await
                .unwrap();
        let series3 =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 3", None)
                .await
                .unwrap();

        // Create books in each series
        let book1 = books::Model {
            id: Uuid::new_v4(),
            series_id: series1.id,
            library_id: library.id,
            title: Some("Book 1".to_string()),
            number: None,
            file_path: "/test/book1.cbz".to_string(),
            file_name: "book1.cbz".to_string(),
            file_size: 1024,
            file_hash: format!("hash_{}", Uuid::new_v4()),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 10,
            deleted: false,
            analyzed: false,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
        };
        let book1: books::Model = BookRepository::create(db.sea_orm_connection(), &book1, None)
            .await
            .unwrap();

        let book2 = books::Model {
            id: Uuid::new_v4(),
            series_id: series2.id,
            library_id: library.id,
            title: Some("Book 2".to_string()),
            number: None,
            file_path: "/test/book2.cbz".to_string(),
            file_name: "book2.cbz".to_string(),
            file_size: 1024,
            file_hash: format!("hash_{}", Uuid::new_v4()),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 10,
            deleted: false,
            analyzed: false,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
        };
        let book2: books::Model = BookRepository::create(db.sea_orm_connection(), &book2, None)
            .await
            .unwrap();

        let book3 = books::Model {
            id: Uuid::new_v4(),
            series_id: series3.id,
            library_id: library.id,
            title: Some("Book 3".to_string()),
            number: None,
            file_path: "/test/book3.cbz".to_string(),
            file_name: "book3.cbz".to_string(),
            file_size: 1024,
            file_hash: format!("hash_{}", Uuid::new_v4()),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 10,
            deleted: false,
            analyzed: false,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
        };
        let book3: books::Model = BookRepository::create(db.sea_orm_connection(), &book3, None)
            .await
            .unwrap();

        // Create user
        use crate::api::permissions::ADMIN_PERMISSIONS;
        use crate::db::entities::users;
        use crate::db::repositories::{ReadProgressRepository, UserRepository};
        use crate::utils::password;

        let password_hash = password::hash_password("test123").unwrap();
        let permissions_vec: Vec<_> = ADMIN_PERMISSIONS.iter().cloned().collect();
        let user = users::Model {
            id: Uuid::new_v4(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password_hash,
            is_admin: false,
            is_active: true,
            email_verified: true,
            permissions: serde_json::to_value(&permissions_vec).unwrap(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };
        let created_user = UserRepository::create(db.sea_orm_connection(), &user)
            .await
            .unwrap();

        // Add reading progress for series1 and series2 (in-progress)
        ReadProgressRepository::upsert(
            db.sea_orm_connection(),
            created_user.id,
            book1.id,
            5,
            false,
        )
        .await
        .unwrap();
        ReadProgressRepository::upsert(
            db.sea_orm_connection(),
            created_user.id,
            book2.id,
            5,
            false,
        )
        .await
        .unwrap();

        // Mark series3 as completed
        ReadProgressRepository::upsert(
            db.sea_orm_connection(),
            created_user.id,
            book3.id,
            10,
            true,
        )
        .await
        .unwrap();

        // Test getting started series (only in-progress, not completed)
        let started =
            SeriesRepository::list_in_progress(db.sea_orm_connection(), created_user.id, None)
                .await
                .unwrap();

        assert_eq!(started.len(), 2); // Only series1 and series2 with in-progress books
        let series_ids: Vec<_> = started.iter().map(|s| s.id).collect();
        assert!(series_ids.contains(&series1.id));
        assert!(series_ids.contains(&series2.id));
        assert!(!series_ids.contains(&series3.id)); // Completed books not included

        // Test filtering by library
        let started = SeriesRepository::list_in_progress(
            db.sea_orm_connection(),
            created_user.id,
            Some(library.id),
        )
        .await
        .unwrap();

        assert_eq!(started.len(), 2);
    }
}
