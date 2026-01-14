//! Repository for series_external_links table operations
//!
//! TODO: Remove allow(dead_code) when external link features are fully integrated

#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::db::entities::series_external_links::{
    self, Entity as ExternalLinks, Model as ExternalLink,
};

/// Repository for series external link operations
pub struct ExternalLinkRepository;

impl ExternalLinkRepository {
    /// Get an external link by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<ExternalLink>> {
        let result = ExternalLinks::find_by_id(id).one(db).await?;
        Ok(result)
    }

    /// Get all external links for a series
    pub async fn get_for_series(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Vec<ExternalLink>> {
        let results = ExternalLinks::find()
            .filter(series_external_links::Column::SeriesId.eq(series_id))
            .all(db)
            .await?;
        Ok(results)
    }

    /// Get an external link by series ID and source name
    pub async fn get_by_source(
        db: &DatabaseConnection,
        series_id: Uuid,
        source_name: &str,
    ) -> Result<Option<ExternalLink>> {
        let normalized = source_name.to_lowercase().trim().to_string();
        let result = ExternalLinks::find()
            .filter(series_external_links::Column::SeriesId.eq(series_id))
            .filter(series_external_links::Column::SourceName.eq(&normalized))
            .one(db)
            .await?;
        Ok(result)
    }

    /// Create a new external link for a series
    pub async fn create(
        db: &DatabaseConnection,
        series_id: Uuid,
        source_name: &str,
        url: &str,
        external_id: Option<&str>,
    ) -> Result<ExternalLink> {
        let now = Utc::now();
        let normalized_source = source_name.to_lowercase().trim().to_string();

        let active_model = series_external_links::ActiveModel {
            id: Set(Uuid::new_v4()),
            series_id: Set(series_id),
            source_name: Set(normalized_source),
            url: Set(url.trim().to_string()),
            external_id: Set(external_id.map(|s| s.trim().to_string())),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let model = active_model.insert(db).await?;
        Ok(model)
    }

    /// Create or update an external link (upsert by series_id + source_name)
    pub async fn upsert(
        db: &DatabaseConnection,
        series_id: Uuid,
        source_name: &str,
        url: &str,
        external_id: Option<&str>,
    ) -> Result<ExternalLink> {
        let existing = Self::get_by_source(db, series_id, source_name).await?;

        match existing {
            Some(existing) => {
                let mut active_model: series_external_links::ActiveModel = existing.into();
                active_model.url = Set(url.trim().to_string());
                active_model.external_id = Set(external_id.map(|s| s.trim().to_string()));
                active_model.updated_at = Set(Utc::now());

                let model = active_model.update(db).await?;
                Ok(model)
            }
            None => Self::create(db, series_id, source_name, url, external_id).await,
        }
    }

    /// Update an external link by ID
    pub async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        url: Option<&str>,
        external_id: Option<Option<&str>>,
    ) -> Result<Option<ExternalLink>> {
        let existing = ExternalLinks::find_by_id(id).one(db).await?;

        let Some(existing) = existing else {
            return Ok(None);
        };

        let mut active_model: series_external_links::ActiveModel = existing.into();
        active_model.updated_at = Set(Utc::now());

        if let Some(url) = url {
            active_model.url = Set(url.trim().to_string());
        }

        if let Some(external_id) = external_id {
            active_model.external_id = Set(external_id.map(|s| s.trim().to_string()));
        }

        let model = active_model.update(db).await?;
        Ok(Some(model))
    }

    /// Delete an external link by ID
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = ExternalLinks::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Delete an external link by series ID and source name
    pub async fn delete_by_source(
        db: &DatabaseConnection,
        series_id: Uuid,
        source_name: &str,
    ) -> Result<bool> {
        let normalized = source_name.to_lowercase().trim().to_string();
        let result = ExternalLinks::delete_many()
            .filter(series_external_links::Column::SeriesId.eq(series_id))
            .filter(series_external_links::Column::SourceName.eq(&normalized))
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// Delete all external links for a series
    pub async fn delete_all_for_series(db: &DatabaseConnection, series_id: Uuid) -> Result<u64> {
        let result = ExternalLinks::delete_many()
            .filter(series_external_links::Column::SeriesId.eq(series_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Check if an external link belongs to a specific series
    pub async fn belongs_to_series(
        db: &DatabaseConnection,
        id: Uuid,
        series_id: Uuid,
    ) -> Result<bool> {
        let result = ExternalLinks::find_by_id(id)
            .filter(series_external_links::Column::SeriesId.eq(series_id))
            .one(db)
            .await?;
        Ok(result.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::{LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;
    use crate::db::ScanningStrategy;

    #[tokio::test]
    async fn test_create_and_get_external_link() {
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

        let link = ExternalLinkRepository::create(
            db.sea_orm_connection(),
            series.id,
            "myanimelist",
            "https://myanimelist.net/manga/12345",
            Some("12345"),
        )
        .await
        .unwrap();

        assert_eq!(link.source_name, "myanimelist");
        assert_eq!(link.url, "https://myanimelist.net/manga/12345");
        assert_eq!(link.external_id, Some("12345".to_string()));
        assert_eq!(link.series_id, series.id);

        let fetched = ExternalLinkRepository::get_by_id(db.sea_orm_connection(), link.id)
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

        ExternalLinkRepository::create(
            db.sea_orm_connection(),
            series.id,
            "myanimelist",
            "https://mal.net/1",
            Some("1"),
        )
        .await
        .unwrap();

        ExternalLinkRepository::create(
            db.sea_orm_connection(),
            series.id,
            "anilist",
            "https://anilist.co/2",
            Some("2"),
        )
        .await
        .unwrap();

        ExternalLinkRepository::create(
            db.sea_orm_connection(),
            series.id,
            "mangadex",
            "https://mangadex.org/3",
            None,
        )
        .await
        .unwrap();

        let links = ExternalLinkRepository::get_for_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();

        assert_eq!(links.len(), 3);
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

        ExternalLinkRepository::create(
            db.sea_orm_connection(),
            series.id,
            "myanimelist",
            "https://mal.net/1",
            Some("1"),
        )
        .await
        .unwrap();

        let link = ExternalLinkRepository::get_by_source(
            db.sea_orm_connection(),
            series.id,
            "MyAnimeList",
        )
        .await
        .unwrap();

        assert!(link.is_some());
        assert_eq!(link.unwrap().source_name, "myanimelist");

        let not_found =
            ExternalLinkRepository::get_by_source(db.sea_orm_connection(), series.id, "anilist")
                .await
                .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_upsert_external_link() {
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
        let link1 = ExternalLinkRepository::upsert(
            db.sea_orm_connection(),
            series.id,
            "anilist",
            "https://anilist.co/old",
            Some("old-id"),
        )
        .await
        .unwrap();

        assert_eq!(link1.url, "https://anilist.co/old");

        // Second upsert updates
        let link2 = ExternalLinkRepository::upsert(
            db.sea_orm_connection(),
            series.id,
            "anilist",
            "https://anilist.co/new",
            Some("new-id"),
        )
        .await
        .unwrap();

        assert_eq!(link1.id, link2.id);
        assert_eq!(link2.url, "https://anilist.co/new");
        assert_eq!(link2.external_id, Some("new-id".to_string()));

        // Verify only one link exists
        let links = ExternalLinkRepository::get_for_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert_eq!(links.len(), 1);
    }

    #[tokio::test]
    async fn test_update_external_link() {
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

        let link = ExternalLinkRepository::create(
            db.sea_orm_connection(),
            series.id,
            "anilist",
            "https://old.url",
            Some("old-id"),
        )
        .await
        .unwrap();

        // Update URL only
        let updated = ExternalLinkRepository::update(
            db.sea_orm_connection(),
            link.id,
            Some("https://new.url"),
            None,
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(updated.url, "https://new.url");
        assert_eq!(updated.external_id, Some("old-id".to_string()));

        // Update external_id only
        let updated = ExternalLinkRepository::update(
            db.sea_orm_connection(),
            link.id,
            None,
            Some(Some("new-id")),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(updated.url, "https://new.url");
        assert_eq!(updated.external_id, Some("new-id".to_string()));

        // Set external_id to None
        let updated =
            ExternalLinkRepository::update(db.sea_orm_connection(), link.id, None, Some(None))
                .await
                .unwrap()
                .unwrap();

        assert_eq!(updated.external_id, None);
    }

    #[tokio::test]
    async fn test_delete_external_link() {
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

        let link = ExternalLinkRepository::create(
            db.sea_orm_connection(),
            series.id,
            "anilist",
            "https://anilist.co/1",
            None,
        )
        .await
        .unwrap();

        let deleted = ExternalLinkRepository::delete(db.sea_orm_connection(), link.id)
            .await
            .unwrap();
        assert!(deleted);

        let fetched = ExternalLinkRepository::get_by_id(db.sea_orm_connection(), link.id)
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

        ExternalLinkRepository::create(
            db.sea_orm_connection(),
            series.id,
            "anilist",
            "https://anilist.co/1",
            None,
        )
        .await
        .unwrap();

        let deleted =
            ExternalLinkRepository::delete_by_source(db.sea_orm_connection(), series.id, "AniList")
                .await
                .unwrap();
        assert!(deleted);

        let fetched =
            ExternalLinkRepository::get_by_source(db.sea_orm_connection(), series.id, "anilist")
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

        for source in ["mal", "anilist", "mangadex"] {
            ExternalLinkRepository::create(
                db.sea_orm_connection(),
                series.id,
                source,
                &format!("https://{}.com", source),
                None,
            )
            .await
            .unwrap();
        }

        let count =
            ExternalLinkRepository::delete_all_for_series(db.sea_orm_connection(), series.id)
                .await
                .unwrap();

        assert_eq!(count, 3);

        let remaining = ExternalLinkRepository::get_for_series(db.sea_orm_connection(), series.id)
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

        let link = ExternalLinkRepository::create(
            db.sea_orm_connection(),
            series.id,
            "  MyAnimeList  ",
            "https://mal.net/1",
            None,
        )
        .await
        .unwrap();

        assert_eq!(link.source_name, "myanimelist");

        // Should find with different case
        let found = ExternalLinkRepository::get_by_source(
            db.sea_orm_connection(),
            series.id,
            "MYANIMELIST",
        )
        .await
        .unwrap();
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn test_url_and_external_id_trimming() {
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

        let link = ExternalLinkRepository::create(
            db.sea_orm_connection(),
            series.id,
            "mal",
            "  https://mal.net/1  ",
            Some("  12345  "),
        )
        .await
        .unwrap();

        assert_eq!(link.url, "https://mal.net/1");
        assert_eq!(link.external_id, Some("12345".to_string()));
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

        let link = ExternalLinkRepository::create(
            db.sea_orm_connection(),
            series1.id,
            "mal",
            "https://mal.net/1",
            None,
        )
        .await
        .unwrap();

        let belongs =
            ExternalLinkRepository::belongs_to_series(db.sea_orm_connection(), link.id, series1.id)
                .await
                .unwrap();
        assert!(belongs);

        let belongs =
            ExternalLinkRepository::belongs_to_series(db.sea_orm_connection(), link.id, series2.id)
                .await
                .unwrap();
        assert!(!belongs);
    }
}
