//! Add `languages` column to `series_tracking` (Phase 6 of release-tracking).
//!
//! Per-series language preference for release-source plugins (e.g.
//! MangaUpdates) that aggregate scanlations across many languages. Stored as a
//! JSON array of ISO 639-1 codes, e.g. `["en"]` or `["en", "es"]`. NULL means
//! "fall back to the server-wide `release_tracking.default_languages` setting"
//! - that fallback policy lives in the plugin/service layer, not the schema.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("series_tracking"))
                    .add_column(ColumnDef::new(Alias::new("languages")).json_binary())
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("series_tracking"))
                    .drop_column(Alias::new("languages"))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
