use sea_orm_migration::prelude::*;

use crate::m20260103_000003_create_series::Series;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create genres taxonomy table
        manager
            .create_table(
                Table::create()
                    .table(Genres::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Genres::Id).uuid().not_null().primary_key())
                    .col(
                        ColumnDef::new(Genres::Name)
                            .string_len(100)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Genres::NormalizedName)
                            .string_len(100)
                            .not_null()
                            .unique_key(),
                    ) // lowercase for matching
                    .col(
                        ColumnDef::new(Genres::CreatedAt)
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
                    .name("idx_genres_normalized_name")
                    .table(Genres::Table)
                    .col(Genres::NormalizedName)
                    .to_owned(),
            )
            .await?;

        // Create series_genres junction table
        manager
            .create_table(
                Table::create()
                    .table(SeriesGenres::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(SeriesGenres::SeriesId).uuid().not_null())
                    .col(ColumnDef::new(SeriesGenres::GenreId).uuid().not_null())
                    .primary_key(
                        Index::create()
                            .col(SeriesGenres::SeriesId)
                            .col(SeriesGenres::GenreId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_series_genres_series_id")
                            .from(SeriesGenres::Table, SeriesGenres::SeriesId)
                            .to(Series::Table, Series::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_series_genres_genre_id")
                            .from(SeriesGenres::Table, SeriesGenres::GenreId)
                            .to(Genres::Table, Genres::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for filtering by genre
        manager
            .create_index(
                Index::create()
                    .name("idx_series_genres_genre_id")
                    .table(SeriesGenres::Table)
                    .col(SeriesGenres::GenreId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SeriesGenres::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Genres::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Genres {
    Table,
    Id,
    Name,
    NormalizedName,
    CreatedAt,
}

#[derive(DeriveIden)]
pub enum SeriesGenres {
    Table,
    SeriesId,
    GenreId,
}
