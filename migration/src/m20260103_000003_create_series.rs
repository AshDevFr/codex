use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Series::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Series::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Series::LibraryId).uuid().not_null())
                    .col(ColumnDef::new(Series::Name).string().not_null())
                    .col(ColumnDef::new(Series::NormalizedName).string().not_null())
                    .col(ColumnDef::new(Series::SortName).string())
                    .col(ColumnDef::new(Series::Summary).text())
                    .col(ColumnDef::new(Series::Publisher).string())
                    .col(ColumnDef::new(Series::Year).integer())
                    .col(ColumnDef::new(Series::BookCount).integer().not_null())
                    .col(ColumnDef::new(Series::UserRating).decimal())
                    .col(ColumnDef::new(Series::ExternalRating).decimal())
                    .col(ColumnDef::new(Series::ExternalRatingCount).integer())
                    .col(ColumnDef::new(Series::ExternalRatingSource).string())
                    .col(ColumnDef::new(Series::CustomMetadata).text())
                    .col(ColumnDef::new(Series::Fingerprint).string())
                    .col(ColumnDef::new(Series::Path).string())
                    .col(ColumnDef::new(Series::ReadingDirection).string())
                    .col(ColumnDef::new(Series::CustomCoverPath).string())
                    .col(ColumnDef::new(Series::SelectedCoverSource).string())
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

        // Add index on normalized_name for search performance
        manager
            .create_index(
                Index::create()
                    .name("idx_series_normalized_name")
                    .table(Series::Table)
                    .col(Series::NormalizedName)
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
enum Series {
    Table,
    Id,
    LibraryId,
    Name,
    NormalizedName,
    SortName,
    Summary,
    Publisher,
    Year,
    BookCount,
    UserRating,
    ExternalRating,
    ExternalRatingCount,
    ExternalRatingSource,
    CustomMetadata,
    Fingerprint,
    Path,
    ReadingDirection,
    CustomCoverPath,
    SelectedCoverSource,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Libraries {
    Table,
    Id,
}
