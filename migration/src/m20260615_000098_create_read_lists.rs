use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Read lists: shared, ordered groupings of books across series
        // (Komga-style "playlists for books").
        manager
            .create_table(
                Table::create()
                    .table(ReadLists::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ReadLists::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ReadLists::Name)
                            .string_len(255)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(ReadLists::NormalizedName)
                            .string_len(255)
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(ReadLists::Summary).text())
                    // Read lists default to manual reading order (the point of a
                    // read list); false => members sorted by release date.
                    .col(
                        ColumnDef::new(ReadLists::Ordered)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(ReadLists::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReadLists::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_read_lists_normalized_name")
                    .table(ReadLists::Table)
                    .col(ReadLists::NormalizedName)
                    .to_owned(),
            )
            .await?;

        // read_list_books: ordered membership.
        manager
            .create_table(
                Table::create()
                    .table(ReadListBooks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ReadListBooks::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ReadListBooks::ReadListId).uuid().not_null())
                    .col(ColumnDef::new(ReadListBooks::BookId).uuid().not_null())
                    // Honored only when read_lists.ordered = true.
                    .col(
                        ColumnDef::new(ReadListBooks::Position)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ReadListBooks::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_read_list_books_read_list_id")
                            .from(ReadListBooks::Table, ReadListBooks::ReadListId)
                            .to(ReadLists::Table, ReadLists::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_read_list_books_book_id")
                            .from(ReadListBooks::Table, ReadListBooks::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // A book appears at most once per read list.
        manager
            .create_index(
                Index::create()
                    .name("idx_read_list_books_unique")
                    .table(ReadListBooks::Table)
                    .col(ReadListBooks::ReadListId)
                    .col(ReadListBooks::BookId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Reverse lookup: which read lists contain a given book.
        manager
            .create_index(
                Index::create()
                    .name("idx_read_list_books_book_id")
                    .table(ReadListBooks::Table)
                    .col(ReadListBooks::BookId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ReadListBooks::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ReadLists::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum ReadLists {
    Table,
    Id,
    Name,
    NormalizedName,
    Summary,
    Ordered,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum ReadListBooks {
    Table,
    Id,
    ReadListId,
    BookId,
    Position,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}
