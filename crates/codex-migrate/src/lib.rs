//! Portable, engine-agnostic, entity-driven data transfer for Codex.
//!
//! Moves every row of every table between two SeaORM backends by materializing
//! typed `Model` values, so SQLite ↔ PostgreSQL type differences (UUID blob vs
//! native `uuid`, text JSON vs JSONB, 0/1 vs `bool`) are handled by SeaORM
//! rather than by hand-written casts. This crate is the shared engine behind
//! the `export`, `import`, and `copy` subcommands.
//!
//! Loads run inside a single destination transaction with foreign-key
//! enforcement disabled ([`fk`]), then commit as a unit. On SQLite the commit
//! re-validates the whole dataset (deferred FK check); on PostgreSQL integrity
//! rests on source consistency plus the row-count verification in [`verify`].

pub mod engine;
pub mod fk;
pub mod registry;
pub mod verify;

use anyhow::{Context, Result};
use sea_orm::{DatabaseConnection, TransactionTrait};

pub use registry::{TableRows, table_names};

/// Outcome of a [`transfer`], carrying per-table and total row counts.
#[derive(Debug, Clone)]
pub struct TransferReport {
    pub tables: Vec<TableRows>,
    pub total_rows: u64,
}

/// Copy all data from `src` into `dst`, producing a faithful 1:1 mirror.
///
/// The destination must already have the schema applied (run migrations
/// first). Because migrations seed rows (e.g. default `settings`), a freshly
/// migrated database is *not* empty; every table is therefore truncated before
/// the load so the source's own rows are authoritative. The entire operation
/// (disable FK → truncate → load) runs in one destination transaction and
/// commits atomically — on SQLite the commit re-validates all foreign keys.
///
/// This is the low-level primitive. The destructive-overwrite safety gate
/// (refuse a target that already holds user data unless `--replace`) lives in
/// the CLI layer, not here.
pub async fn transfer(
    src: &DatabaseConnection,
    dst: &DatabaseConnection,
) -> Result<TransferReport> {
    let txn = dst
        .begin()
        .await
        .context("failed to open destination transaction")?;

    fk::disable(&txn).await?;

    registry::truncate_all(&txn)
        .await
        .context("failed to clear destination before load")?;

    let tables = registry::copy_all(src, &txn, engine::DEFAULT_BATCH_SIZE)
        .await
        .context("failed while copying tables")?;

    txn.commit()
        .await
        .context("failed to commit destination transaction (foreign-key check may have failed)")?;

    let total_rows = tables.iter().map(|t| t.rows).sum();
    Ok(TransferReport { tables, total_rows })
}
