use sea_orm_migration::prelude::*;

use crate::m20260103_000007_create_genres::Genres;
use crate::m20260103_000008_create_tags::Tags;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create book_genres junction table
        manager
            .create_table(
                Table::create()
                    .table(BookGenres::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(BookGenres::BookId).uuid().not_null())
                    .col(ColumnDef::new(BookGenres::GenreId).uuid().not_null())
                    .primary_key(
                        Index::create()
                            .col(BookGenres::BookId)
                            .col(BookGenres::GenreId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_book_genres_book_id")
                            .from(BookGenres::Table, BookGenres::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_book_genres_genre_id")
                            .from(BookGenres::Table, BookGenres::GenreId)
                            .to(Genres::Table, Genres::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for filtering by genre
        manager
            .create_index(
                Index::create()
                    .name("idx_book_genres_genre_id")
                    .table(BookGenres::Table)
                    .col(BookGenres::GenreId)
                    .to_owned(),
            )
            .await?;

        // Create book_tags junction table
        manager
            .create_table(
                Table::create()
                    .table(BookTags::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(BookTags::BookId).uuid().not_null())
                    .col(ColumnDef::new(BookTags::TagId).uuid().not_null())
                    .primary_key(Index::create().col(BookTags::BookId).col(BookTags::TagId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_book_tags_book_id")
                            .from(BookTags::Table, BookTags::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_book_tags_tag_id")
                            .from(BookTags::Table, BookTags::TagId)
                            .to(Tags::Table, Tags::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for filtering by tag
        manager
            .create_index(
                Index::create()
                    .name("idx_book_tags_tag_id")
                    .table(BookTags::Table)
                    .col(BookTags::TagId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BookTags::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(BookGenres::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum BookGenres {
    Table,
    BookId,
    GenreId,
}

#[derive(DeriveIden)]
enum BookTags {
    Table,
    BookId,
    TagId,
}
