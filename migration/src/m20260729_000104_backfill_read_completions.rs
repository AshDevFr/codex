//! Backfill `read_completions` from books that are already marked completed.
//!
//! Without this, every book a user has already finished would read as "never
//! completed" the moment the feature ships, which is exactly the data loss the
//! table exists to prevent.
//!
//! Two details matter:
//!
//! * **`completed_at` can be NULL on a completed row.** That column was added to
//!   `read_progress` after the fact with no backfill, and the logic keeping it
//!   consistent with the `completed` flag came later still, so rows with
//!   `completed = true, completed_at = NULL` exist in the wild. Falling back to
//!   `updated_at` gives those rows a plausible date instead of dropping them; it
//!   is the closest thing to a completion timestamp the row still carries.
//! * **UUIDs are generated in Rust, not SQL.** SQLite has no built-in UUID
//!   function and the Postgres one depends on an extension, so a raw-SQL
//!   `INSERT ... SELECT` would need different statements per backend. Selecting
//!   and then inserting sidesteps the dialect difference entirely.
//!
//! Idempotent: rows already present for a `(user_id, book_id)` are skipped, so
//! re-running (or running after the write hook has started banking completions)
//! does not duplicate history.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, FromQueryResult, Statement, TransactionTrait};

use crate::m20260729_000103_create_read_completions::ReadCompletions;

#[derive(DeriveMigrationName)]
pub struct Migration;

const BATCH_SIZE: u64 = 1000;

#[derive(Debug, FromQueryResult)]
struct CompletedRow {
    user_id: uuid::Uuid,
    book_id: uuid::Uuid,
    started_at: chrono::DateTime<chrono::Utc>,
    completed_at: chrono::DateTime<chrono::Utc>,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();
        let txn = db.begin().await?;

        let mut offset: u64 = 0;
        loop {
            // COALESCE covers the legacy `completed = true, completed_at = NULL`
            // rows described above. The NOT EXISTS keeps the migration
            // idempotent.
            let select_sql = format!(
                "SELECT rp.user_id, rp.book_id, rp.started_at, \
                        COALESCE(rp.completed_at, rp.updated_at) AS completed_at \
                 FROM read_progress rp \
                 WHERE rp.completed = true \
                   AND NOT EXISTS ( \
                     SELECT 1 FROM read_completions rc \
                     WHERE rc.user_id = rp.user_id AND rc.book_id = rp.book_id \
                   ) \
                 ORDER BY rp.user_id, rp.book_id \
                 LIMIT {BATCH_SIZE} OFFSET {offset}"
            );
            let rows = CompletedRow::find_by_statement(Statement::from_string(backend, select_sql))
                .all(&txn)
                .await?;

            if rows.is_empty() {
                break;
            }

            let batch_size = rows.len();

            // One multi-row INSERT per batch rather than per-row round-trips,
            // which would be O(n) on a library with thousands of read books.
            let mut insert = Query::insert();
            insert.into_table(ReadCompletions::Table).columns([
                ReadCompletions::Id,
                ReadCompletions::UserId,
                ReadCompletions::BookId,
                ReadCompletions::StartedAt,
                ReadCompletions::CompletedAt,
            ]);
            for row in &rows {
                insert.values_panic([
                    uuid::Uuid::new_v4().into(),
                    row.user_id.into(),
                    row.book_id.into(),
                    row.started_at.into(),
                    row.completed_at.into(),
                ]);
            }
            txn.execute(backend.build(&insert)).await?;

            // The NOT EXISTS filter means rows just inserted drop out of the
            // next SELECT, so the offset must not advance or this would skip a
            // batch. Re-querying from 0 each time is correct and terminates
            // because every pass shrinks the candidate set.
            if (batch_size as u64) < BATCH_SIZE {
                break;
            }
            offset = 0;
        }

        txn.commit().await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Backfilled rows are indistinguishable from ones recorded live, so
        // there is nothing safe to selectively remove. The table itself is owned
        // by the create migration, whose `down` drops it wholesale.
        let _ = manager;
        Ok(())
    }
}
