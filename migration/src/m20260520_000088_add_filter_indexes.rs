use sea_orm_migration::prelude::*;

/// Indexes that back the new `BookCondition::Format` and
/// `BookCondition::PageCount` filter variants added with the advanced search
/// work. `series_metadata.year` already has an index from the sorting-indexes
/// migration so it is not added again here. Path-substring filtering uses
/// `LIKE '%...%'` which cannot benefit from a standard B-tree index, so we
/// deliberately skip indexing `books.file_path`.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_books_format")
                    .table(Books::Table)
                    .col(Books::Format)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_books_page_count")
                    .table(Books::Table)
                    .col(Books::PageCount)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_books_page_count")
                    .table(Books::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_books_format")
                    .table(Books::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Format,
    PageCount,
}
