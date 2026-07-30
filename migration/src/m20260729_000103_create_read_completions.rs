//! Create the `read_completions` table: an append-only log of finished
//! read-throughs.
//!
//! `read_progress` holds the *current* pass and is deleted outright when a book
//! is marked unread, which is correct for progress but means the fact that the
//! book was ever finished is destroyed with it. Banking each completion in a
//! separate table lets mark-unread keep deleting the progress row while the
//! reading history survives, so re-reading a series stops being a destructive
//! act.
//!
//! Rows are only ever inserted and deleted, never updated: every row is one
//! completed pass, which keeps "how many times have I read this" a plain
//! `COUNT`.
//!
//! Both indexes exist for a reason:
//!
//! * `(user_id, book_id)` serves the per-book count and timeline, and the
//!   duplicate guard that runs on each completion.
//! * `(user_id, completed_at DESC)` serves a chronological history for one user
//!   without a sort.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ReadCompletions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ReadCompletions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ReadCompletions::UserId).uuid().not_null())
                    .col(ColumnDef::new(ReadCompletions::BookId).uuid().not_null())
                    .col(
                        ColumnDef::new(ReadCompletions::StartedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    // Not nullable, unlike `read_progress.completed_at`: a row
                    // only exists because a pass finished, so there is always a
                    // date. The backfill coalesces legacy NULLs.
                    .col(
                        ColumnDef::new(ReadCompletions::CompletedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_read_completions_user_id")
                            .from(ReadCompletions::Table, ReadCompletions::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_read_completions_book_id")
                            .from(ReadCompletions::Table, ReadCompletions::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Per-book history and the duplicate guard. Deliberately not unique: a
        // book can legitimately be completed many times.
        manager
            .create_index(
                Index::create()
                    .name("idx_read_completions_user_book")
                    .table(ReadCompletions::Table)
                    .col(ReadCompletions::UserId)
                    .col(ReadCompletions::BookId)
                    .to_owned(),
            )
            .await?;

        // Chronological history for one user.
        manager
            .create_index(
                Index::create()
                    .name("idx_read_completions_user_date")
                    .table(ReadCompletions::Table)
                    .col(ReadCompletions::UserId)
                    .col((ReadCompletions::CompletedAt, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ReadCompletions::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum ReadCompletions {
    Table,
    Id,
    UserId,
    BookId,
    StartedAt,
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
