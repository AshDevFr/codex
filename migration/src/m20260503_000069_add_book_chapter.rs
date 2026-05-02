//! Add `chapter` and `chapter_lock` columns to `book_metadata` (Phase 11 of metadata-count-split).
//!
//! Per-book classification: `book_metadata` already has `volume Option<i32>` and
//! `volume_lock`. This migration adds the sibling `chapter Option<f32>` plus
//! `chapter_lock`. The combination of populated/null values across the two
//! columns derives the kind of book (volume / chapter / chapter-of-volume /
//! unknown) without needing an explicit enum.
//!
//! Why REAL (f32): chapter numbers in manga frequently include decimals
//! (e.g. 47.5 for "side chapter"). Matches `series_metadata.total_chapter_count`
//! and `series_tracking.latest_known_chapter`, both REAL.

use sea_orm_migration::prelude::*;

use crate::m20260103_000014_create_book_metadata::BookMetadata;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add chapter (REAL/FLOAT NULL).
        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(ColumnDef::new(Alias::new("chapter")).float())
                    .to_owned(),
            )
            .await?;

        // Add chapter_lock (BOOLEAN NOT NULL DEFAULT FALSE).
        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("chapter_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .drop_column(Alias::new("chapter_lock"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .drop_column(Alias::new("chapter"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
