use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BookMetadataRecords::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BookMetadataRecords::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(BookMetadataRecords::BookId)
                            .uuid()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(BookMetadataRecords::Summary).text())
                    .col(ColumnDef::new(BookMetadataRecords::Writer).string())
                    .col(ColumnDef::new(BookMetadataRecords::Penciller).string())
                    .col(ColumnDef::new(BookMetadataRecords::Inker).string())
                    .col(ColumnDef::new(BookMetadataRecords::Colorist).string())
                    .col(ColumnDef::new(BookMetadataRecords::Letterer).string())
                    .col(ColumnDef::new(BookMetadataRecords::CoverArtist).string())
                    .col(ColumnDef::new(BookMetadataRecords::Editor).string())
                    .col(ColumnDef::new(BookMetadataRecords::Publisher).string())
                    .col(ColumnDef::new(BookMetadataRecords::Imprint).string())
                    .col(ColumnDef::new(BookMetadataRecords::Genre).string())
                    .col(ColumnDef::new(BookMetadataRecords::Web).string())
                    .col(ColumnDef::new(BookMetadataRecords::LanguageIso).string())
                    .col(ColumnDef::new(BookMetadataRecords::FormatDetail).string())
                    .col(ColumnDef::new(BookMetadataRecords::BlackAndWhite).boolean())
                    .col(ColumnDef::new(BookMetadataRecords::Manga).boolean())
                    .col(ColumnDef::new(BookMetadataRecords::Year).integer())
                    .col(ColumnDef::new(BookMetadataRecords::Month).integer())
                    .col(ColumnDef::new(BookMetadataRecords::Day).integer())
                    .col(ColumnDef::new(BookMetadataRecords::Volume).integer())
                    .col(ColumnDef::new(BookMetadataRecords::Count).integer())
                    .col(ColumnDef::new(BookMetadataRecords::Isbns).string())
                    .col(
                        ColumnDef::new(BookMetadataRecords::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BookMetadataRecords::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_book_metadata_records_book_id")
                            .from(BookMetadataRecords::Table, BookMetadataRecords::BookId)
                            .to(Books::Table, Books::Id)
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
            .drop_table(Table::drop().table(BookMetadataRecords::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum BookMetadataRecords {
    Table,
    Id,
    BookId,
    Summary,
    Writer,
    Penciller,
    Inker,
    Colorist,
    Letterer,
    CoverArtist,
    Editor,
    Publisher,
    Imprint,
    Genre,
    Web,
    LanguageIso,
    FormatDetail,
    BlackAndWhite,
    Manga,
    Year,
    Month,
    Day,
    Volume,
    Count,
    Isbns,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}

