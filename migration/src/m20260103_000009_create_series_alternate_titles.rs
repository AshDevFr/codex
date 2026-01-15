use sea_orm_migration::prelude::*;

use crate::m20260103_000003_create_series::Series;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create series_alternate_titles table (1:N with series)
        // Stores multiple titles per series (Japanese, Romaji, English, Korean, etc.)
        manager
            .create_table(
                Table::create()
                    .table(SeriesAlternateTitles::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SeriesAlternateTitles::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SeriesAlternateTitles::SeriesId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeriesAlternateTitles::Label)
                            .string_len(100)
                            .not_null(),
                    ) // "Japanese", "Romaji", "English", "Korean"
                    .col(
                        ColumnDef::new(SeriesAlternateTitles::Title)
                            .string_len(500)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeriesAlternateTitles::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeriesAlternateTitles::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_series_alternate_titles_series_id")
                            .from(
                                SeriesAlternateTitles::Table,
                                SeriesAlternateTitles::SeriesId,
                            )
                            .to(Series::Table, Series::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for looking up alternate titles by series
        manager
            .create_index(
                Index::create()
                    .name("idx_series_alternate_titles_series_id")
                    .table(SeriesAlternateTitles::Table)
                    .col(SeriesAlternateTitles::SeriesId)
                    .to_owned(),
            )
            .await?;

        // Unique constraint on (series_id, label) - only one title per label per series
        manager
            .create_index(
                Index::create()
                    .name("idx_series_alternate_titles_unique")
                    .table(SeriesAlternateTitles::Table)
                    .col(SeriesAlternateTitles::SeriesId)
                    .col(SeriesAlternateTitles::Label)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SeriesAlternateTitles::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum SeriesAlternateTitles {
    Table,
    Id,
    SeriesId,
    Label,
    Title,
    CreatedAt,
    UpdatedAt,
}
