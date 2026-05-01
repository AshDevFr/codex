//! Repository for series_metadata table operations
//!
//! TODO: Remove allow(dead_code) when all metadata features are fully integrated

#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use uuid::Uuid;

use crate::db::entities::{series_metadata, series_metadata::Entity as SeriesMetadata};
use crate::utils::normalize_for_search;

/// Repository for series metadata operations
pub struct SeriesMetadataRepository;

impl SeriesMetadataRepository {
    /// Get metadata for a series by series ID
    pub async fn get_by_series_id(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Option<series_metadata::Model>> {
        let result = SeriesMetadata::find_by_id(series_id).one(db).await?;
        Ok(result)
    }

    /// Get metadata for multiple series by their IDs
    ///
    /// Returns a HashMap keyed by series_id for efficient lookups
    pub async fn get_by_series_ids(
        db: &DatabaseConnection,
        series_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, series_metadata::Model>> {
        use sea_orm::{ColumnTrait, QueryFilter};

        if series_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let results = SeriesMetadata::find()
            .filter(series_metadata::Column::SeriesId.is_in(series_ids.to_vec()))
            .all(db)
            .await?;

        Ok(results.into_iter().map(|m| (m.series_id, m)).collect())
    }

    /// Create initial metadata for a series
    /// This is typically called when a series is created
    pub async fn create(
        db: &DatabaseConnection,
        series_id: Uuid,
        title: &str,
    ) -> Result<series_metadata::Model> {
        let now = Utc::now();

        let active_model = series_metadata::ActiveModel {
            series_id: Set(series_id),
            title: Set(title.to_string()),
            title_sort: Set(None),
            search_title: Set(normalize_for_search(title)),
            summary: Set(None),
            publisher: Set(None),
            imprint: Set(None),
            status: Set(None),
            age_rating: Set(None),
            language: Set(None),
            reading_direction: Set(None),
            year: Set(None),
            total_book_count: Set(None),
            total_volume_count: Set(None),
            total_chapter_count: Set(None),
            custom_metadata: Set(None),
            authors_json: Set(None),
            total_book_count_lock: Set(false),
            total_volume_count_lock: Set(false),
            total_chapter_count_lock: Set(false),
            title_lock: Set(false),
            title_sort_lock: Set(false),
            summary_lock: Set(false),
            publisher_lock: Set(false),
            imprint_lock: Set(false),
            status_lock: Set(false),
            age_rating_lock: Set(false),
            language_lock: Set(false),
            reading_direction_lock: Set(false),
            year_lock: Set(false),
            genres_lock: Set(false),
            tags_lock: Set(false),
            custom_metadata_lock: Set(false),
            authors_json_lock: Set(false),
            cover_lock: Set(false),
            alternate_titles_lock: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let model = active_model.insert(db).await?;
        Ok(model)
    }

    /// Update series metadata (full replacement)
    pub async fn replace(
        db: &DatabaseConnection,
        series_id: Uuid,
        title_sort: Option<String>,
        summary: Option<String>,
        publisher: Option<String>,
        year: Option<i32>,
        reading_direction: Option<String>,
    ) -> Result<series_metadata::Model> {
        let existing = Self::get_by_series_id(db, series_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Series metadata not found for series: {}", series_id)
            })?;

        let mut active_model: series_metadata::ActiveModel = existing.into();
        active_model.title_sort = Set(title_sort);
        active_model.summary = Set(summary);
        active_model.publisher = Set(publisher);
        active_model.year = Set(year);
        active_model.reading_direction = Set(reading_direction);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Update specific fields in series metadata (partial update)
    pub async fn update(
        db: &DatabaseConnection,
        metadata: &series_metadata::Model,
    ) -> Result<series_metadata::Model> {
        let mut active_model: series_metadata::ActiveModel = metadata.clone().into();
        active_model.updated_at = Set(Utc::now());
        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Update title and title_sort
    pub async fn update_title(
        db: &DatabaseConnection,
        series_id: Uuid,
        title: String,
        title_sort: Option<String>,
    ) -> Result<series_metadata::Model> {
        let existing = Self::get_by_series_id(db, series_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Series metadata not found for series: {}", series_id)
            })?;

        let mut active_model: series_metadata::ActiveModel = existing.into();
        active_model.search_title = Set(normalize_for_search(&title));
        active_model.title = Set(title);
        active_model.title_sort = Set(title_sort);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Update summary
    pub async fn update_summary(
        db: &DatabaseConnection,
        series_id: Uuid,
        summary: Option<String>,
    ) -> Result<series_metadata::Model> {
        let existing = Self::get_by_series_id(db, series_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Series metadata not found for series: {}", series_id)
            })?;

        let mut active_model: series_metadata::ActiveModel = existing.into();
        active_model.summary = Set(summary);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Update publisher and imprint
    pub async fn update_publisher(
        db: &DatabaseConnection,
        series_id: Uuid,
        publisher: Option<String>,
        imprint: Option<String>,
    ) -> Result<series_metadata::Model> {
        let existing = Self::get_by_series_id(db, series_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Series metadata not found for series: {}", series_id)
            })?;

        let mut active_model: series_metadata::ActiveModel = existing.into();
        active_model.publisher = Set(publisher);
        active_model.imprint = Set(imprint);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Update reading direction
    pub async fn update_reading_direction(
        db: &DatabaseConnection,
        series_id: Uuid,
        reading_direction: Option<String>,
    ) -> Result<series_metadata::Model> {
        let existing = Self::get_by_series_id(db, series_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Series metadata not found for series: {}", series_id)
            })?;

        let mut active_model: series_metadata::ActiveModel = existing.into();
        active_model.reading_direction = Set(reading_direction);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Update year
    pub async fn update_year(
        db: &DatabaseConnection,
        series_id: Uuid,
        year: Option<i32>,
    ) -> Result<series_metadata::Model> {
        let existing = Self::get_by_series_id(db, series_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Series metadata not found for series: {}", series_id)
            })?;

        let mut active_model: series_metadata::ActiveModel = existing.into();
        active_model.year = Set(year);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Update status
    pub async fn update_status(
        db: &DatabaseConnection,
        series_id: Uuid,
        status: Option<String>,
    ) -> Result<series_metadata::Model> {
        let existing = Self::get_by_series_id(db, series_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Series metadata not found for series: {}", series_id)
            })?;

        let mut active_model: series_metadata::ActiveModel = existing.into();
        active_model.status = Set(status);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Update age rating
    pub async fn update_age_rating(
        db: &DatabaseConnection,
        series_id: Uuid,
        age_rating: Option<i32>,
    ) -> Result<series_metadata::Model> {
        let existing = Self::get_by_series_id(db, series_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Series metadata not found for series: {}", series_id)
            })?;

        let mut active_model: series_metadata::ActiveModel = existing.into();
        active_model.age_rating = Set(age_rating);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Update language
    pub async fn update_language(
        db: &DatabaseConnection,
        series_id: Uuid,
        language: Option<String>,
    ) -> Result<series_metadata::Model> {
        let existing = Self::get_by_series_id(db, series_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Series metadata not found for series: {}", series_id)
            })?;

        let mut active_model: series_metadata::ActiveModel = existing.into();
        active_model.language = Set(language);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Update total book count (expected number of books in the series)
    ///
    /// DEPRECATED: kept through Phase 4 of metadata-count-split for the legacy column;
    /// new callers should use [`update_total_volume_count`] and/or
    /// [`update_total_chapter_count`]. Removed entirely in Phase 9.
    pub async fn update_total_book_count(
        db: &DatabaseConnection,
        series_id: Uuid,
        total_book_count: Option<i32>,
    ) -> Result<series_metadata::Model> {
        let existing = Self::get_by_series_id(db, series_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Series metadata not found for series: {}", series_id)
            })?;

        let mut active_model: series_metadata::ActiveModel = existing.into();
        active_model.total_book_count = Set(total_book_count);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Update the expected total volume count for a series.
    ///
    /// Pass `None` to clear the value. The lock state is independent and unchanged.
    pub async fn update_total_volume_count(
        db: &DatabaseConnection,
        series_id: Uuid,
        total_volume_count: Option<i32>,
    ) -> Result<series_metadata::Model> {
        let existing = Self::get_by_series_id(db, series_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Series metadata not found for series: {}", series_id)
            })?;

        let mut active_model: series_metadata::ActiveModel = existing.into();
        active_model.total_volume_count = Set(total_volume_count);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Update the expected total chapter count for a series.
    ///
    /// Pass `None` to clear the value. Chapters are stored as `f32` to support
    /// fractional chapter numbers like 47.5 that some providers expose. The lock
    /// state is independent and unchanged.
    pub async fn update_total_chapter_count(
        db: &DatabaseConnection,
        series_id: Uuid,
        total_chapter_count: Option<f32>,
    ) -> Result<series_metadata::Model> {
        let existing = Self::get_by_series_id(db, series_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Series metadata not found for series: {}", series_id)
            })?;

        let mut active_model: series_metadata::ActiveModel = existing.into();
        active_model.total_chapter_count = Set(total_chapter_count);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Update authors JSON
    pub async fn update_authors_json(
        db: &DatabaseConnection,
        series_id: Uuid,
        authors_json: Option<String>,
    ) -> Result<series_metadata::Model> {
        let existing = Self::get_by_series_id(db, series_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Series metadata not found for series: {}", series_id)
            })?;

        let mut active_model: series_metadata::ActiveModel = existing.into();
        active_model.authors_json = Set(authors_json);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Lock or unlock a specific metadata field
    pub async fn set_lock(
        db: &DatabaseConnection,
        series_id: Uuid,
        field: &str,
        locked: bool,
    ) -> Result<series_metadata::Model> {
        let existing = Self::get_by_series_id(db, series_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Series metadata not found for series: {}", series_id)
            })?;

        let mut active_model: series_metadata::ActiveModel = existing.into();

        match field {
            "title" => active_model.title_lock = Set(locked),
            "title_sort" => active_model.title_sort_lock = Set(locked),
            "summary" => active_model.summary_lock = Set(locked),
            "publisher" => active_model.publisher_lock = Set(locked),
            "imprint" => active_model.imprint_lock = Set(locked),
            "status" => active_model.status_lock = Set(locked),
            "age_rating" => active_model.age_rating_lock = Set(locked),
            "language" => active_model.language_lock = Set(locked),
            "reading_direction" => active_model.reading_direction_lock = Set(locked),
            "year" => active_model.year_lock = Set(locked),
            "total_book_count" => active_model.total_book_count_lock = Set(locked),
            "total_volume_count" => active_model.total_volume_count_lock = Set(locked),
            "total_chapter_count" => active_model.total_chapter_count_lock = Set(locked),
            "genres" => active_model.genres_lock = Set(locked),
            "tags" => active_model.tags_lock = Set(locked),
            "cover" => active_model.cover_lock = Set(locked),
            "alternate_titles" => active_model.alternate_titles_lock = Set(locked),
            _ => return Err(anyhow::anyhow!("Unknown field: {}", field)),
        }

        active_model.updated_at = Set(Utc::now());
        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Check if a specific field is locked
    pub fn is_field_locked(metadata: &series_metadata::Model, field: &str) -> bool {
        match field {
            "title" => metadata.title_lock,
            "title_sort" => metadata.title_sort_lock,
            "summary" => metadata.summary_lock,
            "publisher" => metadata.publisher_lock,
            "imprint" => metadata.imprint_lock,
            "status" => metadata.status_lock,
            "age_rating" => metadata.age_rating_lock,
            "language" => metadata.language_lock,
            "reading_direction" => metadata.reading_direction_lock,
            "year" => metadata.year_lock,
            "total_book_count" => metadata.total_book_count_lock,
            "total_volume_count" => metadata.total_volume_count_lock,
            "total_chapter_count" => metadata.total_chapter_count_lock,
            "genres" => metadata.genres_lock,
            "tags" => metadata.tags_lock,
            "cover" => metadata.cover_lock,
            "alternate_titles" => metadata.alternate_titles_lock,
            _ => false,
        }
    }

    /// Update cover lock state
    pub async fn update_cover_lock(
        db: &DatabaseConnection,
        series_id: Uuid,
        locked: bool,
    ) -> Result<series_metadata::Model> {
        let existing = Self::get_by_series_id(db, series_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Series metadata not found for series: {}", series_id)
            })?;

        let mut active_model: series_metadata::ActiveModel = existing.into();
        active_model.cover_lock = Set(locked);
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(db).await?;
        Ok(model)
    }

    /// Delete metadata for a series (cascaded automatically when series is deleted)
    pub async fn delete(db: &DatabaseConnection, series_id: Uuid) -> Result<()> {
        SeriesMetadata::delete_by_id(series_id).exec(db).await?;
        Ok(())
    }

    /// Reset metadata for a series back to filesystem-derived defaults.
    ///
    /// Deletes the existing metadata row (which cascades to clear lock states)
    /// and recreates a fresh one with just the series directory name as the title.
    /// Does NOT clear related data (genres, tags, etc.) — callers must do that.
    pub async fn reset(
        db: &DatabaseConnection,
        series_id: Uuid,
        title: &str,
    ) -> Result<series_metadata::Model> {
        // Delete existing metadata row
        Self::delete(db, series_id).await?;
        // Recreate with defaults
        Self::create(db, series_id, title).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::repositories::{LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;

    #[tokio::test]
    async fn test_create_and_get_metadata() {
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

        // The series_metadata should have been created automatically by create_with_fingerprint
        // or we can create it manually for testing
        let metadata =
            SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), series.id)
                .await
                .unwrap();

        assert!(metadata.is_some());
        let metadata = metadata.unwrap();
        assert_eq!(metadata.series_id, series.id);
        assert_eq!(metadata.title, "Test Series");
        assert!(metadata.summary.is_none());
    }

    #[tokio::test]
    async fn test_update_metadata() {
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

        // Update summary
        let updated = SeriesMetadataRepository::update_summary(
            db.sea_orm_connection(),
            series.id,
            Some("A test summary".to_string()),
        )
        .await
        .unwrap();

        assert_eq!(updated.summary, Some("A test summary".to_string()));

        // Update publisher
        let updated = SeriesMetadataRepository::update_publisher(
            db.sea_orm_connection(),
            series.id,
            Some("Test Publisher".to_string()),
            Some("Test Imprint".to_string()),
        )
        .await
        .unwrap();

        assert_eq!(updated.publisher, Some("Test Publisher".to_string()));
        assert_eq!(updated.imprint, Some("Test Imprint".to_string()));
    }

    #[tokio::test]
    async fn test_lock_fields() {
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

        // Lock summary field
        let metadata =
            SeriesMetadataRepository::set_lock(db.sea_orm_connection(), series.id, "summary", true)
                .await
                .unwrap();

        assert!(metadata.summary_lock);
        assert!(SeriesMetadataRepository::is_field_locked(
            &metadata, "summary"
        ));
        assert!(!SeriesMetadataRepository::is_field_locked(
            &metadata,
            "publisher"
        ));

        // Unlock summary field
        let metadata = SeriesMetadataRepository::set_lock(
            db.sea_orm_connection(),
            series.id,
            "summary",
            false,
        )
        .await
        .unwrap();

        assert!(!metadata.summary_lock);
    }

    #[tokio::test]
    async fn test_replace_metadata() {
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

        // Replace all metadata
        let updated = SeriesMetadataRepository::replace(
            db.sea_orm_connection(),
            series.id,
            Some("test series".to_string()),
            Some("A new summary".to_string()),
            Some("New Publisher".to_string()),
            Some(2024),
            Some("rtl".to_string()),
        )
        .await
        .unwrap();

        assert_eq!(updated.title_sort, Some("test series".to_string()));
        assert_eq!(updated.summary, Some("A new summary".to_string()));
        assert_eq!(updated.publisher, Some("New Publisher".to_string()));
        assert_eq!(updated.year, Some(2024));
        assert_eq!(updated.reading_direction, Some("rtl".to_string()));
    }

    #[tokio::test]
    async fn test_reset_metadata() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "My Series", None)
                .await
                .unwrap();

        // Populate metadata
        SeriesMetadataRepository::update_title(
            db.sea_orm_connection(),
            series.id,
            "Plugin Title".to_string(),
            Some("sort title".to_string()),
        )
        .await
        .unwrap();

        SeriesMetadataRepository::update_summary(
            db.sea_orm_connection(),
            series.id,
            Some("A summary".to_string()),
        )
        .await
        .unwrap();

        SeriesMetadataRepository::set_lock(db.sea_orm_connection(), series.id, "title", true)
            .await
            .unwrap();

        // Reset metadata back to series.name
        let reset =
            SeriesMetadataRepository::reset(db.sea_orm_connection(), series.id, "My Series")
                .await
                .unwrap();

        assert_eq!(reset.series_id, series.id);
        assert_eq!(reset.title, "My Series");
        assert!(reset.title_sort.is_none());
        assert!(reset.summary.is_none());
        assert!(reset.publisher.is_none());
        assert!(!reset.title_lock);
        assert!(!reset.summary_lock);
    }

    /// Helper that creates a library + series and returns the series UUID.
    async fn make_series(db: &crate::db::Database, name: &str) -> Uuid {
        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();
        SeriesRepository::create(db.sea_orm_connection(), library.id, name, None)
            .await
            .unwrap()
            .id
    }

    #[tokio::test]
    async fn test_update_total_volume_count_round_trip() {
        let (db, _temp_dir) = create_test_db().await;
        let series_id = make_series(&db, "Vol Series").await;

        // Defaults: both new fields unset, both locks false.
        let initial =
            SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), series_id)
                .await
                .unwrap()
                .unwrap();
        assert!(initial.total_volume_count.is_none());
        assert!(!initial.total_volume_count_lock);

