use sea_orm_migration::prelude::*;

use crate::m20260103_000003_create_series::Series;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create series_external_links table (1:N with series)
        // Stores links to external sources like MangaBaka, MAL, MangaDex
        manager
            .create_table(
                Table::create()
                    .table(SeriesExternalLinks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SeriesExternalLinks::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SeriesExternalLinks::SeriesId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeriesExternalLinks::SourceName)
                            .string_len(50)
                            .not_null(),
                    ) // "mangabaka", "myanimelist", "mangadex"
                    .col(ColumnDef::new(SeriesExternalLinks::Url).text().not_null())
                    .col(ColumnDef::new(SeriesExternalLinks::ExternalId).string_len(100)) // ID on the external site
                    .col(
                        ColumnDef::new(SeriesExternalLinks::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeriesExternalLinks::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_series_external_links_series_id")
                            .from(SeriesExternalLinks::Table, SeriesExternalLinks::SeriesId)
                            .to(Series::Table, Series::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint: one link per source per series
        manager
            .create_index(
                Index::create()
                    .name("idx_series_external_links_unique")
                    .table(SeriesExternalLinks::Table)
                    .col(SeriesExternalLinks::SeriesId)
                    .col(SeriesExternalLinks::SourceName)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index for looking up links by series
        manager
            .create_index(
                Index::create()
                    .name("idx_series_external_links_series_id")
                    .table(SeriesExternalLinks::Table)
                    .col(SeriesExternalLinks::SeriesId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SeriesExternalLinks::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum SeriesExternalLinks {
    Table,
    Id,
    SeriesId,
    SourceName,
    Url,
    ExternalId,
    CreatedAt,
    UpdatedAt,
}
