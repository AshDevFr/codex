pub mod copy;
pub mod export;
pub mod import;
pub mod migrate;
pub mod openapi;
pub mod scan;
pub mod seed;
pub mod serve;
pub mod tasks;
pub mod wait_for_migrations;
pub mod worker;

pub use copy::copy_command;
pub use export::export_command;
pub use import::import_command;

use anyhow::{Result, bail};
use codex_migrate::full_verify::FullMismatch;
use codex_migrate::{TableRows, registry, verify};
use sea_orm::DatabaseConnection;
use tracing::{info, warn};

/// Compare per-table source counts against a live target and fail on any
/// mismatch. Shared by `import` and `copy` for their post-load verification.
pub(crate) async fn verify_row_counts(
    source_counts: &[TableRows],
    target: &DatabaseConnection,
) -> Result<()> {
    let target_counts = registry::count_all(target).await?;
    let mismatches = verify::compare(source_counts, &target_counts);
    if mismatches.is_empty() {
        info!("✓ verification: {} tables match", source_counts.len());
        Ok(())
    } else {
        for m in &mismatches {
            warn!("  ✗ {}: source={} target={}", m.table, m.source, m.target);
        }
        bail!(
            "row-count verification failed: {} table(s) differ",
            mismatches.len()
        );
    }
}

/// Print the outcome of a full (per-record) verification as a report.
///
/// This is an informational deep check: it reports which tables differ but does
/// **not** fail the command (the default row-count check is the hard safety
/// gate). `tables_checked` is the total number of tables compared.
pub(crate) fn report_full_verify(mismatches: &[FullMismatch], tables_checked: usize) {
    info!("---------- full verification ----------");
    if mismatches.is_empty() {
        info!("✓ every row matches across all {tables_checked} tables (canonical comparison)");
    } else {
        warn!(
            "⚠ {} of {} table(s) differ:",
            mismatches.len(),
            tables_checked
        );
        for m in mismatches {
            if m.content_differs {
                warn!(
                    "  ✗ {}: {} rows, content differs (values not identical after canonicalization)",
                    m.table, m.source_rows
                );
            } else {
                warn!(
                    "  ✗ {}: row count differs — source={} target={}",
                    m.table, m.source_rows, m.target_rows
                );
            }
        }
        warn!("(full verification is informational; the command did not fail on these)");
    }
    info!("---------------------------------------");
}
pub use migrate::migrate_command;
pub use openapi::{OpenApiFormat, openapi_command};
pub use scan::scan_command;
pub use seed::seed_command;
pub use serve::serve_command;
pub use tasks::{TasksSubcommand, tasks_command};
pub use wait_for_migrations::wait_for_migrations_command;
pub use worker::worker_command;
