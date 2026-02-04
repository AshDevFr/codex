use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create book_external_links table (1:N with books)
        // Stores links to external sources like Open Library, Goodreads, Amazon
        // Mirrors series_external_links pattern
        manager
            .create_table(
                Table::create()
                    .table(BookExternalLinks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BookExternalLinks::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(BookExternalLinks::BookId).uuid().not_null())
                    .col(
                        ColumnDef::new(BookExternalLinks::SourceName)
                            .string_len(50)
                            .not_null(),
                    ) // "openlibrary", "goodreads", "amazon"
                    .col(ColumnDef::new(BookExternalLinks::Url).text().not_null())
                    .col(ColumnDef::new(BookExternalLinks::ExternalId).string_len(100)) // ID on the external site
                    .col(
                        ColumnDef::new(BookExternalLinks::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BookExternalLinks::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_book_external_links_book_id")
                            .from(BookExternalLinks::Table, BookExternalLinks::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint: one link per source per book
        manager
            .create_index(
                Index::create()
                    .name("idx_book_external_links_unique")
                    .table(BookExternalLinks::Table)
                    .col(BookExternalLinks::BookId)
                    .col(BookExternalLinks::SourceName)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index for looking up links by book
        manager
            .create_index(
                Index::create()
                    .name("idx_book_external_links_book_id")
                    .table(BookExternalLinks::Table)
                    .col(BookExternalLinks::BookId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BookExternalLinks::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum BookExternalLinks {
    Table,
    Id,
    BookId,
    SourceName,
    Url,
    ExternalId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}
