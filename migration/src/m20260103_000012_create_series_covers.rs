use sea_orm_migration::prelude::*;

use crate::m20260103_000003_create_series::Series;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create series_covers table (1:N with series)
        // Stores multiple cover images per series with one selected as primary
        manager
            .create_table(
                Table::create()
                    .table(SeriesCovers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SeriesCovers::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SeriesCovers::SeriesId).uuid().not_null())
                    .col(
                        ColumnDef::new(SeriesCovers::Source)
                            .string_len(50)
                            .not_null(),
                    ) // "book:uuid", "custom", "mangabaka"
                    .col(ColumnDef::new(SeriesCovers::Path).text().not_null())
                    .col(
                        ColumnDef::new(SeriesCovers::IsSelected)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(SeriesCovers::Width).integer())
                    .col(ColumnDef::new(SeriesCovers::Height).integer())
                    .col(
                        ColumnDef::new(SeriesCovers::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeriesCovers::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_series_covers_series_id")
                            .from(SeriesCovers::Table, SeriesCovers::SeriesId)
                            .to(Series::Table, Series::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for looking up covers by series
        manager
            .create_index(
                Index::create()
                    .name("idx_series_covers_series_id")
                    .table(SeriesCovers::Table)
                    .col(SeriesCovers::SeriesId)
                    .to_owned(),
            )
            .await?;

        // Partial index for finding selected cover quickly
        // Note: SQLite doesn't support partial indexes via SeaORM, so we create a regular index
        manager
            .create_index(
                Index::create()
                    .name("idx_series_covers_selected")
                    .table(SeriesCovers::Table)
                    .col(SeriesCovers::SeriesId)
                    .col(SeriesCovers::IsSelected)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SeriesCovers::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum SeriesCovers {
    Table,
    Id,
    SeriesId,
    Source,
    Path,
    IsSelected,
    Width,
    Height,
    CreatedAt,
    UpdatedAt,
}
