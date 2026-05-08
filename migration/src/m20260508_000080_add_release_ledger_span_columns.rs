//! Add `chapters` + `volumes` JSON span columns to `release_ledger`.
//!
//! The existing `chapter` (f64) and `volume` (i32) scalars can only hold a
//! single value per release. Real Nyaa compilations frequently cover ranges
//! and even disjoint spans (`v01-04 + v06-09`) — we silently squashed those
//! to the start value, which both mislabeled the inbox and broke the
//! "auto-ignore when fully owned" decision.
//!
//! After this migration:
//!   - `chapters` is a JSON array of `[{ "start": Number, "end": Number }, ...]`
//!     describing every chapter the release covers. `null` when the upstream
//!     title carries no chapter info at all.
//!   - `volumes` mirrors the shape, with integer-valued spans.
//!   - The legacy `chapter` / `volume` scalars stay around as the *primary
//!     value* used for SQL `ORDER BY` (cheap, indexable, no DB-specific
//!     JSON-path syntax). The repo derives them as `max(span.end)` on insert
//!     so "release covering content up to N" sorts by N.
//!
//! Backfill maps every existing single-value row into a one-element span
//! list so the new columns are populated for the historic ledger before the
//! ingestion path stops writing scalars directly.

use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("release_ledger"))
                    .add_column(ColumnDef::new(Alias::new("chapters")).json_binary())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("release_ledger"))
                    .add_column(ColumnDef::new(Alias::new("volumes")).json_binary())
                    .to_owned(),
            )
            .await?;

        // Backfill: turn every existing scalar value into a one-element span.
        // The JSON literal differs slightly across backends — Postgres prefers
        // `jsonb_build_array` / `jsonb_build_object` with native casting,
        // SQLite has `json_array` / `json_object`. Using the build-* helpers
        // keeps numeric typing intact (no string-coerced values).
        let db = manager.get_connection();
        let backend = db.get_database_backend();
        match backend {
            DbBackend::Postgres => {
                db.execute(Statement::from_string(
                    DbBackend::Postgres,
                    r#"UPDATE release_ledger
                       SET chapters = jsonb_build_array(jsonb_build_object('start', chapter, 'end', chapter))
                       WHERE chapter IS NOT NULL"#
                        .to_string(),
                ))
                .await?;
                db.execute(Statement::from_string(
                    DbBackend::Postgres,
                    r#"UPDATE release_ledger
                       SET volumes = jsonb_build_array(jsonb_build_object('start', volume, 'end', volume))
                       WHERE volume IS NOT NULL"#
                        .to_string(),
                ))
                .await?;
            }
            _ => {
                // SQLite (and anything else we treat as "default"): use
                // json_array + json_object. Numeric types round-trip through
                // SQLite's typeless JSON faithfully for our integer / float
                // values.
                db.execute(Statement::from_string(
                    DbBackend::Sqlite,
                    r#"UPDATE release_ledger
                       SET chapters = json_array(json_object('start', chapter, 'end', chapter))
                       WHERE chapter IS NOT NULL"#
                        .to_string(),
                ))
                .await?;
                db.execute(Statement::from_string(
                    DbBackend::Sqlite,
                    r#"UPDATE release_ledger
                       SET volumes = json_array(json_object('start', volume, 'end', volume))
                       WHERE volume IS NOT NULL"#
                        .to_string(),
                ))
                .await?;
            }
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("release_ledger"))
                    .drop_column(Alias::new("volumes"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("release_ledger"))
                    .drop_column(Alias::new("chapters"))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
