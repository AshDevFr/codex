//! Add `metadata_refresh_config` column to `libraries` (Phase 1 of scheduled-metadata-refresh).
//!
//! Stores per-library JSON configuration for the scheduled metadata refresh
//! feature: cron schedule, field groups to refresh, providers, and safety
//! options. Mirrors the existing `scanning_config`/`title_preprocessing_rules`
//! pattern: a nullable TEXT column whose contents are parsed lazily.
//!
//! NULL means "feature off, defaults applied when read." No data backfill
//! needed; the feature is opt-in per library.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Libraries::Table)
                    .add_column(ColumnDef::new(Libraries::MetadataRefreshConfig).text())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Libraries::Table)
                    .drop_column(Libraries::MetadataRefreshConfig)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Libraries {
    Table,
    MetadataRefreshConfig,
}
