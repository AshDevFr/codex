use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Libraries::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Libraries::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Libraries::Name).string().not_null())
                    .col(ColumnDef::new(Libraries::Path).string().not_null())
                    // Series detection strategy (series_volume, series_volume_chapter, flat, etc.)
                    .col(
                        ColumnDef::new(Libraries::SeriesStrategy)
                            .string()
                            .not_null()
                            .default("series_volume"),
                    )
                    // Strategy-specific configuration (JSON)
                    .col(ColumnDef::new(Libraries::SeriesConfig).json())
                    // Book naming strategy (filename, metadata_first, smart, series_name)
                    .col(
                        ColumnDef::new(Libraries::BookStrategy)
                            .string()
                            .not_null()
                            .default("filename"),
                    )
                    // Book strategy-specific configuration (JSON)
                    .col(ColumnDef::new(Libraries::BookConfig).json())
                    // Book number strategy (file_order, metadata, filename, smart)
                    .col(
                        ColumnDef::new(Libraries::NumberStrategy)
                            .string()
                            .not_null()
                            .default("file_order"),
                    )
                    // Number strategy-specific configuration (JSON)
                    .col(ColumnDef::new(Libraries::NumberConfig).json())
                    // Legacy: kept for backward compatibility, stores cron/scan settings
                    .col(ColumnDef::new(Libraries::ScanningConfig).string())
                    .col(
                        ColumnDef::new(Libraries::DefaultReadingDirection)
                            .string()
                            .not_null()
                            .default("LEFT_TO_RIGHT"),
                    )
                    .col(ColumnDef::new(Libraries::AllowedFormats).string())
                    .col(ColumnDef::new(Libraries::ExcludedPatterns).text())
                    .col(
                        ColumnDef::new(Libraries::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Libraries::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Libraries::LastScannedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Libraries::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Libraries {
    Table,
    Id,
    Name,
    Path,
    SeriesStrategy,
    SeriesConfig,
    BookStrategy,
    BookConfig,
    NumberStrategy,
    NumberConfig,
    ScanningConfig,
    DefaultReadingDirection,
    AllowedFormats,
    ExcludedPatterns,
    CreatedAt,
    UpdatedAt,
    LastScannedAt,
}
