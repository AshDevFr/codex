//! Lightweight progress reporting for long transfers.
//!
//! Emitted as `tracing` log lines (per-table start/finish plus a periodic
//! within-table row count) rather than a TTY progress bar, so it reads the same
//! in an interactive terminal and in captured Kubernetes/CI logs.

use tracing::info;

/// Emit a within-table update roughly every this many rows.
pub(crate) const ROW_REPORT_INTERVAL: u64 = 100_000;

/// Whether a transfer should report progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    /// No progress output.
    Silent,
    /// Log per-table and periodic row-count lines.
    Log,
}

impl Progress {
    /// Build from a CLI flag.
    pub fn from_flag(enabled: bool) -> Self {
        if enabled {
            Progress::Log
        } else {
            Progress::Silent
        }
    }

    fn on(self) -> bool {
        matches!(self, Progress::Log)
    }

    pub(crate) fn table_start(self, table: &str) {
        if self.on() {
            info!("  → {table}");
        }
    }

    pub(crate) fn table_rows(self, table: &str, rows: u64) {
        if self.on() {
            info!("    {table}: {rows} rows…");
        }
    }

    pub(crate) fn table_done(self, table: &str, rows: u64) {
        if self.on() {
            info!("  ✓ {table}: {rows} rows");
        }
    }
}
