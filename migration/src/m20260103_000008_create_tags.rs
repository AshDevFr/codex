use sea_orm_migration::prelude::*;

use crate::m20260103_000003_create_series::Series;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create tags taxonomy table
        manager
            .create_table(
                Table::create()
                    .table(Tags::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Tags::Id).uuid().not_null().primary_key())
                    .col(
                        ColumnDef::new(Tags::Name)
                            .string_len(100)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Tags::NormalizedName)
                            .string_len(100)
                            .not_null()
                            .unique_key(),
                    ) // lowercase for matching
                    .col(
                        ColumnDef::new(Tags::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for fast lookup by normalized name
        manager
            .create_index(
                Index::create()
                    .name("idx_tags_normalized_name")
                    .table(Tags::Table)
                    .col(Tags::NormalizedName)
                    .to_owned(),
            )
            .await?;

        // Create series_tags junction table
        manager
            .create_table(
                Table::create()
                    .table(SeriesTags::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(SeriesTags::SeriesId).uuid().not_null())
                    .col(ColumnDef::new(SeriesTags::TagId).uuid().not_null())
                    .primary_key(
                        Index::create()
                            .col(SeriesTags::SeriesId)
                            .col(SeriesTags::TagId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_series_tags_series_id")
                            .from(SeriesTags::Table, SeriesTags::SeriesId)
                            .to(Series::Table, Series::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_series_tags_tag_id")
                            .from(SeriesTags::Table, SeriesTags::TagId)
                            .to(Tags::Table, Tags::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for filtering by tag
        manager
            .create_index(
                Index::create()
                    .name("idx_series_tags_tag_id")
                    .table(SeriesTags::Table)
                    .col(SeriesTags::TagId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SeriesTags::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Tags::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Tags {
    Table,
    Id,
    Name,
    NormalizedName,
    CreatedAt,
}

#[derive(DeriveIden)]
pub enum SeriesTags {
    Table,
    SeriesId,
    TagId,
}
