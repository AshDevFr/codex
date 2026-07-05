//! Deep, per-record verification that compares the *canonical* value of every
//! row on both sides, so representation differences that are semantically
//! irrelevant do not read as mismatches:
//!
//! - numbers: `1.0` and `1` are equal (integer-valued floats normalize to ints)
//! - JSON objects: key order is normalized (PostgreSQL `jsonb` reorders keys)
//! - timestamps: normalized to microsecond precision (PostgreSQL truncates)
//!
//! Each table is reduced to an order-independent 128-bit digest (a wrapping sum
//! of per-row hashes) plus a row count, computed identically from a database
//! connection or from an archive's NDJSON. Matching digests mean the two sides
//! hold the same canonical data; the check streams rows, so it is O(1) memory.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Number, Value};

/// Per-table digest: an order-independent content checksum and a row count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableDigest {
    pub table: String,
    pub checksum: u128,
    pub rows: u64,
}

/// A table whose source and target content or row count disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullMismatch {
    pub table: String,
    pub source_rows: u64,
    pub target_rows: u64,
    /// True when the row counts match but the canonical content differs.
    pub content_differs: bool,
}

/// Accumulates a table's digest one row at a time.
#[derive(Default)]
pub(crate) struct DigestAccumulator {
    checksum: u128,
    rows: u64,
}

impl DigestAccumulator {
    /// Fold one row (any `Serialize` model) into the digest.
    pub(crate) fn add<M: Serialize>(&mut self, model: &M) {
        self.checksum = self
            .checksum
            .wrapping_add(u128::from(canonical_row_hash(model)));
        self.rows += 1;
    }

    pub(crate) fn finish(self, table: String) -> TableDigest {
        TableDigest {
            table,
            checksum: self.checksum,
            rows: self.rows,
        }
    }
}

/// Hash of a row's canonical representation.
fn canonical_row_hash<M: Serialize>(model: &M) -> u64 {
    let value = serde_json::to_value(model).unwrap_or(Value::Null);
    let canonical = serde_json::to_string(&canonicalize(value)).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    hasher.finish()
}

/// Recursively rewrite a JSON value into a canonical form.
fn canonicalize(value: Value) -> Value {
    match value {
        Value::Number(n) => canonical_number(n),
        Value::String(s) => Value::String(canonical_string(&s)),
        Value::Array(items) => Value::Array(items.into_iter().map(canonicalize).collect()),
        Value::Object(map) => {
            // Sort keys so jsonb's reordering doesn't matter, and canonicalize
            // each value.
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut out = serde_json::Map::with_capacity(entries.len());
            for (k, v) in entries {
                out.insert(k, canonicalize(v));
            }
            Value::Object(out)
        }
        other => other,
    }
}

/// Normalize integer-valued floats to integers (`1.0` → `1`), so a value stored
/// as text JSON and re-read from `jsonb` compares equal.
fn canonical_number(n: Number) -> Value {
    if let Some(f) = n.as_f64()
        && f.is_finite()
        && f.fract() == 0.0
        && f.abs() < 9.0e15
    {
        return if f >= 0.0 {
            Value::Number(Number::from(f as u64))
        } else {
            Value::Number(Number::from(f as i64))
        };
    }
    Value::Number(n)
}

/// Truncate RFC 3339 timestamps to microsecond precision so a value that
/// PostgreSQL truncated matches the SQLite original. Non-timestamp strings pass
/// through unchanged (and identical strings on both sides stay identical).
fn canonical_string(s: &str) -> String {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return dt
            .with_timezone(&Utc)
            .format("%Y-%m-%dT%H:%M:%S%.6fZ")
            .to_string();
    }
    s.to_string()
}

/// Compare two sets of per-table digests, returning every table whose row count
/// or canonical content differs.
pub fn compare_digests(source: &[TableDigest], target: &[TableDigest]) -> Vec<FullMismatch> {
    let mut mismatches = Vec::new();
    for s in source {
        let t = target.iter().find(|t| t.table == s.table);
        let (target_rows, target_checksum) = match t {
            Some(t) => (t.rows, t.checksum),
            None => (0, 0),
        };
        if s.rows != target_rows {
            mismatches.push(FullMismatch {
                table: s.table.clone(),
                source_rows: s.rows,
                target_rows,
                content_differs: false,
            });
        } else if s.checksum != target_checksum {
            mismatches.push(FullMismatch {
                table: s.table.clone(),
                source_rows: s.rows,
                target_rows,
                content_differs: true,
            });
        }
    }
    mismatches
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn integer_valued_floats_normalize_to_ints() {
        assert_eq!(canonicalize(json!(1.0)), json!(1));
        assert_eq!(
            canonicalize(json!({"n": 1.0})),
            canonicalize(json!({"n": 1}))
        );
    }

    #[test]
    fn object_key_order_is_normalized() {
        let a = canonicalize(json!({"b": 1, "a": 2}));
        let b = canonicalize(json!({"a": 2, "b": 1}));
        assert_eq!(a, b);
    }

    #[test]
    fn timestamps_truncate_to_microseconds() {
        // Same instant, nanosecond vs microsecond precision.
        let nanos = canonical_string("2026-07-05T12:00:00.123456789Z");
        let micros = canonical_string("2026-07-05T12:00:00.123456Z");
        assert_eq!(nanos, micros);
    }

    #[test]
    fn genuinely_different_rows_hash_differently() {
        assert_ne!(
            canonical_row_hash(&json!({"a": 1})),
            canonical_row_hash(&json!({"a": 2}))
        );
    }
}
