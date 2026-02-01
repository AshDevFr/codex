//! Repository for library operations
//!
//! TODO: Remove allow(dead_code) once all library features are fully integrated

#![allow(dead_code)]

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::db::entities::{libraries, prelude::*};
use crate::models::{BookStrategy, NumberStrategy, SeriesStrategy};

/// Parameters for creating a new library
#[derive(Debug, Clone)]
pub struct CreateLibraryParams {
    pub name: String,
    pub path: String,
    pub series_strategy: SeriesStrategy,
    pub series_config: Option<serde_json::Value>,
    pub book_strategy: BookStrategy,
    pub book_config: Option<serde_json::Value>,
    pub number_strategy: NumberStrategy,
    pub number_config: Option<serde_json::Value>,
    pub scanning_config: Option<String>,
    pub default_reading_direction: Option<String>,
    pub allowed_formats: Option<String>,
    pub excluded_patterns: Option<String>,
    pub title_preprocessing_rules: Option<String>,
    pub auto_match_conditions: Option<String>,
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
            number_strategy: NumberStrategy::default(),
            number_config: None,
            scanning_config: None,
            default_reading_direction: None,
            allowed_formats: None,
            excluded_patterns: None,
            title_preprocessing_rules: None,
            auto_match_conditions: None,
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

    pub fn with_number_strategy(mut self, strategy: NumberStrategy) -> Self {
        self.number_strategy = strategy;
        self
    }

    pub fn with_number_config(mut self, config: Option<serde_json::Value>) -> Self {
        self.number_config = config;
        self
    }

    pub fn with_scanning_config(mut self, config: Option<String>) -> Self {
        self.scanning_config = config;
        self
    }

    pub fn with_title_preprocessing_rules(mut self, rules: Option<String>) -> Self {
        self.title_preprocessing_rules = rules;
        self
    }

    pub fn with_auto_match_conditions(mut self, conditions: Option<String>) -> Self {
        self.auto_match_conditions = conditions;
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
            number_strategy: Set(params.number_strategy.as_str().to_string()),
            number_config: Set(params.number_config),
            scanning_config: Set(params.scanning_config),
            default_reading_direction: Set(params
                .default_reading_direction
                .unwrap_or_else(|| "LEFT_TO_RIGHT".to_string())),
            allowed_formats: Set(params.allowed_formats),
            excluded_patterns: Set(params.excluded_patterns),
            title_preprocessing_rules: Set(params.title_preprocessing_rules),
            auto_match_conditions: Set(params.auto_match_conditions),
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

    /// Get libraries by multiple IDs
    ///
    /// Returns a HashMap keyed by library ID for efficient lookups
    pub async fn get_by_ids(
        db: &DatabaseConnection,
        ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, libraries::Model>> {
        use sea_orm::ColumnTrait;

        if ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let results = Libraries::find()
            .filter(libraries::Column::Id.is_in(ids.to_vec()))
            .all(db)
            .await
            .context("Failed to get libraries by IDs")?;

        Ok(results.into_iter().map(|lib| (lib.id, lib)).collect())
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
            number_strategy: Set(library.number_strategy.clone()),
            number_config: Set(library.number_config.clone()),
            scanning_config: Set(library.scanning_config.clone()),
            default_reading_direction: Set(library.default_reading_direction.clone()),
            allowed_formats: Set(library.allowed_formats.clone()),
            excluded_patterns: Set(library.excluded_patterns.clone()),
            title_preprocessing_rules: Set(library.title_preprocessing_rules.clone()),
            auto_match_conditions: Set(library.auto_match_conditions.clone()),
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
        library
            .series_strategy
            .parse::<SeriesStrategy>()
            .unwrap_or_default()
    }

    /// Get the book strategy for a library
    pub fn get_book_strategy(library: &libraries::Model) -> BookStrategy {
        library
            .book_strategy
            .parse::<BookStrategy>()
            .unwrap_or_default()
    }

    /// Get the number strategy for a library
    pub fn get_number_strategy(library: &libraries::Model) -> NumberStrategy {
        library
            .number_strategy
            .parse::<NumberStrategy>()
            .unwrap_or_default()
    }

    /// Get the preprocessing rules for a library
    ///
    /// Parses the JSON `title_preprocessing_rules` column into a vector of rules.
    /// Returns an empty vector if no rules are configured or if parsing fails.
    pub fn get_preprocessing_rules(
        library: &libraries::Model,
    ) -> Vec<crate::services::metadata::preprocessing::PreprocessingRule> {
        use crate::services::metadata::preprocessing::parse_preprocessing_rules;

        match parse_preprocessing_rules(library.title_preprocessing_rules.as_deref()) {
            Ok(rules) => rules,
            Err(e) => {
                tracing::warn!(
                    "Failed to parse preprocessing rules for library {}: {}",
                    library.id,
                    e
                );
                Vec::new()
            }
        }
    }

    /// Get the auto-match conditions for a library
    ///
    /// Parses the JSON `auto_match_conditions` column.
    /// Returns None if no conditions are configured or if parsing fails.
    pub fn get_auto_match_conditions(
        library: &libraries::Model,
    ) -> Option<crate::services::metadata::preprocessing::AutoMatchConditions> {
        use crate::services::metadata::preprocessing::parse_auto_match_conditions;

        match parse_auto_match_conditions(library.auto_match_conditions.as_deref()) {
            Ok(conditions) => conditions,
            Err(e) => {
                tracing::warn!(
                    "Failed to parse auto-match conditions for library {}: {}",
                    library.id,
                    e
                );
                None
            }
        }
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

    #[tokio::test]
    async fn test_get_number_strategy() {
        let (db, _temp_dir) = create_test_db().await;

        let params =
            CreateLibraryParams::new("Test", "/test").with_number_strategy(NumberStrategy::Smart);

        let library = LibraryRepository::create_with_params(db.sea_orm_connection(), params)
            .await
            .unwrap();

        let strategy = LibraryRepository::get_number_strategy(&library);
        assert_eq!(strategy, NumberStrategy::Smart);
    }

    #[tokio::test]
    async fn test_library_default_number_strategy() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Should default to file_order
        assert_eq!(library.number_strategy, "file_order");
        assert_eq!(
            LibraryRepository::get_number_strategy(&library),
            NumberStrategy::FileOrder
        );
    }

    #[tokio::test]
    async fn test_create_library_with_all_strategies() {
        let (db, _temp_dir) = create_test_db().await;

        let params = CreateLibraryParams::new("Full Strategy Library", "/full/path")
            .with_series_strategy(SeriesStrategy::SeriesVolumeChapter)
            .with_book_strategy(BookStrategy::Smart)
            .with_number_strategy(NumberStrategy::Filename);

        let library = LibraryRepository::create_with_params(db.sea_orm_connection(), params)
            .await
            .unwrap();

        assert_eq!(library.series_strategy, "series_volume_chapter");
        assert_eq!(library.book_strategy, "smart");
        assert_eq!(library.number_strategy, "filename");
    }

    #[tokio::test]
    async fn test_get_preprocessing_rules_none() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // No preprocessing rules configured
        let rules = LibraryRepository::get_preprocessing_rules(&library);
        assert!(rules.is_empty());
    }

    #[tokio::test]
    async fn test_get_preprocessing_rules_valid() {
        let (db, _temp_dir) = create_test_db().await;

        let mut library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Set preprocessing rules
        library.title_preprocessing_rules = Some(
            r#"[{"pattern": "\\s*\\(Digital\\)$", "replacement": "", "description": "Remove Digital suffix"}]"#
                .to_string(),
        );
        LibraryRepository::update(db.sea_orm_connection(), &library)
            .await
            .unwrap();

        let rules = LibraryRepository::get_preprocessing_rules(&library);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, r"\s*\(Digital\)$");
        assert_eq!(rules[0].replacement, "");
        assert_eq!(
            rules[0].description,
            Some("Remove Digital suffix".to_string())
        );
    }

