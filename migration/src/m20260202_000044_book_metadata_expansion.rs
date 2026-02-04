//! Expand book_metadata table with new fields for enhanced book metadata
//!
//! This migration adds:
//! - book_type: Classification (comic, manga, novel, etc.)
//! - subtitle: Book subtitle
//! - authors_json: Structured author information as JSON
//! - translator: Translator name
//! - edition: Edition information
//! - original_title: Original title for translated works
//! - original_year: Original publication year
//! - series_position: Position in a series (decimal for .5 volumes)
//! - series_total: Total books in series
//! - subjects: Subject/topic tags
//! - awards_json: Awards as JSON
//! - custom_metadata: JSON escape hatch
//! - cover_lock: Lock cover to prevent auto-updates
//! - Lock fields for all new metadata fields

use sea_orm_migration::prelude::*;

use crate::m20260103_000014_create_book_metadata::BookMetadata;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add new metadata fields
        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    // Book type classification
                    .add_column(ColumnDef::new(Alias::new("book_type")).string_len(50))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(ColumnDef::new(Alias::new("subtitle")).string_len(500))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    // JSON array of authors with roles
                    .add_column(ColumnDef::new(Alias::new("authors_json")).text())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(ColumnDef::new(Alias::new("translator")).string_len(255))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(ColumnDef::new(Alias::new("edition")).string_len(100))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(ColumnDef::new(Alias::new("original_title")).string_len(500))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(ColumnDef::new(Alias::new("original_year")).integer())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    // Decimal for .5 volumes
                    .add_column(ColumnDef::new(Alias::new("series_position")).decimal())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(ColumnDef::new(Alias::new("series_total")).integer())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    // Comma-separated or JSON array of subjects
                    .add_column(ColumnDef::new(Alias::new("subjects")).text())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    // JSON array of awards
                    .add_column(ColumnDef::new(Alias::new("awards_json")).text())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    // JSON escape hatch for user-defined fields
                    .add_column(ColumnDef::new(Alias::new("custom_metadata")).text())
                    .to_owned(),
            )
            .await?;

        // Add lock fields for all new metadata fields
        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("book_type_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("subtitle_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("authors_json_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("translator_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("edition_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("original_title_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("original_year_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("series_position_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("series_total_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("subjects_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("awards_json_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("custom_metadata_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        // Add cover_lock (mirrors series_metadata.cover_lock)
        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("cover_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop all new columns in reverse order
        let columns_to_drop = [
            "cover_lock",
            "custom_metadata_lock",
            "awards_json_lock",
            "subjects_lock",
            "series_total_lock",
            "series_position_lock",
            "original_year_lock",
            "original_title_lock",
            "edition_lock",
            "translator_lock",
            "authors_json_lock",
            "subtitle_lock",
            "book_type_lock",
            "custom_metadata",
            "awards_json",
            "subjects",
            "series_total",
            "series_position",
            "original_year",
            "original_title",
            "edition",
            "translator",
            "authors_json",
            "subtitle",
            "book_type",
        ];

        for column in columns_to_drop {
            manager
                .alter_table(
                    Table::alter()
                        .table(BookMetadata::Table)
                        .drop_column(Alias::new(column))
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }
}
