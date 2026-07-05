//! Foreign-key enforcement control for bulk loads.
//!
//! A 1:1 load inserts tables in an arbitrary order, so per-row FK enforcement
//! would reject rows whose parents are inserted later. We suppress enforcement
//! for the duration of the load, using a mechanism that needs only the
//! privileges the migration role already has (it just ran the migrations):
//!
//! - **SQLite:** `PRAGMA defer_foreign_keys = ON` defers all FK checks until
//!   COMMIT (and auto-resets there), so a complete, consistent dataset is still
//!   validated as a whole when the transaction commits.
//! - **PostgreSQL:** drop every FK constraint before the load and recreate it
//!   after. Recreating an FK **revalidates** the loaded rows, so integrity is
//!   still checked. This needs only table ownership — unlike
//!   `session_replication_role`, which requires a superuser and is therefore
//!   unavailable on most managed Postgres.
//! - **MySQL:** `SET FOREIGN_KEY_CHECKS = 0` for the session.
//!
//! All of this is transaction-scoped: pass the same
//! [`sea_orm::DatabaseTransaction`] to [`before_load`] and [`after_load`] that
//! performs the load, so a failure rolls the whole thing back.

use anyhow::{Context, Result};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

/// State captured by [`before_load`] that [`after_load`] needs to restore.
pub enum FkGuard {
    /// SQLite: checks deferred; the commit revalidates them.
    Deferred,
    /// PostgreSQL: FK constraints were dropped and must be recreated.
    DroppedConstraints(Vec<PgForeignKey>),
    /// MySQL: session FK checks were disabled.
    SessionDisabled,
}

/// A captured PostgreSQL foreign-key constraint, enough to recreate it verbatim.
pub struct PgForeignKey {
    table: String,
    name: String,
    definition: String,
}

/// Suppress foreign-key enforcement for the current transaction, returning the
/// state needed to restore it in [`after_load`].
pub async fn before_load<C: ConnectionTrait>(conn: &C) -> Result<FkGuard> {
    match conn.get_database_backend() {
        DatabaseBackend::Sqlite => {
            conn.execute_unprepared("PRAGMA defer_foreign_keys = ON")
                .await
                .context("failed to defer SQLite foreign keys")?;
            Ok(FkGuard::Deferred)
        }
        DatabaseBackend::Postgres => {
            let fks = capture_postgres_fks(conn).await?;
            for fk in &fks {
                conn.execute_unprepared(&format!(
                    "ALTER TABLE {} DROP CONSTRAINT \"{}\"",
                    fk.table, fk.name
                ))
                .await
                .with_context(|| format!("failed to drop FK {} on {}", fk.name, fk.table))?;
            }
            Ok(FkGuard::DroppedConstraints(fks))
        }
        DatabaseBackend::MySql => {
            conn.execute_unprepared("SET FOREIGN_KEY_CHECKS = 0")
                .await
                .context("failed to disable MySQL foreign-key checks")?;
            Ok(FkGuard::SessionDisabled)
        }
    }
}

/// Restore foreign-key enforcement after the load. On PostgreSQL this recreates
/// (and thereby revalidates) every constraint captured in [`before_load`].
pub async fn after_load<C: ConnectionTrait>(conn: &C, guard: FkGuard) -> Result<()> {
    match guard {
        // The SQLite commit revalidates deferred FKs; nothing to do here.
        FkGuard::Deferred => Ok(()),
        FkGuard::DroppedConstraints(fks) => {
            for fk in &fks {
                conn.execute_unprepared(&format!(
                    "ALTER TABLE {} ADD CONSTRAINT \"{}\" {}",
                    fk.table, fk.name, fk.definition
                ))
                .await
                .with_context(|| {
                    format!(
                        "failed to recreate FK {} on {} (referential integrity violated?)",
                        fk.name, fk.table
                    )
                })?;
            }
            Ok(())
        }
        FkGuard::SessionDisabled => {
            conn.execute_unprepared("SET FOREIGN_KEY_CHECKS = 1")
                .await
                .context("failed to re-enable MySQL foreign-key checks")?;
            Ok(())
        }
    }
}

/// Read every foreign-key constraint in the `public` schema with a definition
/// string suitable for recreating it verbatim.
async fn capture_postgres_fks<C: ConnectionTrait>(conn: &C) -> Result<Vec<PgForeignKey>> {
    let rows = conn
        .query_all(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT conrelid::regclass::text AS tbl, conname, pg_get_constraintdef(oid) AS def \
             FROM pg_constraint \
             WHERE contype = 'f' AND connamespace = 'public'::regnamespace"
                .to_string(),
        ))
        .await
        .context("failed to read PostgreSQL foreign-key constraints")?;

    let mut fks = Vec::with_capacity(rows.len());
    for row in rows {
        fks.push(PgForeignKey {
            table: row.try_get::<String>("", "tbl")?,
            name: row.try_get::<String>("", "conname")?,
            definition: row.try_get::<String>("", "def")?,
        });
    }
    Ok(fks)
}
