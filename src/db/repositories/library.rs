use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::db::entities::{libraries, prelude::*};
use crate::models::{BookStrategy, SeriesStrategy};

/// Parameters for creating a new library
#[derive(Debug, Clone)]
pub struct CreateLibraryParams {
    pub name: String,
    pub path: String,
    pub series_strategy: SeriesStrategy,
    pub series_config: Option<serde_json::Value>,
    pub book_strategy: BookStrategy,
    pub book_config: Option<serde_json::Value>,
    pub scanning_config: Option<String>,
    pub default_reading_direction: Option<String>,
    pub allowed_formats: Option<String>,
    pub excluded_patterns: Option<String>,
}

impl CreateLibraryParams {
    /// Create params with default strategies
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            series_strategy: SeriesStrategy::default(),
            series_config: None,
            book_strategy: BookStrategy::default(),
            book_config: None,
            scanning_config: None,
            default_reading_direction: None,
            allowed_formats: None,
            excluded_patterns: None,
        }
    }

    pub fn with_series_strategy(mut self, strategy: SeriesStrategy) -> Self {
        self.series_strategy = strategy;
        self
    }

    pub fn with_series_config(mut self, config: Option<serde_json::Value>) -> Self {
        self.series_config = config;
        self
    }

    pub fn with_book_strategy(mut self, strategy: BookStrategy) -> Self {
        self.book_strategy = strategy;
        self
    }

    pub fn with_book_config(mut self, config: Option<serde_json::Value>) -> Self {
        self.book_config = config;
        self
    }

    pub fn with_scanning_config(mut self, config: Option<String>) -> Self {
        self.scanning_config = config;
        self
    }
}

/// Repository for Library operations
pub struct LibraryRepository;

impl LibraryRepository {
    /// Create a new library with full parameters
    pub async fn create_with_params(
        db: &DatabaseConnection,
        params: CreateLibraryParams,
    ) -> Result<libraries::Model> {
        let now = Utc::now();

        let library = libraries::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(params.name),
            path: Set(params.path),
            series_strategy: Set(params.series_strategy.as_str().to_string()),
            series_config: Set(params.series_config),
            book_strategy: Set(params.book_strategy.as_str().to_string()),
            book_config: Set(params.book_config),
            scanning_config: Set(params.scanning_config),
            default_reading_direction: Set(params
                .default_reading_direction
                .unwrap_or_else(|| "LEFT_TO_RIGHT".to_string())),
            allowed_formats: Set(params.allowed_formats),
            excluded_patterns: Set(params.excluded_patterns),
            created_at: Set(now),
            updated_at: Set(now),
            last_scanned_at: Set(None),
        };

