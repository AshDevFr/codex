//! Repository for the `release_sources` table.
//!
//! One row per logical source a plugin (or core) exposes. The plugin → source
//! relationship is many-to-one: e.g., a single Nyaa plugin instance exposes
//! one source per uploader subscription. CRUD here, plus state-tracking
//! helpers (`record_poll_success`, `record_poll_error`) used by the polling
//! task in Phase 4.

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use uuid::Uuid;

use crate::db::entities::release_sources::{
    self, Entity as ReleaseSources, Model as ReleaseSource, kind,
};
use crate::utils::cron::validate_cron_expression;

/// Normalize a caller-supplied cron schedule: trim, treat empty as `None`,
/// validate the parse, and return the trimmed string. Errors when the
/// expression is non-empty but invalid.
fn sanitize_cron_schedule(value: Option<String>) -> Result<Option<String>> {
    let Some(raw) = value else { return Ok(None) };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    validate_cron_expression(trimmed)
        .map_err(|e| anyhow::anyhow!("invalid cron_schedule: {}", e))?;
    Ok(Some(trimmed.to_string()))
}

/// Parameters for creating a new release source. Only the fields a caller is
/// expected to choose live here; `created_at` / `updated_at` / `id` are
/// generated.
#[derive(Debug, Clone)]
pub struct NewReleaseSource {
    pub plugin_id: String,
    pub source_key: String,
    pub display_name: String,
    pub kind: String,
    pub enabled: Option<bool>,
    pub config: Option<serde_json::Value>,
}

/// PATCH-style update payload. Each `Option<T>` distinguishes "leave alone"
/// (`None`) from "set". `cron_schedule` uses `Option<Option<String>>` so the
/// caller can explicitly clear a row's override (revert to inheriting the
/// server-wide default) by sending `Some(None)`.
#[derive(Debug, Default, Clone)]
pub struct ReleaseSourceUpdate {
    pub display_name: Option<String>,
    pub enabled: Option<bool>,
    pub cron_schedule: Option<Option<String>>,
    pub config: Option<Option<serde_json::Value>>,
}

pub struct ReleaseSourceRepository;

impl ReleaseSourceRepository {
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<ReleaseSource>> {
        Ok(ReleaseSources::find_by_id(id).one(db).await?)
    }

    /// Lookup by the natural composite key `(plugin_id, source_key)`.
    pub async fn find_by_key(
        db: &DatabaseConnection,
        plugin_id: &str,
        source_key: &str,
    ) -> Result<Option<ReleaseSource>> {
        Ok(ReleaseSources::find()
            .filter(release_sources::Column::PluginId.eq(plugin_id))
            .filter(release_sources::Column::SourceKey.eq(source_key))
            .one(db)
            .await?)
    }

    /// List all sources, ordered by `(plugin_id, source_key)` for stable display.
    pub async fn list_all(db: &DatabaseConnection) -> Result<Vec<ReleaseSource>> {
        Ok(ReleaseSources::find()
            .order_by_asc(release_sources::Column::PluginId)
            .order_by_asc(release_sources::Column::SourceKey)
            .all(db)
            .await?)
    }

    /// List enabled sources only. Hot path for the scheduler.
    pub async fn list_enabled(db: &DatabaseConnection) -> Result<Vec<ReleaseSource>> {
        Ok(ReleaseSources::find()
            .filter(release_sources::Column::Enabled.eq(true))
            .order_by_asc(release_sources::Column::PluginId)
            .order_by_asc(release_sources::Column::SourceKey)
            .all(db)
            .await?)
    }

    /// Count all sources (used for inventory metrics).
    pub async fn count(db: &DatabaseConnection) -> Result<u64> {
        Ok(ReleaseSources::find().count(db).await?)
    }

