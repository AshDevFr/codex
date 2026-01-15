use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ReadProgress::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ReadProgress::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ReadProgress::UserId).uuid().not_null())
                    .col(ColumnDef::new(ReadProgress::BookId).uuid().not_null())
                    .col(integer(ReadProgress::CurrentPage))
                    .col(ColumnDef::new(ReadProgress::ProgressPercentage).double())
                    .col(boolean(ReadProgress::Completed))
                    .col(
                        ColumnDef::new(ReadProgress::StartedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReadProgress::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ReadProgress::CompletedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_read_progress_user_id")
                            .from(ReadProgress::Table, ReadProgress::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_read_progress_book_id")
                            .from(ReadProgress::Table, ReadProgress::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint on (user_id, book_id) - only one progress record per user per book
        manager
            .create_index(
                Index::create()
                    .name("idx_read_progress_user_book_unique")
                    .table(ReadProgress::Table)
                    .col(ReadProgress::UserId)
                    .col(ReadProgress::BookId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ReadProgress::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ReadProgress {
    Table,
    Id,
    UserId,
    BookId,
    CurrentPage,
    ProgressPercentage,
    Completed,
    StartedAt,
    UpdatedAt,
    CompletedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}
