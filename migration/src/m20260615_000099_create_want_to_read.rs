use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // want_to_read: per-user, flat on-deck queue. Each row flags exactly one
        // series OR one book the user intends to read. Replaces the "open page 1
        // so it shows in Keep Reading" workaround.
        manager
            .create_table(
                Table::create()
                    .table(WantToRead::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WantToRead::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(WantToRead::UserId).uuid().not_null())
                    .col(ColumnDef::new(WantToRead::SeriesId).uuid())
                    .col(ColumnDef::new(WantToRead::BookId).uuid())
                    .col(
                        ColumnDef::new(WantToRead::AddedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    // Exactly one of series_id / book_id must be set. Rendered
                    // inline at CREATE TABLE so it holds on SQLite too (SQLite
                    // cannot ALTER TABLE ADD CONSTRAINT CHECK).
                    .check(Expr::cust(
                        "(series_id IS NOT NULL) <> (book_id IS NOT NULL)",
                    ))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_want_to_read_user_id")
                            .from(WantToRead::Table, WantToRead::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_want_to_read_series_id")
                            .from(WantToRead::Table, WantToRead::SeriesId)
                            .to(Series::Table, Series::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_want_to_read_book_id")
                            .from(WantToRead::Table, WantToRead::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Can't flag the same series twice for one user. NULLs are distinct in
        // unique indexes (both SQLite and PostgreSQL), so book-only rows (with
        // series_id NULL) don't collide.
        manager
            .create_index(
                Index::create()
                    .name("idx_want_to_read_user_series_unique")
                    .table(WantToRead::Table)
                    .col(WantToRead::UserId)
                    .col(WantToRead::SeriesId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Can't flag the same book twice for one user.
        manager
            .create_index(
                Index::create()
                    .name("idx_want_to_read_user_book_unique")
                    .table(WantToRead::Table)
                    .col(WantToRead::UserId)
                    .col(WantToRead::BookId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(WantToRead::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum WantToRead {
    Table,
    Id,
    UserId,
    SeriesId,
    BookId,
    AddedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Series {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}
