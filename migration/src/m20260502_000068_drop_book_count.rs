//! Drop the legacy `series_metadata.total_book_count` and `total_book_count_lock` columns.
//!
//! Phase 9 of the metadata-count-split plan: hard removal. The new
//! `total_volume_count` / `total_chapter_count` columns and their locks are now
//! the sole source of truth, written by `MetadataApplier` and surfaced through
//! every read site. The legacy column is no longer read or written anywhere in
//! the codebase, so we drop it to make any leftover reference fail at compile
//! or runtime.
//!
//! Down: re-adds the legacy columns as nullable (value) and not-null+default
//! (lock). No data restore is possible (volume data was already copied across
//! in migration 067 but is not symmetric to a chapter-organized state). Down
//! exists for symmetry and dev-environment reset; production rollback requires
//! restoring from a pre-Phase-9 backup.

use sea_orm_migration::prelude::*;

use crate::m20260103_000006_create_series_metadata::SeriesMetadata;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .drop_column(Alias::new("total_book_count_lock"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .drop_column(Alias::new("total_book_count"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .add_column(ColumnDef::new(Alias::new("total_book_count")).integer())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("total_book_count_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
