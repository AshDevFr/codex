//! Split `series_metadata.total_book_count` into `total_volume_count` and `total_chapter_count`.
//!
//! Phase 1 of the metadata-count-split plan: adds new columns + locks and backfills
//! the new volume column from the existing single book count, preserving the lock state.
//! The legacy `total_book_count` column stays in place until Phase 9 (hard removal).
//!
//! Why: `total_book_count` is overloaded (volumes, chapters, or whatever). Splitting it
//! lets chapter-organized libraries show real "behind by N" indicators against provider
//! data and lets mixed-format libraries be modeled correctly.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

use crate::m20260103_000006_create_series_metadata::SeriesMetadata;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Step 1: add total_volume_count (INTEGER NULL).
        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .add_column(ColumnDef::new(Alias::new("total_volume_count")).integer())
                    .to_owned(),
            )
            .await?;

        // Step 2: add total_volume_count_lock (BOOLEAN NOT NULL DEFAULT FALSE).
        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("total_volume_count_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        // Step 3: add total_chapter_count (REAL/FLOAT NULL). Chapters can be fractional
        // (e.g. 47.5, 100.5) so a float type is required; integer would be lossy.
        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .add_column(ColumnDef::new(Alias::new("total_chapter_count")).float())
                    .to_owned(),
            )
            .await?;

        // Step 4: add total_chapter_count_lock (BOOLEAN NOT NULL DEFAULT FALSE).
        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("total_chapter_count_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        // Step 5: backfill. Existing total_book_count data is overwhelmingly volume-shaped
        // (most providers and library organizations are volume-oriented), so copy values
        // and locks into the volume columns. Chapter-organized users who emptied + locked
        // total_book_count land on total_volume_count = NULL, lock = true: exactly the
        // semantically clean state they wanted. Chapter columns stay NULL/false until
        // a future metadata refresh populates them from a provider.
        let db = manager.get_connection();
        db.execute(Statement::from_string(
            manager.get_database_backend(),
            "UPDATE series_metadata
                 SET total_volume_count      = total_book_count,
                     total_volume_count_lock = total_book_count_lock
                 WHERE total_book_count IS NOT NULL
                    OR total_book_count_lock = TRUE"
                .to_owned(),
        ))
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Reverse the column additions in opposite order. No data restore needed:
        // the legacy total_book_count column is untouched in up(), so it still holds
        // the original values.
        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .drop_column(Alias::new("total_chapter_count_lock"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .drop_column(Alias::new("total_chapter_count"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .drop_column(Alias::new("total_volume_count_lock"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .drop_column(Alias::new("total_volume_count"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
