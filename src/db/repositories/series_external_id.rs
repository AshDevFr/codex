//! Repository for series_external_ids table operations
//!
//! Provides CRUD operations for managing external provider IDs for series.
//! Used by the plugin auto-match system to track which external source a series
//! was matched from and enable efficient re-fetching.

#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::db::entities::series_external_ids::{
    self, Entity as SeriesExternalIds, Model as SeriesExternalId,
};

/// Repository for series external ID operations
pub struct SeriesExternalIdRepository;

impl SeriesExternalIdRepository {
    /// Get an external ID record by its primary key
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<SeriesExternalId>> {
        let result = SeriesExternalIds::find_by_id(id).one(db).await?;
        Ok(result)
    }

    /// Get all external IDs for a series
    pub async fn get_for_series(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Vec<SeriesExternalId>> {
        let results = SeriesExternalIds::find()
            .filter(series_external_ids::Column::SeriesId.eq(series_id))
            .all(db)
            .await?;
        Ok(results)
    }

    /// Get an external ID by series ID and source
    pub async fn get_by_source(
        db: &DatabaseConnection,
        series_id: Uuid,
        source: &str,
    ) -> Result<Option<SeriesExternalId>> {
        let result = SeriesExternalIds::find()
            .filter(series_external_ids::Column::SeriesId.eq(series_id))
            .filter(series_external_ids::Column::Source.eq(source))
            .one(db)
            .await?;
        Ok(result)
    }

    /// Get an external ID for a series from a specific plugin
    pub async fn get_for_plugin(
        db: &DatabaseConnection,
        series_id: Uuid,
        plugin_name: &str,
    ) -> Result<Option<SeriesExternalId>> {
        let source = series_external_ids::Model::plugin_source(plugin_name);
        Self::get_by_source(db, series_id, &source).await
    }

    /// Create a new external ID record
    pub async fn create(
        db: &DatabaseConnection,
        series_id: Uuid,
        source: &str,
        external_id: &str,
        external_url: Option<&str>,
        metadata_hash: Option<&str>,
    ) -> Result<SeriesExternalId> {
        let now = Utc::now();

        let active_model = series_external_ids::ActiveModel {
            id: Set(Uuid::new_v4()),
            series_id: Set(series_id),
            source: Set(source.to_string()),
            external_id: Set(external_id.to_string()),
            external_url: Set(external_url.map(|s| s.to_string())),
            metadata_hash: Set(metadata_hash.map(|s| s.to_string())),
            last_synced_at: Set(Some(now)),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let model = active_model.insert(db).await?;
        Ok(model)
    }

    /// Create an external ID record for a plugin source
    pub async fn create_for_plugin(
        db: &DatabaseConnection,
        series_id: Uuid,
        plugin_name: &str,
        external_id: &str,
        external_url: Option<&str>,
        metadata_hash: Option<&str>,
    ) -> Result<SeriesExternalId> {
        let source = series_external_ids::Model::plugin_source(plugin_name);
        Self::create(
            db,
            series_id,
            &source,
            external_id,
            external_url,
            metadata_hash,
        )
        .await
    }

    /// Create or update an external ID record (upsert by series_id + source)
    pub async fn upsert(
        db: &DatabaseConnection,
        series_id: Uuid,
        source: &str,
        external_id: &str,
        external_url: Option<&str>,
        metadata_hash: Option<&str>,
    ) -> Result<SeriesExternalId> {
        let existing = Self::get_by_source(db, series_id, source).await?;

        match existing {
            Some(existing) => {
                let now = Utc::now();
                let mut active_model: series_external_ids::ActiveModel = existing.into();
                active_model.external_id = Set(external_id.to_string());
                active_model.external_url = Set(external_url.map(|s| s.to_string()));
                active_model.metadata_hash = Set(metadata_hash.map(|s| s.to_string()));
                active_model.last_synced_at = Set(Some(now));
                active_model.updated_at = Set(now);

                let model = active_model.update(db).await?;
                Ok(model)
            }
            None => {
                Self::create(
                    db,
                    series_id,
                    source,
                    external_id,
                    external_url,
                    metadata_hash,
                )
                .await
            }
        }
    }

    /// Upsert an external ID for a plugin source
    pub async fn upsert_for_plugin(
        db: &DatabaseConnection,
        series_id: Uuid,
        plugin_name: &str,
        external_id: &str,
        external_url: Option<&str>,
        metadata_hash: Option<&str>,
    ) -> Result<SeriesExternalId> {
        let source = series_external_ids::Model::plugin_source(plugin_name);
        Self::upsert(
            db,
            series_id,
            &source,
            external_id,
            external_url,
            metadata_hash,
        )
        .await
    }

    /// Update the metadata hash and last synced timestamp
    pub async fn update_sync_info(
        db: &DatabaseConnection,
        id: Uuid,
        metadata_hash: Option<&str>,
    ) -> Result<Option<SeriesExternalId>> {
        let existing = SeriesExternalIds::find_by_id(id).one(db).await?;

        let Some(existing) = existing else {
            return Ok(None);
        };

        let now = Utc::now();
        let mut active_model: series_external_ids::ActiveModel = existing.into();
        active_model.metadata_hash = Set(metadata_hash.map(|s| s.to_string()));
        active_model.last_synced_at = Set(Some(now));
        active_model.updated_at = Set(now);

        let model = active_model.update(db).await?;
        Ok(Some(model))
    }

    /// Delete an external ID record by ID
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = SeriesExternalIds::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Delete an external ID by series ID and source
    pub async fn delete_by_source(
        db: &DatabaseConnection,
        series_id: Uuid,
        source: &str,
    ) -> Result<bool> {
        let result = SeriesExternalIds::delete_many()
            .filter(series_external_ids::Column::SeriesId.eq(series_id))
            .filter(series_external_ids::Column::Source.eq(source))
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// Delete all external IDs for a series
    pub async fn delete_all_for_series(db: &DatabaseConnection, series_id: Uuid) -> Result<u64> {
        let result = SeriesExternalIds::delete_many()
            .filter(series_external_ids::Column::SeriesId.eq(series_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Get external IDs for multiple series by their IDs
    ///
    /// Returns a HashMap keyed by series_id for efficient lookups
    pub async fn get_for_series_ids(
        db: &DatabaseConnection,
        series_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<SeriesExternalId>>> {
        if series_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let results = SeriesExternalIds::find()
            .filter(series_external_ids::Column::SeriesId.is_in(series_ids.to_vec()))
            .all(db)
            .await?;

        let mut map: HashMap<Uuid, Vec<SeriesExternalId>> = HashMap::new();

        for external_id in results {
            map.entry(external_id.series_id)
                .or_default()
                .push(external_id);
        }

        Ok(map)
    }

    /// Find series external IDs by multiple external ID values and source
    ///
    /// Returns a HashMap keyed by external_id for efficient reverse lookups.
    /// Used during pull sync to batch-match pulled entries to Codex series.
    pub async fn find_by_external_ids_and_source(
        db: &DatabaseConnection,
        external_ids: &[String],
        source: &str,
    ) -> Result<HashMap<String, SeriesExternalId>> {
        if external_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let results = SeriesExternalIds::find()
            .filter(series_external_ids::Column::ExternalId.is_in(external_ids.to_vec()))
            .filter(series_external_ids::Column::Source.eq(source))
            .all(db)
            .await?;

        Ok(results
            .into_iter()
            .map(|e| (e.external_id.clone(), e))
            .collect())
    }

    /// Check if an external ID record belongs to a specific series
    pub async fn belongs_to_series(
        db: &DatabaseConnection,
        id: Uuid,
        series_id: Uuid,
    ) -> Result<bool> {
        let result = SeriesExternalIds::find_by_id(id)
            .filter(series_external_ids::Column::SeriesId.eq(series_id))
            .one(db)
            .await?;
        Ok(result.is_some())
    }

    /// Count external IDs for a series
    pub async fn count_for_series(db: &DatabaseConnection, series_id: Uuid) -> Result<u64> {
        let count = SeriesExternalIds::find()
            .filter(series_external_ids::Column::SeriesId.eq(series_id))
            .count(db)
            .await?;
        Ok(count)
    }

    /// Find all series with a specific external source
    pub async fn find_by_source(
        db: &DatabaseConnection,
        source: &str,
    ) -> Result<Vec<SeriesExternalId>> {
        let results = SeriesExternalIds::find()
            .filter(series_external_ids::Column::Source.eq(source))
            .all(db)
            .await?;
        Ok(results)
    }

    /// Find a series external ID by external ID value and source
    ///
    /// Used for reverse lookups, e.g. finding which Codex series has a given
    /// AniList media ID (`source = "api:anilist"`, `external_id = "12345"`).
    pub async fn find_by_external_id_and_source(
        db: &DatabaseConnection,
        external_id: &str,
        source: &str,
    ) -> Result<Option<SeriesExternalId>> {
        let result = SeriesExternalIds::find()
            .filter(series_external_ids::Column::ExternalId.eq(external_id))
            .filter(series_external_ids::Column::Source.eq(source))
            .one(db)
            .await?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::repositories::{LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;

    #[tokio::test]
    async fn test_create_and_get_external_id() {
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

        let external = SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "plugin:mangabaka",
            "12345",
            Some("https://mangabaka.com/manga/12345"),
            Some("abc123hash"),
        )
        .await
        .unwrap();

        assert_eq!(external.source, "plugin:mangabaka");
        assert_eq!(external.external_id, "12345");
        assert_eq!(
            external.external_url,
            Some("https://mangabaka.com/manga/12345".to_string())
        );
        assert_eq!(external.metadata_hash, Some("abc123hash".to_string()));
        assert!(external.last_synced_at.is_some());
        assert_eq!(external.series_id, series.id);

        let fetched = SeriesExternalIdRepository::get_by_id(db.sea_orm_connection(), external.id)
            .await
            .unwrap();
        assert!(fetched.is_some());
    }

    #[tokio::test]
    async fn test_create_for_plugin() {
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

        let external = SeriesExternalIdRepository::create_for_plugin(
            db.sea_orm_connection(),
            series.id,
            "mangabaka",
            "12345",
            None,
            None,
        )
        .await
        .unwrap();

        assert_eq!(external.source, "plugin:mangabaka");
        assert!(external.is_plugin_source());
        assert_eq!(external.plugin_name(), Some("mangabaka"));
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

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "plugin:mangabaka",
            "1",
            None,
            None,
        )
        .await
        .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "comicinfo",
            "2",
            None,
            None,
        )
        .await
        .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "manual",
            "3",
            None,
            None,
        )
        .await
        .unwrap();

        let externals =
            SeriesExternalIdRepository::get_for_series(db.sea_orm_connection(), series.id)
                .await
                .unwrap();

        assert_eq!(externals.len(), 3);
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

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "plugin:mangabaka",
            "12345",
            None,
            None,
        )
        .await
        .unwrap();

        let found = SeriesExternalIdRepository::get_by_source(
            db.sea_orm_connection(),
            series.id,
            "plugin:mangabaka",
        )
        .await
        .unwrap();

        assert!(found.is_some());
        assert_eq!(found.unwrap().external_id, "12345");

        let not_found = SeriesExternalIdRepository::get_by_source(
            db.sea_orm_connection(),
            series.id,
            "plugin:other",
        )
        .await
        .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_get_for_plugin() {
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

        SeriesExternalIdRepository::create_for_plugin(
            db.sea_orm_connection(),
            series.id,
            "mangabaka",
            "12345",
            None,
            None,
        )
        .await
        .unwrap();

        let found = SeriesExternalIdRepository::get_for_plugin(
            db.sea_orm_connection(),
            series.id,
            "mangabaka",
        )
        .await
        .unwrap();

        assert!(found.is_some());
        assert_eq!(found.unwrap().external_id, "12345");
    }

    #[tokio::test]
    async fn test_upsert_external_id() {
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
        let external1 = SeriesExternalIdRepository::upsert(
            db.sea_orm_connection(),
            series.id,
            "plugin:mangabaka",
            "old-id",
            Some("https://old.url"),
            Some("old-hash"),
        )
        .await
        .unwrap();

        assert_eq!(external1.external_id, "old-id");

        // Second upsert updates
        let external2 = SeriesExternalIdRepository::upsert(
            db.sea_orm_connection(),
            series.id,
            "plugin:mangabaka",
            "new-id",
            Some("https://new.url"),
            Some("new-hash"),
        )
        .await
        .unwrap();

        assert_eq!(external1.id, external2.id);
        assert_eq!(external2.external_id, "new-id");
        assert_eq!(external2.external_url, Some("https://new.url".to_string()));
        assert_eq!(external2.metadata_hash, Some("new-hash".to_string()));

        // Verify only one record exists
        let externals =
            SeriesExternalIdRepository::get_for_series(db.sea_orm_connection(), series.id)
                .await
                .unwrap();
        assert_eq!(externals.len(), 1);
    }

    #[tokio::test]
    async fn test_update_sync_info() {
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

        let external = SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "plugin:mangabaka",
            "12345",
            None,
            Some("old-hash"),
        )
        .await
        .unwrap();

        let old_synced = external.last_synced_at;

        // Wait a tiny bit to ensure timestamp changes
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let updated = SeriesExternalIdRepository::update_sync_info(
            db.sea_orm_connection(),
            external.id,
            Some("new-hash"),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(updated.metadata_hash, Some("new-hash".to_string()));
        assert!(updated.last_synced_at > old_synced);
    }

    #[tokio::test]
    async fn test_delete_external_id() {
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

        let external = SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "plugin:mangabaka",
            "12345",
            None,
            None,
        )
        .await
        .unwrap();

        let deleted = SeriesExternalIdRepository::delete(db.sea_orm_connection(), external.id)
            .await
            .unwrap();
        assert!(deleted);

        let fetched = SeriesExternalIdRepository::get_by_id(db.sea_orm_connection(), external.id)
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

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "plugin:mangabaka",
            "12345",
            None,
            None,
        )
        .await
        .unwrap();

        let deleted = SeriesExternalIdRepository::delete_by_source(
            db.sea_orm_connection(),
            series.id,
            "plugin:mangabaka",
        )
        .await
        .unwrap();
        assert!(deleted);

        let fetched = SeriesExternalIdRepository::get_by_source(
            db.sea_orm_connection(),
            series.id,
            "plugin:mangabaka",
        )
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

        for source in ["plugin:mangabaka", "comicinfo", "manual"] {
            SeriesExternalIdRepository::create(
                db.sea_orm_connection(),
                series.id,
                source,
                "12345",
                None,
                None,
            )
            .await
            .unwrap();
        }

        let count =
            SeriesExternalIdRepository::delete_all_for_series(db.sea_orm_connection(), series.id)
                .await
                .unwrap();

        assert_eq!(count, 3);

        let remaining =
            SeriesExternalIdRepository::get_for_series(db.sea_orm_connection(), series.id)
                .await
                .unwrap();
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn test_count_for_series() {
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

        let count =
            SeriesExternalIdRepository::count_for_series(db.sea_orm_connection(), series.id)
                .await
                .unwrap();
        assert_eq!(count, 0);

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "plugin:mangabaka",
            "1",
            None,
            None,
        )
        .await
        .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "comicinfo",
            "2",
            None,
            None,
        )
        .await
        .unwrap();

        let count =
            SeriesExternalIdRepository::count_for_series(db.sea_orm_connection(), series.id)
                .await
                .unwrap();
        assert_eq!(count, 2);
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

        let external = SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series1.id,
            "plugin:mangabaka",
            "12345",
            None,
            None,
        )
        .await
        .unwrap();

        let belongs = SeriesExternalIdRepository::belongs_to_series(
            db.sea_orm_connection(),
            external.id,
            series1.id,
        )
        .await
        .unwrap();
        assert!(belongs);

        let belongs = SeriesExternalIdRepository::belongs_to_series(
            db.sea_orm_connection(),
            external.id,
            series2.id,
        )
        .await
        .unwrap();
        assert!(!belongs);
    }

    #[tokio::test]
    async fn test_get_for_series_ids() {
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

        let series3 =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 3", None)
                .await
                .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series1.id,
            "plugin:mangabaka",
            "1",
            None,
            None,
        )
        .await
        .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series1.id,
            "comicinfo",
            "1a",
            None,
            None,
        )
        .await
        .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series2.id,
            "plugin:mangabaka",
            "2",
            None,
            None,
        )
        .await
        .unwrap();

        let map = SeriesExternalIdRepository::get_for_series_ids(
            db.sea_orm_connection(),
            &[series1.id, series2.id, series3.id],
        )
        .await
        .unwrap();

        assert_eq!(map.len(), 2); // series3 has no external IDs
        assert_eq!(map.get(&series1.id).unwrap().len(), 2);
        assert_eq!(map.get(&series2.id).unwrap().len(), 1);
        assert!(!map.contains_key(&series3.id));
    }

    #[tokio::test]
    async fn test_find_by_source() {
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

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series1.id,
            "plugin:mangabaka",
            "1",
            None,
            None,
        )
        .await
        .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series2.id,
            "plugin:mangabaka",
            "2",
            None,
            None,
        )
        .await
        .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series1.id,
            "comicinfo",
            "1a",
            None,
            None,
        )
        .await
        .unwrap();

        let mangabaka_ids =
            SeriesExternalIdRepository::find_by_source(db.sea_orm_connection(), "plugin:mangabaka")
                .await
                .unwrap();

        assert_eq!(mangabaka_ids.len(), 2);

        let comicinfo_ids =
            SeriesExternalIdRepository::find_by_source(db.sea_orm_connection(), "comicinfo")
                .await
                .unwrap();

        assert_eq!(comicinfo_ids.len(), 1);
    }

    #[tokio::test]
    async fn test_find_by_external_id_and_source() {
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

        // Create api:anilist external IDs for both series
        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series1.id,
            "api:anilist",
            "12345",
            None,
            None,
        )
        .await
        .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series2.id,
            "api:anilist",
            "67890",
            None,
            None,
        )
        .await
        .unwrap();

        // Also create a different source with the same external ID
        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series1.id,
            "api:myanimelist",
            "12345",
            None,
            None,
        )
        .await
        .unwrap();

        // Find by anilist ID
        let found = SeriesExternalIdRepository::find_by_external_id_and_source(
            db.sea_orm_connection(),
            "12345",
            "api:anilist",
        )
        .await
        .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().series_id, series1.id);

        // Find different anilist ID
        let found = SeriesExternalIdRepository::find_by_external_id_and_source(
            db.sea_orm_connection(),
            "67890",
            "api:anilist",
        )
        .await
        .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().series_id, series2.id);

        // Non-existent external ID returns None
        let not_found = SeriesExternalIdRepository::find_by_external_id_and_source(
            db.sea_orm_connection(),
            "99999",
            "api:anilist",
        )
        .await
        .unwrap();
        assert!(not_found.is_none());

        // Same external ID but different source returns correct result
        let found_mal = SeriesExternalIdRepository::find_by_external_id_and_source(
            db.sea_orm_connection(),
            "12345",
            "api:myanimelist",
        )
        .await
        .unwrap();
        assert!(found_mal.is_some());
        assert_eq!(found_mal.unwrap().series_id, series1.id);
    }

    #[tokio::test]
    async fn test_find_by_external_ids_and_source_empty_input() {
        let (db, _temp_dir) = create_test_db().await;

        let result = SeriesExternalIdRepository::find_by_external_ids_and_source(
            db.sea_orm_connection(),
            &[],
            "plugin:mangabaka",
        )
        .await
        .unwrap();

        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_find_by_external_ids_and_source_multiple_ids() {
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

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series1.id,
            "api:anilist",
            "ext_1",
            None,
            None,
        )
        .await
        .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series2.id,
            "api:anilist",
            "ext_2",
            None,
            None,
        )
        .await
        .unwrap();

        let result = SeriesExternalIdRepository::find_by_external_ids_and_source(
            db.sea_orm_connection(),
            &["ext_1".to_string(), "ext_2".to_string()],
            "api:anilist",
        )
        .await
        .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result.get("ext_1").unwrap().series_id, series1.id);
        assert_eq!(result.get("ext_2").unwrap().series_id, series2.id);
    }

    #[tokio::test]
    async fn test_find_by_external_ids_and_source_filters_by_source() {
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

        // Same external_id but different sources
        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "12345",
            None,
            None,
        )
        .await
        .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:myanimelist",
            "12345",
            None,
            None,
        )
        .await
        .unwrap();

        let result = SeriesExternalIdRepository::find_by_external_ids_and_source(
            db.sea_orm_connection(),
            &["12345".to_string()],
            "api:anilist",
        )
        .await
        .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result.get("12345").unwrap().source, "api:anilist");
    }

    #[tokio::test]
    async fn test_find_by_external_ids_and_source_partial_match() {
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

        // Only one external ID exists
        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "111",
            None,
            None,
        )
        .await
        .unwrap();

        // Query for multiple IDs — only one should match
        let result = SeriesExternalIdRepository::find_by_external_ids_and_source(
            db.sea_orm_connection(),
            &["111".to_string(), "222".to_string(), "333".to_string()],
            "api:anilist",
        )
        .await
        .unwrap();

        assert_eq!(result.len(), 1);
        assert!(result.contains_key("111"));
        assert!(!result.contains_key("222"));
    }
}