    /// Create a new source. Validates `kind` against the canonical set.
    /// New rows always start with `cron_schedule = NULL` (inherit the
    /// server-wide default); admins can override per-row via PATCH.
    pub async fn create(
        db: &DatabaseConnection,
        params: NewReleaseSource,
    ) -> Result<ReleaseSource> {
        if !kind::is_valid(&params.kind) {
            anyhow::bail!("invalid kind: {}", params.kind);
        }
        if params.plugin_id.trim().is_empty() {
            anyhow::bail!("plugin_id cannot be empty");
        }
        if params.source_key.trim().is_empty() {
            anyhow::bail!("source_key cannot be empty");
        }

        let now = Utc::now();
        let active = release_sources::ActiveModel {
            id: Set(Uuid::new_v4()),
            plugin_id: Set(params.plugin_id),
            source_key: Set(params.source_key),
            display_name: Set(params.display_name),
            kind: Set(params.kind),
            enabled: Set(params.enabled.unwrap_or(true)),
            cron_schedule: Set(None),
            last_polled_at: Set(None),
            last_error: Set(None),
            last_error_at: Set(None),
            etag: Set(None),
            config: Set(params.config),
            last_summary: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };
        Ok(active.insert(db).await?)
    }

    /// Get-or-create a synthetic in-core source (used by the metadata-piggyback
    /// path in Phase 5). Distinct from `create` so callers don't accidentally
    /// create duplicate synthetic rows.
    pub async fn get_or_create(
        db: &DatabaseConnection,
        params: NewReleaseSource,
    ) -> Result<ReleaseSource> {
        if let Some(existing) = Self::find_by_key(db, &params.plugin_id, &params.source_key).await?
        {
            return Ok(existing);
        }
        Self::create(db, params).await
    }

    /// Idempotent upsert keyed on `(plugin_id, source_key)`.
    ///
    /// On insert, the row is created with `params` and defaults to enabled.
    /// On update, **only the plugin-owned descriptive fields** are refreshed
    /// (`display_name`, `kind`, `config`). User-managed fields (`enabled`,
    /// `cron_schedule`) are preserved so an admin's schedule override or
    /// disable toggle survives a plugin re-registration.
    ///
    /// Used by `releases/register_sources` so a plugin can declare its full
    /// desired-state list on every initialize without trampling user choices.
    pub async fn upsert(
        db: &DatabaseConnection,
        params: NewReleaseSource,
    ) -> Result<ReleaseSource> {
        if !kind::is_valid(&params.kind) {
            anyhow::bail!("invalid kind: {}", params.kind);
        }
        if let Some(existing) = Self::find_by_key(db, &params.plugin_id, &params.source_key).await?
        {
            let mut active: release_sources::ActiveModel = existing.into();
            active.display_name = Set(params.display_name);
            active.kind = Set(params.kind);
            active.config = Set(params.config);
            active.updated_at = Set(Utc::now());
            return Ok(active.update(db).await?);
        }
        Self::create(db, params).await
    }

    /// Return every row owned by `plugin_id`, ordered by `source_key`.
    pub async fn list_by_plugin(
        db: &DatabaseConnection,
        plugin_id: &str,
    ) -> Result<Vec<ReleaseSource>> {
        Ok(ReleaseSources::find()
            .filter(release_sources::Column::PluginId.eq(plugin_id))
            .order_by_asc(release_sources::Column::SourceKey)
            .all(db)
            .await?)
    }

    /// Delete every row owned by `plugin_id` whose `source_key` is **not** in
    /// `keep_keys`. Returns the number of rows removed. Cascades to
    /// `release_ledger`. Used by `register_sources` to prune sources that the
    /// plugin no longer declares.
    pub async fn delete_by_plugin_excluding(
        db: &DatabaseConnection,
        plugin_id: &str,
        keep_keys: &[String],
    ) -> Result<u64> {
        let mut query =
            ReleaseSources::delete_many().filter(release_sources::Column::PluginId.eq(plugin_id));
        if !keep_keys.is_empty() {
            query = query.filter(release_sources::Column::SourceKey.is_not_in(keep_keys.to_vec()));
        }
        let result = query.exec(db).await?;
        Ok(result.rows_affected)
    }

