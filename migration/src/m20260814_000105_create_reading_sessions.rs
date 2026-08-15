//! Create the `reading_sessions` table: an append-only log of reading activity
//! from which `read_progress` and `read_completions` are derived.
//!
//! `read_progress` stores where a reader ended up, which is derived state. It
//! carries none of the facts that produced it, so two clients writing the same
//! row can only be reconciled by picking a winner rather than by merging. The
//! defect that forces the change is a client which read further syncing first
//! and then being clobbered by a stale client that syncs later: a bare page
//! number cannot distinguish "newest arrival" from "furthest read".
//!
//! Recording the activity instead makes that distinction available. Ordering is
//! by `client_ended_at`, when the reading actually happened, rather than by
//! arrival, so a late-arriving stale session sorts before the fresher one it
//! would otherwise overwrite. `server_recorded_at` is only a tiebreak, and is
//! stamped by the server so a device with a skewed clock cannot win on ordering
//! alone.
//!
//! Making mark-unread a `reset` row in this same log is what fixes completion
//! races: the ordering between "I finished this" and "I am starting over"
//! becomes recorded data instead of an accident of arrival order.
//!
//! Rows are only ever inserted, with one exception: an append that lands within
//! the coalescing window of the previous session for the same device and pass
//! extends that row rather than adding another. Without it, OPDS page streaming
//! would write one row per page turn.
//!
//! `read_progress` and `read_completions` keep their existing shape and stay
//! populated, so reverting to a binary that predates this table leaves a working
//! system.
//!
//! The three indexes each serve one access path:
//!
//! * `(user_id, book_id, pass, client_ended_at)` is the fold's ordered slice.
//! * `(user_id, client_started_at)` serves per-user reading statistics.
//! * `(user_id, book_id, device_id, pass, client_ended_at)` finds the candidate
//!   row for coalescing on append.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ReadingSessions::Table)
                    .if_not_exists()
                    // Client-generated, so replaying a batch after a dropped
                    // connection collides on the primary key instead of
                    // double-counting.
                    .col(
                        ColumnDef::new(ReadingSessions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ReadingSessions::UserId).uuid().not_null())
                    .col(ColumnDef::new(ReadingSessions::BookId).uuid().not_null())
                    // Stable per install. Legacy producers that have no device
                    // concept derive one from the API key or user agent.
                    .col(
                        ColumnDef::new(ReadingSessions::DeviceId)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ReadingSessions::DeviceName).string().null())
                    // Which read-through. Incremented by a `reset`.
                    .col(
                        ColumnDef::new(ReadingSessions::Pass)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    // 'progress' | 'completed' | 'reset'
                    .col(ColumnDef::new(ReadingSessions::Kind).string().not_null())
                    .col(ColumnDef::new(ReadingSessions::ToPage).integer().null())
                    .col(
                        ColumnDef::new(ReadingSessions::ToPercentage)
                            .double()
                            .null(),
                    )
                    .col(ColumnDef::new(ReadingSessions::R2Progression).text().null())
                    // Measured by the reader, never derived from the timestamps
                    // below: wall-clock elapsed counts a book left open on the
                    // nightstand as reading. NULL when the producer cannot
                    // measure it, which is the honest value for OPDS, Komga,
                    // and KOReader.
                    .col(
                        ColumnDef::new(ReadingSessions::ActiveDurationMs)
                            .big_integer()
                            .null(),
                    )
                    // 'measured' | 'inferred' | 'unknown'. Kept distinct so
                    // statistics can report provenance rather than silently
                    // blending reconstructed time into measured totals.
                    .col(
                        ColumnDef::new(ReadingSessions::DurationSource)
                            .string()
                            .not_null()
                            .default("unknown"),
                    )
                    .col(ColumnDef::new(ReadingSessions::PagesRead).integer().null())
                    .col(
                        ColumnDef::new(ReadingSessions::ClientStartedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReadingSessions::ClientEndedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReadingSessions::ServerRecordedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_reading_sessions_user_id")
                            .from(ReadingSessions::Table, ReadingSessions::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_reading_sessions_book_id")
                            .from(ReadingSessions::Table, ReadingSessions::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // The fold's ordered slice. Column order matches the sort the fold
        // applies, so it reads straight off the index.
        manager
            .create_index(
                Index::create()
                    .name("idx_reading_sessions_fold")
                    .table(ReadingSessions::Table)
                    .col(ReadingSessions::UserId)
                    .col(ReadingSessions::BookId)
                    .col(ReadingSessions::Pass)
                    .col(ReadingSessions::ClientEndedAt)
                    .to_owned(),
            )
            .await?;

        // Per-user reading statistics over a time range.
        manager
            .create_index(
                Index::create()
                    .name("idx_reading_sessions_stats")
                    .table(ReadingSessions::Table)
                    .col(ReadingSessions::UserId)
                    .col(ReadingSessions::ClientStartedAt)
                    .to_owned(),
            )
            .await?;

        // Finds the coalescing candidate: the most recent session for this
        // device within the current pass.
        manager
            .create_index(
                Index::create()
                    .name("idx_reading_sessions_coalesce")
                    .table(ReadingSessions::Table)
                    .col(ReadingSessions::UserId)
                    .col(ReadingSessions::BookId)
                    .col(ReadingSessions::DeviceId)
                    .col(ReadingSessions::Pass)
                    .col((ReadingSessions::ClientEndedAt, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ReadingSessions::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum ReadingSessions {
    Table,
    Id,
    UserId,
    BookId,
    DeviceId,
    DeviceName,
    Pass,
    Kind,
    ToPage,
    ToPercentage,
    R2Progression,
    ActiveDurationMs,
    DurationSource,
    PagesRead,
    ClientStartedAt,
    ClientEndedAt,
    ServerRecordedAt,
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