        // Set volume count.
        let updated = SeriesMetadataRepository::update_total_volume_count(
            db.sea_orm_connection(),
            series_id,
            Some(14),
        )
        .await
        .unwrap();
        assert_eq!(updated.total_volume_count, Some(14));
        // Lock should not have been touched.
        assert!(!updated.total_volume_count_lock);
        // Chapter side untouched.
        assert!(updated.total_chapter_count.is_none());

        // Clear via None.
        let cleared = SeriesMetadataRepository::update_total_volume_count(
            db.sea_orm_connection(),
            series_id,
            None,
        )
        .await
        .unwrap();
        assert!(cleared.total_volume_count.is_none());
    }

    #[tokio::test]
    async fn test_update_total_chapter_count_round_trip_fractional() {
        let (db, _temp_dir) = create_test_db().await;
        let series_id = make_series(&db, "Chap Series").await;

        // Set fractional chapter count to confirm float storage works.
        let updated = SeriesMetadataRepository::update_total_chapter_count(
            db.sea_orm_connection(),
            series_id,
            Some(109.5),
        )
        .await
        .unwrap();
        assert_eq!(updated.total_chapter_count, Some(109.5));
        assert!(!updated.total_chapter_count_lock);
        // Volume side untouched.
        assert!(updated.total_volume_count.is_none());

        // Clear via None.
        let cleared = SeriesMetadataRepository::update_total_chapter_count(
            db.sea_orm_connection(),
            series_id,
            None,
        )
        .await
        .unwrap();
        assert!(cleared.total_chapter_count.is_none());
    }

    #[tokio::test]
    async fn test_volume_and_chapter_locks_are_independent() {
        let (db, _temp_dir) = create_test_db().await;
        let series_id = make_series(&db, "Lock Series").await;

        // Lock volume count, leave chapter count untouched.
        let after_vol_lock = SeriesMetadataRepository::set_lock(
            db.sea_orm_connection(),
            series_id,
            "total_volume_count",
            true,
        )
        .await
        .unwrap();
        assert!(after_vol_lock.total_volume_count_lock);
        assert!(!after_vol_lock.total_chapter_count_lock);
        assert!(SeriesMetadataRepository::is_field_locked(
            &after_vol_lock,
            "total_volume_count"
        ));
        assert!(!SeriesMetadataRepository::is_field_locked(
            &after_vol_lock,
            "total_chapter_count"
        ));

        // Lock chapter count too; both locked now.
        let after_chap_lock = SeriesMetadataRepository::set_lock(
            db.sea_orm_connection(),
            series_id,
            "total_chapter_count",
            true,
        )
        .await
        .unwrap();
        assert!(after_chap_lock.total_volume_count_lock);
        assert!(after_chap_lock.total_chapter_count_lock);

        // Unlock chapter count: volume lock stays.
        let after_unlock = SeriesMetadataRepository::set_lock(
            db.sea_orm_connection(),
            series_id,
            "total_chapter_count",
            false,
        )
        .await
        .unwrap();
        assert!(after_unlock.total_volume_count_lock);
        assert!(!after_unlock.total_chapter_count_lock);
    }
}