    /// Apply a PATCH-style update.
    pub async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        update: ReleaseSourceUpdate,
    ) -> Result<ReleaseSource> {
        let existing = ReleaseSources::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("release source {} not found", id))?;

        let mut active: release_sources::ActiveModel = existing.into();
        if let Some(name) = update.display_name {
            active.display_name = Set(name);
        }
        if let Some(enabled) = update.enabled {
            active.enabled = Set(enabled);
        }
        if let Some(cron) = update.cron_schedule {
            // Some(None) -> clear (inherit server default); Some(Some(s)) -> set override.
            let sanitized = sanitize_cron_schedule(cron)?;
            active.cron_schedule = Set(sanitized);
        }
        if let Some(cfg) = update.config {
            active.config = Set(cfg);
        }
        active.updated_at = Set(Utc::now());
        Ok(active.update(db).await?)
    }

    /// Record a successful poll. Clears any prior error and bumps `last_polled_at`.
    /// Optionally sets a new etag/cursor.
    pub async fn record_poll_success(
        db: &DatabaseConnection,
        id: Uuid,
        polled_at: DateTime<Utc>,
        etag: Option<String>,
        summary: Option<String>,
    ) -> Result<()> {
        let existing = ReleaseSources::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("release source {} not found", id))?;
        let mut active: release_sources::ActiveModel = existing.into();
        active.last_polled_at = Set(Some(polled_at));
        active.last_error = Set(None);
        active.last_error_at = Set(None);
        if let Some(e) = etag {
            active.etag = Set(Some(e));
        }
        // None passed by the caller means "leave alone"; older callers can pass
        // None and keep their existing behavior. Pass Some("…") to overwrite.
        if let Some(s) = summary {
            active.last_summary = Set(Some(s));
        }
        active.updated_at = Set(Utc::now());
        active.update(db).await?;
        Ok(())
    }

    /// Record a poll error. Does NOT touch `last_polled_at` (we still consider
    /// the poll attempt observed, but `last_error` lets the UI surface failures).
    pub async fn record_poll_error(
        db: &DatabaseConnection,
        id: Uuid,
        error: &str,
        errored_at: DateTime<Utc>,
    ) -> Result<()> {
        let existing = ReleaseSources::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("release source {} not found", id))?;
        let mut active: release_sources::ActiveModel = existing.into();
        active.last_error = Set(Some(error.to_string()));
        active.last_error_at = Set(Some(errored_at));
        active.updated_at = Set(Utc::now());
        active.update(db).await?;
        Ok(())
    }

    /// Delete a source. Cascades to `release_ledger` rows.
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = ReleaseSources::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Reset all transient poll state on a source: clears `etag`,
    /// `last_polled_at`, `last_error`, `last_error_at`, and `last_summary`.
    /// Leaves user-managed fields (`enabled`, `cron_schedule`,
    /// `display_name`, `config`) untouched.
    ///
    /// Used by the source-reset admin endpoint so a forced re-poll fetches
    /// the upstream feed afresh (no `If-None-Match` 304) and re-records
    /// every release as `announced`.
    pub async fn clear_poll_state(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        let existing = ReleaseSources::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("release source {} not found", id))?;
        let mut active: release_sources::ActiveModel = existing.into();
        active.last_polled_at = Set(None);
        active.last_error = Set(None);
        active.last_error_at = Set(None);
        active.etag = Set(None);
        active.last_summary = Set(None);
        active.updated_at = Set(Utc::now());
        active.update(db).await?;
        Ok(())
    }

    /// Clear only the `etag` for this source. Used when a user deletes
    /// individual ledger rows and wants the next poll to bypass the
    /// upstream's `If-None-Match` cache (so the deleted row gets re-recorded
    /// in `announced` state). Lighter than `clear_poll_state`: poll history
    /// (`last_polled_at`, `last_error`, `last_summary`) is preserved.
    pub async fn clear_etag(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        // Use update_many so a missing row is a silent no-op rather than an
        // error. Per-row ledger deletes can race with a source deletion; the
        // dropped ledger row is the user's intent regardless of whether the
        // source still exists.
        ReleaseSources::update_many()
            .col_expr(
                release_sources::Column::Etag,
                sea_orm::sea_query::Expr::value(Option::<String>::None),
            )
            .col_expr(
                release_sources::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(Utc::now()),
            )
            .filter(release_sources::Column::Id.eq(id))
            .exec(db)
            .await?;
        Ok(())
    }

    /// Clear `etag` on every source in `ids` in a single statement.
    pub async fn clear_etag_many(db: &DatabaseConnection, ids: &[Uuid]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        ReleaseSources::update_many()
            .col_expr(
                release_sources::Column::Etag,
                sea_orm::sea_query::Expr::value(Option::<String>::None),
            )
            .col_expr(
                release_sources::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(Utc::now()),
            )
            .filter(release_sources::Column::Id.is_in(ids.to_vec()))
            .exec(db)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::create_test_db;

    fn nyaa_source() -> NewReleaseSource {
        NewReleaseSource {
            plugin_id: "release-nyaa".to_string(),
            source_key: "nyaa:user:tsuna69".to_string(),
            display_name: "Nyaa - tsuna69".to_string(),
            kind: kind::RSS_UPLOADER.to_string(),
            enabled: None,
            config: None,
        }
    }

    #[tokio::test]
    async fn create_and_lookup_roundtrip() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let created = ReleaseSourceRepository::create(conn, nyaa_source())
            .await
            .unwrap();
        assert_eq!(created.plugin_id, "release-nyaa");
        assert!(created.enabled, "default to enabled");

        let by_id = ReleaseSourceRepository::get_by_id(conn, created.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(by_id.id, created.id);

        let by_key =
            ReleaseSourceRepository::find_by_key(conn, "release-nyaa", "nyaa:user:tsuna69")
                .await
                .unwrap()
                .unwrap();
        assert_eq!(by_key.id, created.id);
    }

    #[tokio::test]
    async fn create_rejects_invalid_kind() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let mut params = nyaa_source();
        params.kind = "frobnicate".to_string();
        let err = ReleaseSourceRepository::create(conn, params)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("invalid kind"));
    }

    #[tokio::test]
    async fn update_rejects_invalid_cron() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let s = ReleaseSourceRepository::create(conn, nyaa_source())
            .await
            .unwrap();
        let err = ReleaseSourceRepository::update(
            conn,
            s.id,
            ReleaseSourceUpdate {
                cron_schedule: Some(Some("not a cron".to_string())),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
        assert!(err.to_string().to_lowercase().contains("cron"));
    }

    #[tokio::test]
    async fn update_clears_cron_schedule_with_explicit_none() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let s = ReleaseSourceRepository::create(conn, nyaa_source())
            .await
            .unwrap();
        // Set an override.
        ReleaseSourceRepository::update(
            conn,
            s.id,
            ReleaseSourceUpdate {
                cron_schedule: Some(Some("0 */6 * * *".to_string())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let after_set = ReleaseSourceRepository::get_by_id(conn, s.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after_set.cron_schedule.as_deref(), Some("0 */6 * * *"));

        // Clear back to inherit.
        ReleaseSourceRepository::update(
            conn,
            s.id,
            ReleaseSourceUpdate {
                cron_schedule: Some(None),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let after_clear = ReleaseSourceRepository::get_by_id(conn, s.id)
            .await
            .unwrap()
            .unwrap();
        assert!(after_clear.cron_schedule.is_none());
    }

    #[tokio::test]
    async fn update_treats_empty_cron_as_clear() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let s = ReleaseSourceRepository::create(conn, nyaa_source())
            .await
            .unwrap();
        ReleaseSourceRepository::update(
            conn,
            s.id,
            ReleaseSourceUpdate {
                cron_schedule: Some(Some("0 */6 * * *".to_string())),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        ReleaseSourceRepository::update(
            conn,
            s.id,
            ReleaseSourceUpdate {
                cron_schedule: Some(Some("   ".to_string())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let after = ReleaseSourceRepository::get_by_id(conn, s.id)
            .await
            .unwrap()
            .unwrap();
        assert!(after.cron_schedule.is_none());
    }

    #[tokio::test]
    async fn get_or_create_is_idempotent_per_key() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let a = ReleaseSourceRepository::get_or_create(conn, nyaa_source())
            .await
            .unwrap();
        let b = ReleaseSourceRepository::get_or_create(conn, nyaa_source())
            .await
            .unwrap();
        assert_eq!(a.id, b.id, "same key returns same row");
    }

    #[tokio::test]
    async fn list_enabled_filters_disabled_rows() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let a = ReleaseSourceRepository::create(conn, nyaa_source())
            .await
            .unwrap();
        let mut p2 = nyaa_source();
        p2.source_key = "nyaa:user:other".to_string();
        let b = ReleaseSourceRepository::create(conn, p2).await.unwrap();

        ReleaseSourceRepository::update(
            conn,
            b.id,
            ReleaseSourceUpdate {
                enabled: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let enabled = ReleaseSourceRepository::list_enabled(conn).await.unwrap();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].id, a.id);
    }

    #[tokio::test]
    async fn record_poll_success_clears_error_and_sets_etag() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let s = ReleaseSourceRepository::create(conn, nyaa_source())
            .await
            .unwrap();

        // Seed an error first.
        ReleaseSourceRepository::record_poll_error(conn, s.id, "503 upstream", Utc::now())
            .await
            .unwrap();
        let after_err = ReleaseSourceRepository::get_by_id(conn, s.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after_err.last_error.as_deref(), Some("503 upstream"));

        // Successful poll clears the error and sets etag + summary.
        let polled_at = Utc::now();
        ReleaseSourceRepository::record_poll_success(
            conn,
            s.id,
            polled_at,
            Some("\"etag-1\"".to_string()),
            Some("Fetched 0 items".to_string()),
        )
        .await
        .unwrap();
        let after_ok = ReleaseSourceRepository::get_by_id(conn, s.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after_ok.last_error, None);
        assert_eq!(after_ok.last_error_at, None);
        assert_eq!(after_ok.last_polled_at, Some(polled_at));
        assert_eq!(after_ok.etag.as_deref(), Some("\"etag-1\""));
        assert_eq!(after_ok.last_summary.as_deref(), Some("Fetched 0 items"));
    }

    #[tokio::test]
    async fn record_poll_error_does_not_touch_last_polled_at() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let s = ReleaseSourceRepository::create(conn, nyaa_source())
            .await
            .unwrap();

        // First a success.
        let success_at = Utc::now();
        ReleaseSourceRepository::record_poll_success(conn, s.id, success_at, None, None)
            .await
            .unwrap();

        // Then an error.
        ReleaseSourceRepository::record_poll_error(conn, s.id, "boom", Utc::now())
            .await
            .unwrap();

        let after = ReleaseSourceRepository::get_by_id(conn, s.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            after.last_polled_at,
            Some(success_at),
            "last_polled_at preserved on error so users can see when we last got data"
        );
        assert_eq!(after.last_error.as_deref(), Some("boom"));
    }

    #[tokio::test]
    async fn unique_constraint_on_plugin_id_source_key() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        ReleaseSourceRepository::create(conn, nyaa_source())
            .await
            .unwrap();

        // Same (plugin_id, source_key) - should fail at the unique index.
        let result = ReleaseSourceRepository::create(conn, nyaa_source()).await;
        assert!(result.is_err(), "duplicate key must fail");
    }

    #[tokio::test]
    async fn upsert_creates_when_missing_and_preserves_user_fields_on_update() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        // First call creates the row.
        let created = ReleaseSourceRepository::upsert(conn, nyaa_source())
            .await
            .unwrap();
        assert!(created.enabled);
        assert!(
            created.cron_schedule.is_none(),
            "fresh row inherits server-wide default"
        );

        // Admin disables and sets a cron override.
        ReleaseSourceRepository::update(
            conn,
            created.id,
            ReleaseSourceUpdate {
                enabled: Some(false),
                cron_schedule: Some(Some("0 */6 * * *".to_string())),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        // Plugin re-registers with a different display name, kind, and config.
        let mut params = nyaa_source();
        params.display_name = "Nyaa: tsuna69 (refreshed)".to_string();
        params.config = Some(serde_json::json!({ "subscription": "tsuna69" }));
        let updated = ReleaseSourceRepository::upsert(conn, params).await.unwrap();

        assert_eq!(updated.id, created.id, "same key returns same row");
        assert_eq!(updated.display_name, "Nyaa: tsuna69 (refreshed)");
        assert_eq!(
            updated.config,
            Some(serde_json::json!({ "subscription": "tsuna69" }))
        );
        assert!(
            !updated.enabled,
            "user-set enabled flag must survive a plugin re-register"
        );
        assert_eq!(
            updated.cron_schedule.as_deref(),
            Some("0 */6 * * *"),
            "user-set cron_schedule must survive a plugin re-register"
        );
    }

    #[tokio::test]
    async fn list_by_plugin_returns_only_that_plugins_rows() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        ReleaseSourceRepository::create(conn, nyaa_source())
            .await
            .unwrap();
        let mut other = nyaa_source();
        other.plugin_id = "release-mangaupdates".to_string();
        other.source_key = "default".to_string();
        ReleaseSourceRepository::create(conn, other).await.unwrap();

        let nyaa = ReleaseSourceRepository::list_by_plugin(conn, "release-nyaa")
            .await
            .unwrap();
        assert_eq!(nyaa.len(), 1);
        assert_eq!(nyaa[0].plugin_id, "release-nyaa");
    }

    #[tokio::test]
    async fn delete_by_plugin_excluding_prunes_missing_keys() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let mut a = nyaa_source();
        a.source_key = "user:tsuna69".to_string();
        let mut b = nyaa_source();
        b.source_key = "user:other".to_string();
        let mut c = nyaa_source();
        c.source_key = "user:gone".to_string();
        ReleaseSourceRepository::create(conn, a).await.unwrap();
        ReleaseSourceRepository::create(conn, b).await.unwrap();
        ReleaseSourceRepository::create(conn, c).await.unwrap();

        // Keep only the first two.
        let keep = vec!["user:tsuna69".to_string(), "user:other".to_string()];
        let removed =
            ReleaseSourceRepository::delete_by_plugin_excluding(conn, "release-nyaa", &keep)
                .await
                .unwrap();
        assert_eq!(removed, 1);

        let remaining = ReleaseSourceRepository::list_by_plugin(conn, "release-nyaa")
            .await
            .unwrap();
        assert_eq!(remaining.len(), 2);
        let keys: Vec<&str> = remaining.iter().map(|r| r.source_key.as_str()).collect();
        assert!(keys.contains(&"user:tsuna69"));
        assert!(keys.contains(&"user:other"));
    }

    #[tokio::test]
    async fn delete_by_plugin_excluding_with_empty_keep_removes_all() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        ReleaseSourceRepository::create(conn, nyaa_source())
            .await
            .unwrap();
        let mut other = nyaa_source();
        other.source_key = "user:other".to_string();
        ReleaseSourceRepository::create(conn, other).await.unwrap();

        let removed =
            ReleaseSourceRepository::delete_by_plugin_excluding(conn, "release-nyaa", &[])
                .await
                .unwrap();
        assert_eq!(removed, 2);

        let remaining = ReleaseSourceRepository::list_by_plugin(conn, "release-nyaa")
            .await
            .unwrap();
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn delete_by_plugin_excluding_does_not_touch_other_plugins() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        ReleaseSourceRepository::create(conn, nyaa_source())
            .await
            .unwrap();
        let mut other = nyaa_source();
        other.plugin_id = "release-mangaupdates".to_string();
        other.source_key = "default".to_string();
        ReleaseSourceRepository::create(conn, other).await.unwrap();

        // Wipe everything for nyaa; mangaupdates row must survive.
        ReleaseSourceRepository::delete_by_plugin_excluding(conn, "release-nyaa", &[])
            .await
            .unwrap();

        let mu = ReleaseSourceRepository::list_by_plugin(conn, "release-mangaupdates")
            .await
            .unwrap();
        assert_eq!(mu.len(), 1);
    }

    #[tokio::test]
    async fn clear_poll_state_resets_transient_fields_only() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let s = ReleaseSourceRepository::create(conn, nyaa_source())
            .await
            .unwrap();

        // Seed some poll state and a user override.
        ReleaseSourceRepository::record_poll_success(
            conn,
            s.id,
            Utc::now(),
            Some("\"etag-1\"".to_string()),
            Some("Fetched 3 items".to_string()),
        )
        .await
        .unwrap();
        ReleaseSourceRepository::update(
            conn,
            s.id,
            ReleaseSourceUpdate {
                enabled: Some(false),
                cron_schedule: Some(Some("0 */6 * * *".to_string())),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        ReleaseSourceRepository::clear_poll_state(conn, s.id)
            .await
            .unwrap();

        let after = ReleaseSourceRepository::get_by_id(conn, s.id)
            .await
            .unwrap()
            .unwrap();
        assert!(after.etag.is_none());
        assert!(after.last_polled_at.is_none());
        assert!(after.last_error.is_none());
        assert!(after.last_error_at.is_none());
        assert!(after.last_summary.is_none());
        // User-managed fields preserved.
        assert!(!after.enabled);
        assert_eq!(after.cron_schedule.as_deref(), Some("0 */6 * * *"));
    }

    #[tokio::test]
    async fn delete_removes_row() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let s = ReleaseSourceRepository::create(conn, nyaa_source())
            .await
            .unwrap();

        let removed = ReleaseSourceRepository::delete(conn, s.id).await.unwrap();
        assert!(removed);
        let gone = ReleaseSourceRepository::get_by_id(conn, s.id)
            .await
            .unwrap();
        assert!(gone.is_none());
    }
}
