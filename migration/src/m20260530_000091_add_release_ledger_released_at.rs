//! Split the release-ledger timestamp into "detected" vs "released".
//!
//! Originally `release_ledger.observed_at` was overloaded: plugins set it from
//! the upstream feed's publish date (`<pubDate>`), so it actually meant "when
//! the release was published", not "when Codex detected it". That made the
//! column name misleading and gave the inbox no way to sort by detection time.
//!
//! This migration adds a nullable `released_at` (the upstream publish date) and
//! re-points `observed_at` at detection time:
//!
//! - `released_at = observed_at`  — the old `observed_at` *was* the release date.
//! - `observed_at = created_at`   — detection time is when the row was inserted.
//!
//! Both assignments read the pre-update row values in a single `UPDATE`
//! (true on SQLite and Postgres), so the order within the SET list is safe.
//!
//! `released_at` stays nullable: plugins emit it from a feed field that can be
//! absent, and third-party plugins predating this change won't send it at all.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ReleaseLedger::Table)
                    .add_column(
                        ColumnDef::new(ReleaseLedger::ReleasedAt).timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;

        // Backfill: the old observed_at held the release date; detection time
        // is the row's created_at. Single UPDATE so both RHS read old values.
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE release_ledger \
                 SET released_at = observed_at, observed_at = created_at",
            )
            .await?;

        // Inbox/per-series sort by release date (newest first), NULLs excluded
        // from the hot path via the index only covering non-null rows.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX idx_release_ledger_released \
                 ON release_ledger(released_at DESC) WHERE released_at IS NOT NULL",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS idx_release_ledger_released")
            .await?;

        // Restore the original overloaded observed_at (release date) before
        // dropping released_at, so a re-up reconstructs the same state.
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE release_ledger \
                 SET observed_at = released_at WHERE released_at IS NOT NULL",
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ReleaseLedger::Table)
                    .drop_column(ReleaseLedger::ReleasedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum ReleaseLedger {
    Table,
    ReleasedAt,
}
