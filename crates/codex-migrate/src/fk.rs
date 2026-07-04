//! Foreign-key enforcement control for bulk loads.
//!
//! A 1:1 load inserts tables in an arbitrary order, so per-row FK enforcement
//! would reject rows whose parents are inserted later. Rather than maintain a
//! drift-prone topological insert order across ~60 tables (some
//! self-referential), we disable enforcement for the duration of the load and
//! rely on the source data already being internally consistent.
//!
//! These calls are transaction-scoped and must run on the same connection that
//! performs the load — pass a [`sea_orm::DatabaseTransaction`].
//!
//! - **SQLite:** `PRAGMA defer_foreign_keys = ON` defers all FK checks until
//!   COMMIT (and auto-resets there), so a complete, consistent dataset is still
//!   validated as a whole when the transaction commits.
//! - **PostgreSQL:** `SET LOCAL session_replication_role = replica` suppresses
//!   FK (and other) triggers for the transaction. This requires the connection
//!   role to be a superuser or the table owner. There is no commit-time
//!   re-check, so integrity rests on source consistency plus the row-count
//!   verification performed after the load.

use anyhow::{Context, Result};
use sea_orm::{ConnectionTrait, DatabaseBackend};

/// Disable foreign-key enforcement for the current transaction.
pub async fn disable<C: ConnectionTrait>(conn: &C) -> Result<()> {
    let sql = match conn.get_database_backend() {
        DatabaseBackend::Sqlite => "PRAGMA defer_foreign_keys = ON",
        DatabaseBackend::Postgres => "SET LOCAL session_replication_role = replica",
        DatabaseBackend::MySql => "SET FOREIGN_KEY_CHECKS = 0",
    };
    conn.execute_unprepared(sql)
        .await
        .with_context(|| format!("failed to disable foreign-key enforcement ({sql})"))?;
    Ok(())
}
