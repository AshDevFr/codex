use sea_orm_migration::prelude::*;

use crate::m20260103_000003_create_series::Series;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create user_series_ratings table (N:N via user)
        // Stores per-user ratings (1-100) with optional notes
        manager
            .create_table(
                Table::create()
                    .table(UserSeriesRatings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserSeriesRatings::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UserSeriesRatings::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(UserSeriesRatings::SeriesId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserSeriesRatings::Rating)
                            .integer()
                            .not_null(),
                    ) // 1-100 (displayed as 1-10 in UI with 0.1 precision)
                    .col(ColumnDef::new(UserSeriesRatings::Notes).text())
                    .col(
                        ColumnDef::new(UserSeriesRatings::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserSeriesRatings::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_series_ratings_user_id")
                            .from(UserSeriesRatings::Table, UserSeriesRatings::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_series_ratings_series_id")
                            .from(UserSeriesRatings::Table, UserSeriesRatings::SeriesId)
                            .to(Series::Table, Series::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint: one rating per user per series
        manager
            .create_index(
                Index::create()
                    .name("idx_user_series_ratings_unique")
                    .table(UserSeriesRatings::Table)
                    .col(UserSeriesRatings::UserId)
                    .col(UserSeriesRatings::SeriesId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index for looking up ratings by user (for "my ratings" list)
        manager
            .create_index(
                Index::create()
                    .name("idx_user_series_ratings_user_id")
                    .table(UserSeriesRatings::Table)
                    .col(UserSeriesRatings::UserId)
                    .to_owned(),
            )
            .await?;

        // Index for looking up ratings by series
        manager
            .create_index(
                Index::create()
                    .name("idx_user_series_ratings_series_id")
                    .table(UserSeriesRatings::Table)
                    .col(UserSeriesRatings::SeriesId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserSeriesRatings::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum UserSeriesRatings {
    Table,
    Id,
    UserId,
    SeriesId,
    Rating,
    Notes,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
