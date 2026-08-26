//! Per-user, per-series reader settings.
//!
//! These lived in `localStorage`, which meant they died with a browser profile
//! and never followed a reader to a second device. They also had no way to
//! express "this series is wrong for me" without editing the series metadata
//! every other user sees, which required a permission most readers do not have.
//!
//! Shaped after `user_series_ratings`: the same user/series pair, the same
//! cascade on both sides, and the same unique index. `settings` holds a sparse
//! JSON object so untouched settings keep inheriting from the series metadata
//! and the library default.

use sea_orm_migration::prelude::*;

use crate::m20260103_000003_create_series::Series;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserSeriesReaderSettings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserSeriesReaderSettings::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UserSeriesReaderSettings::UserId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserSeriesReaderSettings::SeriesId)
                            .uuid()
                            .not_null(),
                    )
                    // Sparse: only the keys the user actually overrode.
                    .col(
                        ColumnDef::new(UserSeriesReaderSettings::Settings)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserSeriesReaderSettings::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserSeriesReaderSettings::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_series_reader_settings_user_id")
                            .from(
                                UserSeriesReaderSettings::Table,
                                UserSeriesReaderSettings::UserId,
                            )
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_series_reader_settings_series_id")
                            .from(
                                UserSeriesReaderSettings::Table,
                                UserSeriesReaderSettings::SeriesId,
                            )
                            .to(Series::Table, Series::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // One record per user per series.
        manager
            .create_index(
                Index::create()
                    .name("idx_user_series_reader_settings_unique")
                    .table(UserSeriesReaderSettings::Table)
                    .col(UserSeriesReaderSettings::UserId)
                    .col(UserSeriesReaderSettings::SeriesId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Book listings resolve reading direction for one user across the
        // series on the page, so the batch lookup is keyed by user.
        manager
            .create_index(
                Index::create()
                    .name("idx_user_series_reader_settings_user_id")
                    .table(UserSeriesReaderSettings::Table)
                    .col(UserSeriesReaderSettings::UserId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(UserSeriesReaderSettings::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
pub enum UserSeriesReaderSettings {
    Table,
    Id,
    UserId,
    SeriesId,
    Settings,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
