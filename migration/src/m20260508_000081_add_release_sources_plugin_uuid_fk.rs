//! Add `plugin_uuid` FK column to `release_sources` for cascade-on-delete.
//!
//! `release_sources.plugin_id` is a string (the plugin's manifest name) used
//! by the plugin RPC layer for self-identification. There's no FK to
//! `plugins.id`, so deleting a plugin row leaves orphaned source rows behind:
//! they keep showing in the settings UI under the deleted plugin name and
//! survive across reinstalls.
//!
//! Rather than convert `plugin_id` to UUID end-to-end (which would churn
//! the entire RPC contract and ~40 test fixtures), we add a *parallel*
//! `plugin_uuid` column. The string column stays as the plugin's
//! self-identifier; the UUID is the lifecycle anchor. The repository's
//! create/upsert path populates both consistently by looking up `plugins`
//! by name once at insert time.
//!
//! Backfill rules:
//!   - For each existing row, set `plugin_uuid = (SELECT id FROM plugins
//!     WHERE plugins.name = release_sources.plugin_id)`.
//!   - Rows whose lookup fails (orphans — plugin already deleted) are
//!     dropped. The reserved `plugin_id = 'core'` synthetic-source value
//!     has no plugins row by design; those rows keep `plugin_uuid = NULL`
//!     (the FK is nullable to accommodate them).
//!
//! Cross-DB note on the FK: PostgreSQL accepts
//! `ALTER TABLE ... ADD CONSTRAINT FK` directly. SQLite does not — adding
//! a FK to an existing column requires the (error-prone) "12-step" table
//! swap, and even that interacts badly with the inbound FK from
//! `release_ledger.source_id` because connection-level PRAGMAs interact
//! with the migrator's transaction in surprising ways.
//!
//! Pragmatic compromise: PostgreSQL deployments get the DB-level FK +
//! cascade. SQLite deployments rely on app-level cleanup in the plugin
//! delete handler (see [`crate::api::routes::v1::handlers::plugins::delete_plugin`]).
//! The `plugin_uuid` column is populated on both backends so the app can
//! cleanly relate sources to plugins regardless. The few SQLite users
//! who delete plugin rows directly via SQL (rather than the API) will
//! see orphans — those can be cleaned up with a single
//! `DELETE FROM release_sources WHERE plugin_uuid IS NULL AND plugin_id != 'core'`.

use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = db.get_database_backend();

        // 1. Add the column on both backends.
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("release_sources"))
                    .add_column(ColumnDef::new(Alias::new("plugin_uuid")).uuid())
                    .to_owned(),
            )
            .await?;

        // 2. Backfill from `plugins.name`.
        db.execute(Statement::from_string(
            backend,
            r#"UPDATE release_sources
               SET plugin_uuid = (
                   SELECT id FROM plugins WHERE plugins.name = release_sources.plugin_id
               )
               WHERE plugin_id != 'core'"#
                .to_string(),
        ))
        .await?;

        // 3. Drop orphans (plugin already deleted; no plugins row to FK to).
        //    The associated `release_ledger` rows are removed by the existing
        //    `fk_release_ledger_source_id` cascade.
        db.execute(Statement::from_string(
            backend,
            r#"DELETE FROM release_sources
               WHERE plugin_id != 'core' AND plugin_uuid IS NULL"#
                .to_string(),
        ))
        .await?;

        // 4. Add FK on PostgreSQL/MySQL. Skip on SQLite — the app-level
        //    cleanup in the plugin delete handler handles the cascade
        //    there; see the module-level comment.
        if matches!(backend, DbBackend::Postgres | DbBackend::MySql) {
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_release_sources_plugin_uuid")
                        .from(Alias::new("release_sources"), Alias::new("plugin_uuid"))
                        .to(Alias::new("plugins"), Alias::new("id"))
                        .on_delete(ForeignKeyAction::Cascade)
                        .to_owned(),
                )
                .await?;
        }

        // 5. Index the new column. Speeds the cascade-lookup (Postgres) and
        //    administrative joins on both backends.
        db.execute(Statement::from_string(
            backend,
            "CREATE INDEX idx_release_sources_plugin_uuid \
             ON release_sources(plugin_uuid)"
                .to_string(),
        ))
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = db.get_database_backend();
        db.execute(Statement::from_string(
            backend,
            "DROP INDEX IF EXISTS idx_release_sources_plugin_uuid".to_string(),
        ))
        .await?;
        if matches!(backend, DbBackend::Postgres | DbBackend::MySql) {
            manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .table(Alias::new("release_sources"))
                        .name("fk_release_sources_plugin_uuid")
                        .to_owned(),
                )
                .await?;
        }
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("release_sources"))
                    .drop_column(Alias::new("plugin_uuid"))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
