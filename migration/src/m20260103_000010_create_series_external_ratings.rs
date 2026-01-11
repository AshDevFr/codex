use sea_orm_migration::prelude::*;

use crate::m20260103_000003_create_series::Series;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create series_external_ratings table (1:N with series)
        // Stores ratings from external sources like MangaBaka, MAL, AniList
        manager
            .create_table(
                Table::create()
                    .table(SeriesExternalRatings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SeriesExternalRatings::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SeriesExternalRatings::SeriesId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeriesExternalRatings::SourceName)
                            .string_len(50)
                            .not_null(),
                    ) // "mangabaka", "myanimelist", "anilist"
                    .col(
                        ColumnDef::new(SeriesExternalRatings::Rating)
                            .decimal()
                            .not_null(),
                    ) // Normalized to 0-100
                    .col(ColumnDef::new(SeriesExternalRatings::VoteCount).integer())
                    .col(
                        ColumnDef::new(SeriesExternalRatings::FetchedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeriesExternalRatings::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeriesExternalRatings::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_series_external_ratings_series_id")
                            .from(
                                SeriesExternalRatings::Table,
                                SeriesExternalRatings::SeriesId,
                            )
                            .to(Series::Table, Series::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint: one rating per source per series
        manager
            .create_index(
                Index::create()
                    .name("idx_series_external_ratings_unique")
                    .table(SeriesExternalRatings::Table)
                    .col(SeriesExternalRatings::SeriesId)
                    .col(SeriesExternalRatings::SourceName)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index for looking up ratings by series
        manager
            .create_index(
                Index::create()
                    .name("idx_series_external_ratings_series_id")
                    .table(SeriesExternalRatings::Table)
                    .col(SeriesExternalRatings::SeriesId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SeriesExternalRatings::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum SeriesExternalRatings {
    Table,
    Id,
    SeriesId,
    SourceName,
    Rating,
    VoteCount,
    FetchedAt,
    CreatedAt,
    UpdatedAt,
}
