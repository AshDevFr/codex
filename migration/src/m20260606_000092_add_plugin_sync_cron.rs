//! Add `sync_cron_schedule` column to `plugins`.
//!
//! Admin-managed cadence for scheduled user-plugin syncs. NULL means
//! "no scheduled sync" (the default, so existing installs are unchanged);
//! a non-NULL value is a normalized cron expression. When set on a
//! sync-capable plugin, the scheduler fans out a `UserPluginSync` task for
//! every connected user who has opted into auto sync.
//!
//! The per-user opt-in itself is NOT stored here: it lives host-side in
//! `user_plugins.config._codex.autoSync`, so no `user_plugins` schema
//! change is needed.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Plugins::Table)
                    .add_column(ColumnDef::new(Plugins::SyncCronSchedule).text())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Plugins::Table)
                    .drop_column(Plugins::SyncCronSchedule)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Plugins {
    Table,
    SyncCronSchedule,
}