        library.insert(db).await.context("Failed to create library")
    }

    /// Create a new library (legacy signature for backward compatibility)
    pub async fn create(
        db: &DatabaseConnection,
        name: &str,
        path: &str,
        _strategy: crate::db::ScanningStrategy, // Legacy parameter, ignored
    ) -> Result<libraries::Model> {
        let params = CreateLibraryParams::new(name, path);
        Self::create_with_params(db, params).await
    }

    /// Get a library by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<libraries::Model>> {
        Libraries::find_by_id(id)
            .one(db)
            .await
            .context("Failed to get library by ID")
    }

    /// Get all libraries
    pub async fn list_all(db: &DatabaseConnection) -> Result<Vec<libraries::Model>> {
        Libraries::find()
            .order_by_asc(libraries::Column::Name)
            .all(db)
            .await
            .context("Failed to list libraries")
    }

    /// Get library by path
    pub async fn get_by_path(
        db: &DatabaseConnection,
        path: &str,
    ) -> Result<Option<libraries::Model>> {
        Libraries::find()
            .filter(libraries::Column::Path.eq(path))
            .one(db)
            .await
            .context("Failed to get library by path")
    }

    /// Update library
    pub async fn update(db: &DatabaseConnection, library: &libraries::Model) -> Result<()> {
        let active = libraries::ActiveModel {
            id: Set(library.id),
            name: Set(library.name.clone()),
            path: Set(library.path.clone()),
            series_strategy: Set(library.series_strategy.clone()),
            series_config: Set(library.series_config.clone()),
            book_strategy: Set(library.book_strategy.clone()),
            book_config: Set(library.book_config.clone()),
            scanning_config: Set(library.scanning_config.clone()),
            default_reading_direction: Set(library.default_reading_direction.clone()),
            allowed_formats: Set(library.allowed_formats.clone()),
            excluded_patterns: Set(library.excluded_patterns.clone()),
            created_at: Set(library.created_at),
            updated_at: Set(Utc::now()),
            last_scanned_at: Set(library.last_scanned_at),
        };

        active
            .update(db)
            .await
            .context("Failed to update library")?;

        Ok(())
    }

    /// Update last scanned timestamp
    pub async fn update_last_scanned(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        let library = Libraries::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Library not found"))?;

        let mut active: libraries::ActiveModel = library.into();
        active.last_scanned_at = Set(Some(Utc::now()));
        active.updated_at = Set(Utc::now());

        active
            .update(db)
            .await
            .context("Failed to update last scanned timestamp")?;

        Ok(())
    }

    /// Delete a library
    /// Note: task_metrics are automatically deleted via CASCADE foreign key
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        Libraries::delete_by_id(id)
            .exec(db)
            .await
            .context("Failed to delete library")?;
        Ok(())
    }

    /// Get the series strategy for a library
    pub fn get_series_strategy(library: &libraries::Model) -> SeriesStrategy {
        SeriesStrategy::from_str(&library.series_strategy).unwrap_or_default()
    }

    /// Get the book strategy for a library
    pub fn get_book_strategy(library: &libraries::Model) -> BookStrategy {
        BookStrategy::from_str(&library.book_strategy).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::create_test_db;
    use crate::db::ScanningStrategy;

    #[tokio::test]
    async fn test_create_library() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        assert_eq!(library.name, "Test Library");
        assert_eq!(library.path, "/test/path");
        assert_eq!(library.series_strategy, "series_volume");
        assert_eq!(library.book_strategy, "filename");
    }

    #[tokio::test]
    async fn test_create_library_with_params() {
        let (db, _temp_dir) = create_test_db().await;

        let params = CreateLibraryParams::new("Manga Library", "/manga/path")
            .with_series_strategy(SeriesStrategy::SeriesVolumeChapter)
            .with_book_strategy(BookStrategy::Smart);

        let library = LibraryRepository::create_with_params(db.sea_orm_connection(), params)
            .await
            .unwrap();

        assert_eq!(library.name, "Manga Library");
        assert_eq!(library.series_strategy, "series_volume_chapter");
        assert_eq!(library.book_strategy, "smart");
    }

    #[tokio::test]
    async fn test_get_library_by_id() {
        let (db, _temp_dir) = create_test_db().await;

        let created = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let retrieved = LibraryRepository::get_by_id(db.sea_orm_connection(), created.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.name, "Test Library");
    }

    #[tokio::test]
    async fn test_get_library_by_id_not_found() {
        let (db, _temp_dir) = create_test_db().await;

        let result = LibraryRepository::get_by_id(db.sea_orm_connection(), Uuid::new_v4())
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_all_libraries() {
        let (db, _temp_dir) = create_test_db().await;

        LibraryRepository::create(
            db.sea_orm_connection(),
            "Library 1",
            "/path1",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        LibraryRepository::create(
            db.sea_orm_connection(),
            "Library 2",
            "/path2",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let libraries = LibraryRepository::list_all(db.sea_orm_connection())
            .await
            .unwrap();

        assert_eq!(libraries.len(), 2);
        assert_eq!(libraries[0].name, "Library 1");
        assert_eq!(libraries[1].name, "Library 2");
    }

    #[tokio::test]
    async fn test_get_library_by_path() {
        let (db, _temp_dir) = create_test_db().await;

        let created = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let retrieved = LibraryRepository::get_by_path(db.sea_orm_connection(), "/test/path")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.path, "/test/path");
    }

    #[tokio::test]
    async fn test_update_library() {
        let (db, _temp_dir) = create_test_db().await;

        let mut library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Original Name",
            "/original/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        library.name = "Updated Name".to_string();
        library.path = "/updated/path".to_string();

        LibraryRepository::update(db.sea_orm_connection(), &library)
            .await
            .unwrap();

        let retrieved = LibraryRepository::get_by_id(db.sea_orm_connection(), library.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.name, "Updated Name");
        assert_eq!(retrieved.path, "/updated/path");
    }

    #[tokio::test]
    async fn test_update_last_scanned() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        assert!(library.last_scanned_at.is_none());

        LibraryRepository::update_last_scanned(db.sea_orm_connection(), library.id)
            .await
            .unwrap();

        let retrieved = LibraryRepository::get_by_id(db.sea_orm_connection(), library.id)
            .await
            .unwrap()
            .unwrap();

        assert!(retrieved.last_scanned_at.is_some());
    }

    #[tokio::test]
    async fn test_delete_library() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "To Delete",
            "/delete/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        LibraryRepository::delete(db.sea_orm_connection(), library.id)
            .await
            .unwrap();

        let result = LibraryRepository::get_by_id(db.sea_orm_connection(), library.id)
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete_library_also_deletes_task_metrics() {
        use crate::db::repositories::task_metrics::{TaskCompletionData, TaskMetricsRepository};

        let (db, _temp_dir) = create_test_db().await;

        // Create a library
        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Library with Metrics",
            "/metrics/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Record some task metrics for this library
        let data = TaskCompletionData {
            task_type: "scan_library".to_string(),
            library_id: Some(library.id),
            success: true,
            retried: false,
            duration_ms: 1000,
            queue_wait_ms: 50,
            items_processed: 10,
            bytes_processed: 1024,
            error: None,
        };
        TaskMetricsRepository::record_completion(db.sea_orm_connection(), data)
            .await
            .unwrap();

        // Verify metrics exist
        let metrics = TaskMetricsRepository::get_current_aggregates(db.sea_orm_connection())
            .await
            .unwrap();
        assert!(!metrics.is_empty());
        assert!(metrics.iter().any(|m| m.library_id == Some(library.id)));

        // Delete the library
        LibraryRepository::delete(db.sea_orm_connection(), library.id)
            .await
            .unwrap();

        // Verify library is deleted
        let result = LibraryRepository::get_by_id(db.sea_orm_connection(), library.id)
            .await
            .unwrap();
        assert!(result.is_none());

        // Verify task metrics for this library are also deleted
        let metrics = TaskMetricsRepository::get_current_aggregates(db.sea_orm_connection())
            .await
            .unwrap();
        assert!(!metrics.iter().any(|m| m.library_id == Some(library.id)));
    }

    #[tokio::test]
    async fn test_library_default_reading_direction() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Should default to LEFT_TO_RIGHT
        assert_eq!(library.default_reading_direction, "LEFT_TO_RIGHT");
    }

    #[tokio::test]
    async fn test_library_update_reading_direction() {
        let (db, _temp_dir) = create_test_db().await;

        let mut library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Manga Library",
            "/manga/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Update reading direction to right-to-left for manga
        library.default_reading_direction = "RIGHT_TO_LEFT".to_string();
        LibraryRepository::update(db.sea_orm_connection(), &library)
            .await
            .unwrap();

        let retrieved = LibraryRepository::get_by_id(db.sea_orm_connection(), library.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.default_reading_direction, "RIGHT_TO_LEFT");
    }

    #[tokio::test]
    async fn test_library_allowed_formats() {
        let (db, _temp_dir) = create_test_db().await;

        let mut library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Comic Library",
            "/comics/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Set allowed formats to only CBZ and CBR
        library.allowed_formats = Some(r#"["CBZ","CBR"]"#.to_string());
        LibraryRepository::update(db.sea_orm_connection(), &library)
            .await
            .unwrap();

        let retrieved = LibraryRepository::get_by_id(db.sea_orm_connection(), library.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            retrieved.allowed_formats,
            Some(r#"["CBZ","CBR"]"#.to_string())
        );
    }

    #[tokio::test]
    async fn test_library_excluded_patterns() {
        let (db, _temp_dir) = create_test_db().await;

        let mut library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Set excluded patterns
        let patterns = ".DS_Store\nThumbs.db\n@eaDir/*";
        library.excluded_patterns = Some(patterns.to_string());
        LibraryRepository::update(db.sea_orm_connection(), &library)
            .await
            .unwrap();

        let retrieved = LibraryRepository::get_by_id(db.sea_orm_connection(), library.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.excluded_patterns, Some(patterns.to_string()));
    }

    #[tokio::test]
    async fn test_library_all_new_fields() {
        let (db, _temp_dir) = create_test_db().await;

        let mut library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Complete Library",
            "/complete/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Update all new fields
        library.default_reading_direction = "TOP_TO_BOTTOM".to_string();
        library.allowed_formats = Some(r#"["EPUB","PDF"]"#.to_string());
        library.excluded_patterns = Some("*.tmp\n*.bak".to_string());

        LibraryRepository::update(db.sea_orm_connection(), &library)
            .await
            .unwrap();

        let retrieved = LibraryRepository::get_by_id(db.sea_orm_connection(), library.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.default_reading_direction, "TOP_TO_BOTTOM");
        assert_eq!(
            retrieved.allowed_formats,
            Some(r#"["EPUB","PDF"]"#.to_string())
        );
        assert_eq!(
            retrieved.excluded_patterns,
            Some("*.tmp\n*.bak".to_string())
        );
    }

    #[tokio::test]
    async fn test_get_series_strategy() {
        let (db, _temp_dir) = create_test_db().await;

        let params = CreateLibraryParams::new("Test", "/test")
            .with_series_strategy(SeriesStrategy::SeriesVolumeChapter);

        let library = LibraryRepository::create_with_params(db.sea_orm_connection(), params)
            .await
            .unwrap();

        let strategy = LibraryRepository::get_series_strategy(&library);
        assert_eq!(strategy, SeriesStrategy::SeriesVolumeChapter);
    }

    #[tokio::test]
    async fn test_get_book_strategy() {
        let (db, _temp_dir) = create_test_db().await;

        let params =
            CreateLibraryParams::new("Test", "/test").with_book_strategy(BookStrategy::Smart);

        let library = LibraryRepository::create_with_params(db.sea_orm_connection(), params)
            .await
            .unwrap();

        let strategy = LibraryRepository::get_book_strategy(&library);
        assert_eq!(strategy, BookStrategy::Smart);
    }
}
