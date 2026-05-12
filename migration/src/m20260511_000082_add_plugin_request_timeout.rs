//! Add `request_timeout_seconds` column to `plugins`.
//!
//! Per-plugin override for the host → plugin RPC deadline. NULL means
//! "use the manager's default" (currently 30s). Long-running plugins
//! (e.g. release pollers fanning out to many series) can be configured
//! with a larger value so their forward calls don't get killed before
//! the plugin finishes.
//!
//! Handlers that already pass their own deadline (`poll_release_source`
//! uses `plugin.task_request_timeout_seconds`, default 300s) still win
//! over this column — it just changes the baseline for callers that
//! don't.

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
                    .add_column(ColumnDef::new(Plugins::RequestTimeoutSeconds).integer())
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
                    .drop_column(Plugins::RequestTimeoutSeconds)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Plugins {
    Table,
    RequestTimeoutSeconds,
}
