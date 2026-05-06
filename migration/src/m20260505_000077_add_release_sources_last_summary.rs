//! Add `last_summary` column to `release_sources`.
//!
//! Free-form text written by the poll-source task on every successful poll
//! completion (e.g. `"fetched 12 items, matched 0, recorded 0"`). The
//! Release tracking settings UI surfaces it under the per-row status badge
//! so users can see *why* a poll returned no announcements (no tracked
//! series with aliases, upstream not modified, etc.) without grepping
//! container logs. NULL until the first successful poll.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("release_sources"))
                    .add_column(ColumnDef::new(Alias::new("last_summary")).text())
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("release_sources"))
                    .drop_column(Alias::new("last_summary"))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
