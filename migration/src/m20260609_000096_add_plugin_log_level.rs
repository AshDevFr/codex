//! Add `log_level` column to `plugins`.
//!
//! Per-plugin override for the log level the host sends each plugin at
//! `initialize`. NULL means "use the default", which is the host's own
//! `logging.level` (the plugin SDK has no `trace`, so `trace` is delivered as
//! `debug`). Set a value here to make a single plugin more or less verbose
//! without touching the server level or any other plugin.

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
                    .add_column(ColumnDef::new(Plugins::LogLevel).string())
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
                    .drop_column(Plugins::LogLevel)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Plugins {
    Table,
    LogLevel,
}