    #[tokio::test]
    async fn test_get_preprocessing_rules_invalid_json() {
        let (db, _temp_dir) = create_test_db().await;

        let mut library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Set invalid JSON - should return empty vec
        library.title_preprocessing_rules = Some("not valid json".to_string());

        let rules = LibraryRepository::get_preprocessing_rules(&library);
        assert!(rules.is_empty());
    }

    #[tokio::test]
    async fn test_get_auto_match_conditions_none() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // No conditions configured
        let conditions = LibraryRepository::get_auto_match_conditions(&library);
        assert!(conditions.is_none());
    }

    #[tokio::test]
    async fn test_get_auto_match_conditions_valid() {
        use crate::services::metadata::preprocessing::{ConditionMode, ConditionOperator};

        let (db, _temp_dir) = create_test_db().await;

        let mut library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Set auto-match conditions
        library.auto_match_conditions = Some(
            r#"{"mode": "all", "rules": [{"field": "book_count", "operator": "gte", "value": 1}]}"#
                .to_string(),
        );
        LibraryRepository::update(db.sea_orm_connection(), &library)
            .await
            .unwrap();

        let conditions = LibraryRepository::get_auto_match_conditions(&library);
        assert!(conditions.is_some());

        let conditions = conditions.unwrap();
        assert_eq!(conditions.mode, ConditionMode::All);
        assert_eq!(conditions.rules.len(), 1);
        assert_eq!(conditions.rules[0].field, "book_count");
        assert_eq!(conditions.rules[0].operator, ConditionOperator::Gte);
    }

    #[tokio::test]
    async fn test_get_auto_match_conditions_invalid_json() {
        let (db, _temp_dir) = create_test_db().await;

        let mut library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Set invalid JSON - should return None
        library.auto_match_conditions = Some("not valid json".to_string());

        let conditions = LibraryRepository::get_auto_match_conditions(&library);
        assert!(conditions.is_none());
    }
}
