use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BookDuplicates::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BookDuplicates::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(BookDuplicates::FileHash).string().not_null())
                    .col(ColumnDef::new(BookDuplicates::BookIds).text().not_null())
                    .col(
                        ColumnDef::new(BookDuplicates::DuplicateCount)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BookDuplicates::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BookDuplicates::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Add index on file_hash for fast lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_book_duplicates_file_hash")
                    .table(BookDuplicates::Table)
                    .col(BookDuplicates::FileHash)
                    .to_owned(),
            )
            .await?;

        // Add index on duplicate_count for filtering/sorting
        manager
            .create_index(
                Index::create()
                    .name("idx_book_duplicates_count")
                    .table(BookDuplicates::Table)
                    .col(BookDuplicates::DuplicateCount)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BookDuplicates::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum BookDuplicates {
    Table,
    Id,
    FileHash,
    BookIds,
    DuplicateCount,
    CreatedAt,
    UpdatedAt,
}
