//! Repository for SeriesDuplicates operations
//!
//! Rebuilds the `series_duplicates` table from current series state using two
//! detection passes:
//!
//! 1. **External-ID pass**: groups series by `(source, external_id)` in
//!    `series_external_ids`. High-confidence matches; not scoped to library.
//! 2. **Title pass**: groups series by `(library_id, search_title)` from
//!    `series_metadata`. Lower-confidence matches; scoped to a single library so
//!    we do not collide common names across distinct libraries.

#![allow(dead_code)]

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, PaginatorTrait, QueryFilter, Set, Statement,
};
use tracing::{debug, info};
use uuid::Uuid;

use crate::db::entities::prelude::SeriesDuplicates;
use crate::db::entities::series_duplicates::{self, MATCH_TYPE_EXTERNAL_ID, MATCH_TYPE_TITLE};

/// Repository for SeriesDuplicates operations
pub struct SeriesDuplicatesRepository;

impl SeriesDuplicatesRepository {
    /// Find all duplicate groups (both match types).
    pub async fn find_all(db: &DatabaseConnection) -> Result<Vec<series_duplicates::Model>> {
        SeriesDuplicates::find()
            .all(db)
            .await
            .context("Failed to find all series duplicates")
    }

    /// Find duplicate groups filtered by match type.
    pub async fn find_by_match_type(
        db: &DatabaseConnection,
        match_type: &str,
    ) -> Result<Vec<series_duplicates::Model>> {
        SeriesDuplicates::find()
            .filter(series_duplicates::Column::MatchType.eq(match_type))
            .all(db)
            .await
            .context("Failed to find series duplicates by match type")
    }

    /// Count duplicate groups.
    pub async fn count(db: &DatabaseConnection) -> Result<u64> {
        SeriesDuplicates::find()
            .count(db)
            .await
            .context("Failed to count series duplicates")
    }

    /// Delete a specific group by id.
    pub async fn delete_group(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        SeriesDuplicates::delete_by_id(id)
            .exec(db)
            .await
            .context("Failed to delete series duplicate group")?;
        Ok(())
    }

    /// Rebuild the entire `series_duplicates` table from current series state.
    ///
    /// Returns the total number of duplicate groups found (both match types).
    pub async fn rebuild_from_series(db: &DatabaseConnection) -> Result<usize> {
        info!("Starting series duplicate rebuild");

        debug!("Clearing existing series duplicate records");
        let delete_stmt = Statement::from_string(
            db.get_database_backend(),
            "DELETE FROM series_duplicates".to_owned(),
        );
        db.execute(delete_stmt)
            .await
            .context("Failed to clear series duplicate records")?;

        let mut total = 0usize;
        total += Self::rebuild_external_id_groups(db).await?;
        total += Self::rebuild_title_groups(db).await?;

        info!("Series duplicate rebuild complete: {} groups found", total);
        Ok(total)
    }

