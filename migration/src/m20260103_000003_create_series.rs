use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create slim series table - core identity only
        // Rich metadata is in series_metadata table (1:1 relationship)
        manager
            .create_table(
                Table::create()
                    .table(Series::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Series::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Series::LibraryId).uuid().not_null())
                    .col(ColumnDef::new(Series::Name).string_len(500).not_null())
                    .col(
                        ColumnDef::new(Series::NormalizedName)
                            .string_len(500)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Series::BookCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(Series::Fingerprint).string_len(64))
                    .col(ColumnDef::new(Series::Path).text())
                    .col(ColumnDef::new(Series::CustomMetadata).text()) // JSON escape hatch for user-defined fields
                    .col(
                        ColumnDef::new(Series::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Series::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_series_library_id")
                            .from(Series::Table, Series::LibraryId)
                            .to(Libraries::Table, Libraries::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Index on normalized_name for search performance
        manager
            .create_index(
                Index::create()
                    .name("idx_series_normalized_name")
                    .table(Series::Table)
                    .col(Series::NormalizedName)
                    .to_owned(),
            )
            .await?;

        // Index on library_id for filtering
        manager
            .create_index(
                Index::create()
                    .name("idx_series_library_id")
                    .table(Series::Table)
                    .col(Series::LibraryId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Series::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Series {
    Table,
    Id,
    LibraryId,
    Name,
    NormalizedName,
    BookCount,
    Fingerprint,
    Path,
    CustomMetadata,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Libraries {
    Table,
    Id,
}
