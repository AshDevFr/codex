use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Books::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Books::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Books::SeriesId).uuid().not_null())
                    .col(ColumnDef::new(Books::LibraryId).uuid().not_null())
                    .col(ColumnDef::new(Books::Title).string())
                    .col(ColumnDef::new(Books::Number).decimal())
                    .col(ColumnDef::new(Books::FilePath).string().not_null())
                    .col(ColumnDef::new(Books::FileName).string().not_null())
                    .col(ColumnDef::new(Books::FileSize).big_integer().not_null())
                    .col(ColumnDef::new(Books::FileHash).string().not_null())
                    .col(
                        ColumnDef::new(Books::PartialHash)
                            .string()
                            .not_null()
                            .default(""),
                    )
                    .col(ColumnDef::new(Books::Format).string().not_null())
                    .col(ColumnDef::new(Books::PageCount).integer().not_null())
                    .col(
                        ColumnDef::new(Books::Deleted)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Books::Analyzed)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Books::ModifiedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Books::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Books::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_books_series_id")
                            .from(Books::Table, Books::SeriesId)
                            .to(Series::Table, Series::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_books_library_id")
                            .from(Books::Table, Books::LibraryId)
                            .to(Libraries::Table, Libraries::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Add index on deleted column for performance
        manager
            .create_index(
                Index::create()
                    .name("idx_books_deleted")
                    .table(Books::Table)
                    .col(Books::Deleted)
                    .to_owned(),
            )
            .await?;

        // Add index on analyzed column for queued analysis queries
        manager
            .create_index(
                Index::create()
                    .name("idx_books_analyzed")
                    .table(Books::Table)
                    .col(Books::Analyzed)
                    .to_owned(),
            )
            .await?;

        // Add index on title for search performance
        manager
            .create_index(
                Index::create()
                    .name("idx_books_title")
                    .table(Books::Table)
                    .col(Books::Title)
                    .to_owned(),
            )
            .await?;

        // Add composite unique index on library_id + file_path
        // This ensures the same file path can only exist once per library
        manager
            .create_index(
                Index::create()
                    .name("idx_books_library_file_path_unique")
                    .table(Books::Table)
                    .col(Books::LibraryId)
                    .col(Books::FilePath)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Books::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
    SeriesId,
    LibraryId,
    Title,
    Number,
    FilePath,
    FileName,
    FileSize,
    FileHash,
    PartialHash,
    Format,
    PageCount,
    Deleted,
    Analyzed,
    ModifiedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Series {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Libraries {
    Table,
    Id,
}
