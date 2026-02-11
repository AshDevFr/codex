//! Add internal_config column to plugins table
//!
//! This migration adds:
//! - `internal_config`: JSON TEXT column for server-side per-plugin configuration.
//!   This is distinct from `config` (which is sent to the plugin process).
//!   `internal_config` stores settings that Codex uses internally to control its
//!   own behavior when interacting with the plugin (e.g., search_results_limit).
//!
//! Nullable — NULL means all defaults apply.

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
                    .add_column(ColumnDef::new(Plugins::InternalConfig).text())
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
                    .drop_column(Plugins::InternalConfig)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Plugins {
    Table,
    InternalConfig,
}
