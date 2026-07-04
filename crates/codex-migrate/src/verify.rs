//! Post-load verification: per-table row-count parity between source and
//! target. A mismatch means rows were dropped or duplicated and the transfer
//! must be treated as failed.

use crate::registry::TableRows;

/// A table whose source and target row counts disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountMismatch {
    pub table: String,
    pub source: u64,
    pub target: u64,
}

/// Compare two per-table count sets (as produced by
/// [`crate::registry::count_all`]) and return every table that differs.
/// Tables present in only one set are reported with `0` for the missing side.
pub fn compare(source: &[TableRows], target: &[TableRows]) -> Vec<CountMismatch> {
    let mut mismatches = Vec::new();
    for s in source {
        let target_rows = target
            .iter()
            .find(|t| t.table == s.table)
            .map(|t| t.rows)
            .unwrap_or(0);
        if target_rows != s.rows {
            mismatches.push(CountMismatch {
                table: s.table.clone(),
                source: s.rows,
                target: target_rows,
            });
        }
    }
    // Tables that exist on the target but not the source.
    for t in target {
        if !source.iter().any(|s| s.table == t.table) && t.rows != 0 {
            mismatches.push(CountMismatch {
                table: t.table.clone(),
                source: 0,
                target: t.rows,
            });
        }
    }
    mismatches
}
