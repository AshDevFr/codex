//! Backfill `reading_sessions` from the existing `read_progress` rows.
//!
//! `read_progress` is a projection of the session log from here on, and the fold
//! rebuilds it from scratch on every write. Without a seed session, the first
//! write to an already-read book would fold over a log containing only that one
//! event and throw away everything the reader had done before it.
//!
//! One session per existing row, reconstructing what that row implies:
//!
//! * **`kind` mirrors the `completed` flag.** A completed row backfills as a
//!   `completed` event so the fold keeps reporting it as finished, and so the
//!   completion already banked in `read_completions` is recognised as belonging
//!   to this pass rather than being duplicated.
//! * **`pass` is always 1.** Earlier passes are already banked in
//!   `read_completions`, which the fold never regenerates, so numbering the seed
//!   as the first pass is both correct and simplest. Resets after the cutover
//!   count up from there.
//! * **Duration is NULL, not zero.** Nothing measured reading time before this
//!   table existed. Zero would be a claim; NULL is the truth, and
//!   `duration_source = 'unknown'` says so explicitly.
//! * **`client_ended_at` takes `updated_at`.** The fold sorts on it, and
//!   `updated_at` is the closest thing the old row carries to "when this reading
//!   last happened". Anything a client syncs after the cutover is later than
//!   that and correctly wins.
//! * **UUIDs are generated in Rust.** SQLite has no built-in UUID function and
//!   the Postgres one needs an extension, so selecting and then inserting avoids
//!   needing a different statement per backend.
//!
//! Books marked unread have no `read_progress` row and so seed nothing. That is
//! correct: an empty log folds to no progress row, which is exactly their state.
//!
//! Idempotent: a `(user_id, book_id)` that already has sessions is skipped, so
//! re-running after the write path has started appending does not double-seed.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, FromQueryResult, Statement, TransactionTrait};

use crate::m20260814_000105_create_reading_sessions::ReadingSessions;

#[derive(DeriveMigrationName)]
pub struct Migration;

const BATCH_SIZE: u64 = 1000;
const LEGACY_DEVICE_ID: &str = "legacy";

#[derive(Debug, FromQueryResult)]
struct ProgressRow {
    user_id: uuid::Uuid,
    book_id: uuid::Uuid,
    current_page: i32,
    progress_percentage: Option<f64>,
    completed: bool,
    started_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    r2_progression: Option<String>,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();
        let txn = db.begin().await?;

        loop {
            let select_sql = format!(
                "SELECT rp.user_id, rp.book_id, rp.current_page, rp.progress_percentage, \
                        rp.completed, rp.started_at, rp.updated_at, rp.r2_progression \
                 FROM read_progress rp \
                 WHERE NOT EXISTS ( \
                   SELECT 1 FROM reading_sessions rs \
                   WHERE rs.user_id = rp.user_id AND rs.book_id = rp.book_id \
                 ) \
                 ORDER BY rp.user_id, rp.book_id \
                 LIMIT {BATCH_SIZE}"
            );
            let rows = ProgressRow::find_by_statement(Statement::from_string(backend, select_sql))
                .all(&txn)
                .await?;

            if rows.is_empty() {
                break;
            }

            let batch_size = rows.len();

            let mut insert = Query::insert();
            insert.into_table(ReadingSessions::Table).columns([
                ReadingSessions::Id,
                ReadingSessions::UserId,
                ReadingSessions::BookId,
                ReadingSessions::DeviceId,
                ReadingSessions::Pass,
                ReadingSessions::Kind,
                ReadingSessions::ToPage,
                ReadingSessions::ToPercentage,
                ReadingSessions::R2Progression,
                ReadingSessions::ActiveDurationMs,
                ReadingSessions::DurationSource,
                ReadingSessions::ClientStartedAt,
                ReadingSessions::ClientEndedAt,
                ReadingSessions::ServerRecordedAt,
            ]);
            for row in &rows {
                let kind = if row.completed {
                    "completed"
                } else {
                    "progress"
                };
                insert.values_panic([
                    uuid::Uuid::new_v4().into(),
                    row.user_id.into(),
                    row.book_id.into(),
                    LEGACY_DEVICE_ID.into(),
                    1.into(),
                    kind.into(),
                    row.current_page.into(),
                    row.progress_percentage.into(),
                    row.r2_progression.clone().into(),
                    Option::<i64>::None.into(),
                    "unknown".into(),
                    row.started_at.into(),
                    row.updated_at.into(),
                    row.updated_at.into(),
                ]);
            }
            txn.execute(backend.build(&insert)).await?;

            // Rows just inserted drop out of the next SELECT through the NOT
            // EXISTS filter, so there is no offset to advance. Every pass
            // shrinks the candidate set, which is what terminates the loop.
            if (batch_size as u64) < BATCH_SIZE {
                break;
            }
        }

        txn.commit().await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Backfilled sessions are indistinguishable from ones appended live, so
        // there is nothing safe to selectively remove. The table is owned by the
        // create migration, whose `down` drops it wholesale.
        let _ = manager;
        Ok(())
    }
}
