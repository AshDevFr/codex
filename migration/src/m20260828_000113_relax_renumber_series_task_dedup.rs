//! Let a renumber request queue behind a pass that is already running.
//!
//! `unique_pending_series` treats `pending` and `processing` alike, so a second
//! `renumber_series` row for a series cannot be inserted while one is in
//! flight. For every other task type that is the behaviour we want: a running
//! task will observe whatever the caller wants done, so folding the request
//! into it loses nothing.
//!
//! A renumber pass is the exception. It reads the series once, at the top, and
//! skips any book with no `book_metadata` row yet. A book whose analysis
//! finishes after that read is invisible to the running pass, so folding its
//! request into that pass drops it: the pass completes, nothing is left in the
//! queue, and the book keeps a null number for good. The visible symptom is a
//! series numbered 1, 2, 5, 6, 7 with two books showing no number, gaps that
//! survive a rescan because a scan only queues a pass when it creates,
//! deletes, or restores a book.
//!
//! So `renumber_series` dedups on `pending` alone. One pass may then be
//! processing while a successor waits, and briefly both can run: they build the
//! same position map from the same filenames and write the same numbers, so the
//! overlap is idempotent.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Dedup every task type across `pending` and `processing`, except
/// `renumber_series`, which dedups on `pending` only.
///
/// The drop and the create are one statement on purpose. Issued separately
/// they can land on different connections in the pool, and the second one has
/// been observed still seeing the index the first one dropped.
const RELAXED: &str = r#"
    DROP INDEX IF EXISTS unique_pending_series;
    CREATE UNIQUE INDEX unique_pending_series ON tasks(series_id, task_type)
    WHERE series_id IS NOT NULL
      AND (
        status = 'pending'
        OR (status = 'processing' AND task_type <> 'renumber_series')
      );
"#;

/// The original index, for `down`.
const STRICT: &str = r#"
    DROP INDEX IF EXISTS unique_pending_series;
    CREATE UNIQUE INDEX unique_pending_series ON tasks(series_id, task_type)
    WHERE status IN ('pending', 'processing') AND series_id IS NOT NULL;
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(RELAXED).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(STRICT).await?;
        Ok(())
    }
}
