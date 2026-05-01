//! Repository for the `series_aliases` table.
//!
//! Title aliases used by release-source plugins to match incoming release
//! titles against tracked series when an external ID isn't available (e.g.
//! Nyaa). Distinct from `alternate_title.rs` which manages localized titles
//! with labels.

#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::db::entities::series_aliases::{
    self, Entity as SeriesAliases, Model as SeriesAlias, alias_source, normalize_alias,
};

pub struct SeriesAliasRepository;

impl SeriesAliasRepository {
    /// Get an alias row by id.
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<SeriesAlias>> {
        Ok(SeriesAliases::find_by_id(id).one(db).await?)
    }

    /// Get all aliases for a series, ordered by alias for stable display.
    pub async fn get_for_series(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Vec<SeriesAlias>> {
        use sea_orm::QueryOrder;
        let results = SeriesAliases::find()
            .filter(series_aliases::Column::SeriesId.eq(series_id))
            .order_by_asc(series_aliases::Column::Alias)
            .all(db)
            .await?;
        Ok(results)
    }

    /// Bulk-fetch aliases for many series, returned as a HashMap keyed by series_id.
    pub async fn get_for_series_ids(
        db: &DatabaseConnection,
        series_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<SeriesAlias>>> {
        if series_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let results = SeriesAliases::find()
            .filter(series_aliases::Column::SeriesId.is_in(series_ids.to_vec()))
            .all(db)
            .await?;
        let mut map: HashMap<Uuid, Vec<SeriesAlias>> = HashMap::new();
        for row in results {
            map.entry(row.series_id).or_default().push(row);
        }
        Ok(map)
    }

    /// Find every series whose normalized alias equals `normalized`.
    /// Returns rows so the caller can reach `series_id` and the original alias.
    pub async fn find_by_normalized(
        db: &DatabaseConnection,
        normalized: &str,
    ) -> Result<Vec<SeriesAlias>> {
        Ok(SeriesAliases::find()
            .filter(series_aliases::Column::Normalized.eq(normalized))
            .all(db)
            .await?)
    }

    /// Create an alias. Returns the existing row if `(series_id, alias)`
    /// already exists - aliases are idempotent on add.
    pub async fn create(
        db: &DatabaseConnection,
        series_id: Uuid,
        alias: &str,
        source: &str,
    ) -> Result<SeriesAlias> {
        if !alias_source::is_valid(source) {
            anyhow::bail!("invalid alias source: {}", source);
        }
        let trimmed = alias.trim();
        if trimmed.is_empty() {
            anyhow::bail!("alias cannot be empty");
        }

        // Idempotent on (series_id, alias).
        if let Some(existing) = SeriesAliases::find()
            .filter(series_aliases::Column::SeriesId.eq(series_id))
            .filter(series_aliases::Column::Alias.eq(trimmed))
            .one(db)
            .await?
        {
            return Ok(existing);
        }

        let normalized = normalize_alias(trimmed);
        if normalized.is_empty() {
            anyhow::bail!("alias normalizes to empty string");
        }

        let active = series_aliases::ActiveModel {
            id: Set(Uuid::new_v4()),
            series_id: Set(series_id),
            alias: Set(trimmed.to_string()),
            normalized: Set(normalized),
            source: Set(source.to_string()),
            created_at: Set(Utc::now()),
        };
        Ok(active.insert(db).await?)
    }

    /// Bulk-insert aliases for a series. Existing aliases (by normalized text)
    /// are skipped. Returns the number of newly inserted rows.
    pub async fn bulk_create(
        db: &DatabaseConnection,
        series_id: Uuid,
        aliases: &[&str],
        source: &str,
    ) -> Result<usize> {
        if !alias_source::is_valid(source) {
            anyhow::bail!("invalid alias source: {}", source);
        }
        let mut inserted = 0;
        for alias in aliases {
            // Skip blanks defensively; create() also checks but a noisy upstream
            // shouldn't cause a hard error here.
            if alias.trim().is_empty() {
                continue;
            }
            // create() is idempotent; we count only true inserts by checking before/after.
            let before = Self::count_for_series_with_alias(db, series_id, alias.trim()).await?;
            Self::create(db, series_id, alias, source).await?;
            let after = Self::count_for_series_with_alias(db, series_id, alias.trim()).await?;
            if after > before {
                inserted += 1;
            }
        }
        Ok(inserted)
    }

    async fn count_for_series_with_alias(
        db: &DatabaseConnection,
        series_id: Uuid,
        alias: &str,
    ) -> Result<u64> {
        let count = SeriesAliases::find()
            .filter(series_aliases::Column::SeriesId.eq(series_id))
            .filter(series_aliases::Column::Alias.eq(alias))
            .count(db)
            .await?;
        Ok(count)
    }

    /// Delete an alias by id. Returns true if a row was removed.
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = SeriesAliases::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Delete all aliases from a given source for a series. Useful for
    /// "refresh metadata-sourced aliases" without touching manual aliases.
    pub async fn delete_by_source_for_series(
        db: &DatabaseConnection,
        series_id: Uuid,
        source: &str,
    ) -> Result<u64> {
        let result = SeriesAliases::delete_many()
            .filter(series_aliases::Column::SeriesId.eq(series_id))
            .filter(series_aliases::Column::Source.eq(source))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Delete all aliases for a series (independent of cascade).
    pub async fn delete_all_for_series(db: &DatabaseConnection, series_id: Uuid) -> Result<u64> {
        let result = SeriesAliases::delete_many()
            .filter(series_aliases::Column::SeriesId.eq(series_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Count aliases for a series.
    pub async fn count_for_series(db: &DatabaseConnection, series_id: Uuid) -> Result<u64> {
        let count = SeriesAliases::find()
            .filter(series_aliases::Column::SeriesId.eq(series_id))
            .count(db)
            .await?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::repositories::{LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;

    async fn make_two_series(db: &DatabaseConnection) -> (Uuid, Uuid) {
        let library = LibraryRepository::create(db, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let s1 = SeriesRepository::create(db, library.id, "Series 1", None)
            .await
            .unwrap();
        let s2 = SeriesRepository::create(db, library.id, "Series 2", None)
            .await
            .unwrap();
        (s1.id, s2.id)
    }

    #[tokio::test]
    async fn create_inserts_with_normalized_form() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (s1, _) = make_two_series(conn).await;

        let row = SeriesAliasRepository::create(conn, s1, "My Hero Academia!", "manual")
            .await
            .unwrap();
        assert_eq!(row.alias, "My Hero Academia!");
        assert_eq!(row.normalized, "my hero academia");
        assert_eq!(row.source, "manual");
    }

    #[tokio::test]
    async fn create_is_idempotent_per_series() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (s1, _) = make_two_series(conn).await;

        let r1 = SeriesAliasRepository::create(conn, s1, "Boku no Hero", "manual")
            .await
            .unwrap();
        let r2 = SeriesAliasRepository::create(conn, s1, "Boku no Hero", "manual")
            .await
            .unwrap();
        assert_eq!(r1.id, r2.id, "same alias on same series returns same row");

        let count = SeriesAliasRepository::count_for_series(conn, s1)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn same_alias_allowed_on_different_series() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (s1, s2) = make_two_series(conn).await;

        let a = SeriesAliasRepository::create(conn, s1, "Common Title", "metadata")
            .await
            .unwrap();
        let b = SeriesAliasRepository::create(conn, s2, "Common Title", "metadata")
            .await
            .unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(a.normalized, b.normalized);
    }

    #[tokio::test]
    async fn create_rejects_blank_or_punctuation_only() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (s1, _) = make_two_series(conn).await;

        let err = SeriesAliasRepository::create(conn, s1, "   ", "manual")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("empty"));

        let err = SeriesAliasRepository::create(conn, s1, "!!!---!!!", "manual")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("normalize"));
    }

    #[tokio::test]
    async fn create_rejects_invalid_source() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (s1, _) = make_two_series(conn).await;

        let err = SeriesAliasRepository::create(conn, s1, "X", "auto")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("invalid alias source"));
    }

    #[tokio::test]
    async fn find_by_normalized_returns_all_matches() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (s1, s2) = make_two_series(conn).await;

        SeriesAliasRepository::create(conn, s1, "My Series", "manual")
            .await
            .unwrap();
        SeriesAliasRepository::create(conn, s2, "MY SERIES!", "metadata")
            .await
            .unwrap();
        SeriesAliasRepository::create(conn, s1, "Other Title", "manual")
            .await
            .unwrap();

        let matches = SeriesAliasRepository::find_by_normalized(conn, "my series")
            .await
            .unwrap();
        assert_eq!(matches.len(), 2, "both series share normalized 'my series'");
        let mut series_ids: Vec<Uuid> = matches.into_iter().map(|m| m.series_id).collect();
        series_ids.sort();
        let mut expected = [s1, s2];
        expected.sort();
        assert_eq!(series_ids, expected);
    }

    #[tokio::test]
    async fn bulk_create_dedups_and_counts_inserts() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (s1, _) = make_two_series(conn).await;

        let inserted = SeriesAliasRepository::bulk_create(
            conn,
            s1,
            &["Title A", "Title B", "Title A", ""],
            "metadata",
        )
        .await
        .unwrap();
        assert_eq!(inserted, 2, "blank skipped, duplicate dedup'd");

        let again =
            SeriesAliasRepository::bulk_create(conn, s1, &["Title A", "Title C"], "metadata")
                .await
                .unwrap();
        assert_eq!(again, 1, "Title A already present, only Title C is new");

        let count = SeriesAliasRepository::count_for_series(conn, s1)
            .await
            .unwrap();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn delete_by_source_only_touches_that_source() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (s1, _) = make_two_series(conn).await;

        SeriesAliasRepository::create(conn, s1, "Manual One", "manual")
            .await
            .unwrap();
        SeriesAliasRepository::create(conn, s1, "Meta One", "metadata")
            .await
            .unwrap();
        SeriesAliasRepository::create(conn, s1, "Meta Two", "metadata")
            .await
            .unwrap();

        let removed = SeriesAliasRepository::delete_by_source_for_series(conn, s1, "metadata")
            .await
            .unwrap();
        assert_eq!(removed, 2);

        let remaining = SeriesAliasRepository::get_for_series(conn, s1)
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].source, "manual");
    }

    #[tokio::test]
    async fn cascade_deletes_aliases_when_series_deleted() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (s1, _) = make_two_series(conn).await;

        SeriesAliasRepository::create(conn, s1, "Will Be Cascaded", "manual")
            .await
            .unwrap();
        SeriesRepository::delete(conn, s1).await.unwrap();

        let after = SeriesAliasRepository::get_for_series(conn, s1)
            .await
            .unwrap();
        assert!(after.is_empty());
    }

    #[tokio::test]
    async fn get_for_series_ids_handles_empty_input() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let map = SeriesAliasRepository::get_for_series_ids(conn, &[])
            .await
            .unwrap();
        assert!(map.is_empty());
    }
}
