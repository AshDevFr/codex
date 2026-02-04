//! Add metadata_targets column to plugins table
//!
//! This migration adds:
//! - `metadata_targets`: JSON array specifying which resource types the plugin should
//!   auto-match against. Nullable - NULL means "auto" (infer from plugin capabilities).
//!
//! Example values:
//! - `NULL`: Auto-detect from plugin manifest capabilities (backward compatible default)
//! - `["series"]`: Only run series auto-match
//! - `["book"]`: Only run book auto-match (uses book title, not series title)
//! - `["series", "book"]`: Run both series and book auto-match

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
                    .add_column(ColumnDef::new(Plugins::MetadataTargets).text())
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
                    .drop_column(Plugins::MetadataTargets)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Plugins {
    Table,
    MetadataTargets,
}