    /// Detect series sharing the same `(source, external_id)` tuple.
    async fn rebuild_external_id_groups(db: &DatabaseConnection) -> Result<usize> {
        // Group external IDs by (source, external_id) and emit a row per group
        // that contains more than one *distinct* series.
        let query = match db.get_database_backend() {
            DatabaseBackend::Postgres => {
                r#"
                SELECT
                    source || ':' || external_id AS match_key,
                    json_agg(DISTINCT series_id) AS series_ids,
                    COUNT(DISTINCT series_id) AS duplicate_count
                FROM series_external_ids
                WHERE external_id != ''
                GROUP BY source, external_id
                HAVING COUNT(DISTINCT series_id) > 1
                ORDER BY COUNT(DISTINCT series_id) DESC
                "#
            }
            DatabaseBackend::Sqlite => {
                // SQLite has no DISTINCT inside GROUP_CONCAT with an ORDER BY,
                // but we can rely on the fact that each row in this table is
                // already unique per (series_id, source). Concatenate ids and
                // de-duplicate in application code defensively.
                r#"
                SELECT
                    source || ':' || external_id AS match_key,
                    GROUP_CONCAT(LOWER(HEX(series_id)), ',') AS series_ids_str,
                    COUNT(DISTINCT series_id) AS duplicate_count
                FROM series_external_ids
                WHERE external_id != ''
                GROUP BY source, external_id
                HAVING COUNT(DISTINCT series_id) > 1
                ORDER BY COUNT(DISTINCT series_id) DESC
                "#
            }
            _ => return Err(anyhow::anyhow!("Unsupported database backend")),
        };

        let stmt = Statement::from_string(db.get_database_backend(), query.to_owned());
        let rows = db
            .query_all(stmt)
            .await
            .context("Failed to query for series external-id duplicates")?;

        debug!("Found {} external-id duplicate groups", rows.len());

        let mut count = 0;
        for row in rows {
            let match_key: String = row.try_get("", "match_key")?;
            let duplicate_count: i64 = row.try_get("", "duplicate_count")?;
            let duplicate_count = duplicate_count as i32;

            let series_ids = parse_series_ids(db.get_database_backend(), &row)?;
            if series_ids.len() < 2 {
                continue;
            }

            insert_group(
                db,
                MATCH_TYPE_EXTERNAL_ID,
                &match_key,
                None,
                &series_ids,
                duplicate_count,
            )
            .await?;
            count += 1;
        }

        Ok(count)
    }

    /// Detect series in the same library that share the same `search_title`.
    async fn rebuild_title_groups(db: &DatabaseConnection) -> Result<usize> {
        let query = match db.get_database_backend() {
            DatabaseBackend::Postgres => {
                r#"
                SELECT
                    s.library_id AS library_id,
                    sm.search_title AS match_key,
                    json_agg(sm.series_id) AS series_ids,
                    COUNT(*) AS duplicate_count
                FROM series_metadata sm
                JOIN series s ON s.id = sm.series_id
                WHERE sm.search_title IS NOT NULL
                  AND sm.search_title != ''
                GROUP BY s.library_id, sm.search_title
                HAVING COUNT(*) > 1
                ORDER BY COUNT(*) DESC
                "#
            }
            DatabaseBackend::Sqlite => {
                r#"
                SELECT
                    LOWER(HEX(s.library_id)) AS library_id_hex,
                    sm.search_title AS match_key,
                    GROUP_CONCAT(LOWER(HEX(sm.series_id)), ',') AS series_ids_str,
                    COUNT(*) AS duplicate_count
                FROM series_metadata sm
                JOIN series s ON s.id = sm.series_id
                WHERE sm.search_title IS NOT NULL
                  AND sm.search_title != ''
                GROUP BY s.library_id, sm.search_title
                HAVING COUNT(*) > 1
                ORDER BY COUNT(*) DESC
                "#
            }
            _ => return Err(anyhow::anyhow!("Unsupported database backend")),
        };

        let stmt = Statement::from_string(db.get_database_backend(), query.to_owned());
        let rows = db
            .query_all(stmt)
            .await
            .context("Failed to query for series title duplicates")?;

        debug!("Found {} title duplicate groups", rows.len());

        let mut count = 0;
        for row in rows {
            let match_key: String = row.try_get("", "match_key")?;
            let duplicate_count: i64 = row.try_get("", "duplicate_count")?;
            let duplicate_count = duplicate_count as i32;

            let library_id = match db.get_database_backend() {
                DatabaseBackend::Postgres => {
                    let id: Uuid = row.try_get("", "library_id")?;
                    Some(id)
                }
                DatabaseBackend::Sqlite => {
                    let hex: String = row.try_get("", "library_id_hex")?;
                    Some(parse_hex_uuid(&hex)?)
                }
                _ => None,
            };

            let series_ids = parse_series_ids(db.get_database_backend(), &row)?;
            if series_ids.len() < 2 {
                continue;
            }

            insert_group(
                db,
                MATCH_TYPE_TITLE,
                &match_key,
                library_id,
                &series_ids,
                duplicate_count,
            )
            .await?;
            count += 1;
        }

        Ok(count)
    }

