use sea_orm_migration::prelude::*;

use crate::m20260103_000003_create_series::Series;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create series_metadata table (1:1 with series)
        // Contains all descriptive metadata with lock fields
        manager
            .create_table(
                Table::create()
                    .table(SeriesMetadata::Table)
                    .if_not_exists()
                    // Primary key is the series_id (1:1 relationship)
                    .col(
                        ColumnDef::new(SeriesMetadata::SeriesId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    // Descriptive metadata fields
                    .col(
                        ColumnDef::new(SeriesMetadata::Title)
                            .string_len(500)
                            .not_null(),
                    )
                    .col(ColumnDef::new(SeriesMetadata::TitleSort).string_len(500))
                    .col(ColumnDef::new(SeriesMetadata::Summary).text())
                    .col(ColumnDef::new(SeriesMetadata::Publisher).string_len(255))
                    .col(ColumnDef::new(SeriesMetadata::Imprint).string_len(255))
                    .col(
                        ColumnDef::new(SeriesMetadata::Status)
                            .string_len(20)
                            .default("ongoing"),
                    ) // ongoing, ended, hiatus, abandoned, unknown
                    .col(ColumnDef::new(SeriesMetadata::AgeRating).integer()) // e.g., 13, 16, 18
                    .col(ColumnDef::new(SeriesMetadata::Language).string_len(10)) // BCP47: "en", "ja", "ko"
                    .col(ColumnDef::new(SeriesMetadata::ReadingDirection).string_len(10)) // ltr, rtl, ttb, btt
                    .col(ColumnDef::new(SeriesMetadata::Year).integer())
                    .col(ColumnDef::new(SeriesMetadata::TotalBookCount).integer()) // Expected total (for ongoing series)
                    // Lock fields (prevent auto-refresh from overwriting user edits)
                    .col(
                        ColumnDef::new(SeriesMetadata::TitleLock)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(SeriesMetadata::TitleSortLock)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(SeriesMetadata::SummaryLock)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(SeriesMetadata::PublisherLock)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(SeriesMetadata::ImprintLock)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(SeriesMetadata::StatusLock)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(SeriesMetadata::AgeRatingLock)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(SeriesMetadata::LanguageLock)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(SeriesMetadata::ReadingDirectionLock)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(SeriesMetadata::YearLock)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(SeriesMetadata::GenresLock)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(SeriesMetadata::TagsLock)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    // Timestamps
                    .col(
                        ColumnDef::new(SeriesMetadata::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeriesMetadata::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_series_metadata_series_id")
                            .from(SeriesMetadata::Table, SeriesMetadata::SeriesId)
                            .to(Series::Table, Series::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SeriesMetadata::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum SeriesMetadata {
    Table,
    SeriesId,
    Title,
    TitleSort,
    Summary,
    Publisher,
    Imprint,
    Status,
    AgeRating,
    Language,
    ReadingDirection,
    Year,
    TotalBookCount,
    // Lock fields
    TitleLock,
    TitleSortLock,
    SummaryLock,
    PublisherLock,
    ImprintLock,
    StatusLock,
    AgeRatingLock,
    LanguageLock,
    ReadingDirectionLock,
    YearLock,
    GenresLock,
    TagsLock,
    // Timestamps
    CreatedAt,
    UpdatedAt,
}