    /// Remove a series from any duplicate groups it participates in. Groups
    /// that fall below 2 members are deleted. Intended to be called when a
    /// series is hard-deleted.
    pub async fn cleanup_for_series(db: &DatabaseConnection, series_id: Uuid) -> Result<()> {
        debug!("Cleaning up series duplicates for series {}", series_id);

        let groups = SeriesDuplicates::find()
            .all(db)
            .await
            .context("Failed to fetch series duplicates for cleanup")?;

        for group in groups {
            let mut ids: Vec<Uuid> = serde_json::from_str(&group.series_ids)
                .context("Failed to parse series_ids JSON")?;

            if !ids.contains(&series_id) {
                continue;
            }

            ids.retain(|id| id != &series_id);

            if ids.len() <= 1 {
                debug!(
                    "Deleting series duplicate group {} (only {} series remaining)",
                    group.id,
                    ids.len()
                );
                SeriesDuplicates::delete_by_id(group.id)
                    .exec(db)
                    .await
                    .context("Failed to delete series duplicate group")?;
            } else {
                let mut active: series_duplicates::ActiveModel = group.into();
                active.series_ids = Set(serde_json::to_string(&ids)?);
                active.duplicate_count = Set(ids.len() as i32);
                active.updated_at = Set(Utc::now());
                active
                    .update(db)
                    .await
                    .context("Failed to update series duplicate group")?;
            }
        }

        Ok(())
    }
}

/// Parse the `series_ids` column from a query row, handling backend differences.
fn parse_series_ids(backend: DatabaseBackend, row: &sea_orm::QueryResult) -> Result<Vec<Uuid>> {
    let ids = match backend {
        DatabaseBackend::Postgres => {
            let json: serde_json::Value = row.try_get("", "series_ids")?;
            serde_json::from_value(json).context("Failed to parse series_ids JSON")?
        }
        DatabaseBackend::Sqlite => {
            let s: String = row.try_get("", "series_ids_str")?;
            s.split(',')
                .map(parse_hex_uuid)
                .collect::<Result<Vec<Uuid>>>()
                .context("Failed to parse series UUIDs from hex string")?
        }
        _ => return Err(anyhow::anyhow!("Unsupported database backend")),
    };
    // De-duplicate defensively in case the backend query produced repeats.
    let mut seen = std::collections::HashSet::new();
    Ok(ids.into_iter().filter(|id| seen.insert(*id)).collect())
}

/// Parse a 32-char hex UUID string (no dashes) into a `Uuid`.
fn parse_hex_uuid(hex: &str) -> Result<Uuid> {
    if hex.len() != 32 {
        return Err(anyhow::anyhow!("Invalid hex UUID length: {}", hex.len()));
    }
    let dashed = format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    );
    Uuid::parse_str(&dashed).context("Failed to parse UUID from hex string")
}

async fn insert_group(
    db: &DatabaseConnection,
    match_type: &str,
    match_key: &str,
    library_id: Option<Uuid>,
    series_ids: &[Uuid],
    duplicate_count: i32,
) -> Result<()> {
    let now = Utc::now();
    let model = series_duplicates::ActiveModel {
        id: Set(Uuid::new_v4()),
        match_type: Set(match_type.to_string()),
        match_key: Set(match_key.to_string()),
        library_id: Set(library_id),
        series_ids: Set(serde_json::to_string(series_ids)?),
        duplicate_count: Set(duplicate_count),
        created_at: Set(now),
        updated_at: Set(now),
    };
    model
        .insert(db)
        .await
        .context("Failed to insert series duplicate group")?;
    debug!(
        "Added series duplicate group: type={} key={} count={}",
        match_type, match_key, duplicate_count
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_uuid_round_trip() {
        let id = Uuid::new_v4();
        let hex = id.as_simple().to_string();
        let parsed = parse_hex_uuid(&hex).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn test_parse_hex_uuid_rejects_invalid_length() {
        assert!(parse_hex_uuid("nope").is_err());
    }
}
