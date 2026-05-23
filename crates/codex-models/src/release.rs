//! Release-tracking value types shared across the db, services, and tasks
//! layers.
//!
//! These are pure data shapes and small helpers. The ledger-shaped service
//! logic (auto-ignore, candidate validation, language gating) stays in
//! `codex::services::release`; this module only holds the types and the
//! span helpers that repositories need to speak.

use serde::{Deserialize, Serialize};

/// Inclusive numeric span. Single values are encoded as `start == end`
/// (e.g. `NumericSpan { start: 5.0, end: 5.0 }`).
///
/// A release candidate carries one [`Vec<NumericSpan>`] per axis (volumes
/// and chapters). Disjoint coverage (`v01-04 + v06-09`) is preserved as
/// multiple spans; the host's auto-ignore walks every value in every span
/// before deciding the user owns the release.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumericSpan {
    pub start: f64,
    pub end: f64,
}

/// Normalize a span list:
///   1. Swap any span where `start > end` (defensive against buggy plugins).
///   2. Sort ascending by `start`, then `end`.
///   3. Merge overlapping spans (touching counts as overlap).
///
/// Mirrors the parser-side `normalizeSpans` in `plugins/release-nyaa` so
/// host and plugin agree on the canonical shape stored in the ledger.
/// Returns `None` when the input is `Some(empty)` so callers can collapse
/// "I parsed an empty list" into "no info" before persistence.
pub fn normalize_spans(spans: Option<Vec<NumericSpan>>) -> Option<Vec<NumericSpan>> {
    let raw = spans?;
    if raw.is_empty() {
        return None;
    }
    let mut fixed: Vec<NumericSpan> = raw
        .into_iter()
        .map(|s| {
            if s.start <= s.end {
                s
            } else {
                NumericSpan {
                    start: s.end,
                    end: s.start,
                }
            }
        })
        .collect();
    fixed.sort_by(|a, b| {
        a.start
            .partial_cmp(&b.start)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.end
                    .partial_cmp(&b.end)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    let mut out: Vec<NumericSpan> = Vec::with_capacity(fixed.len());
    for s in fixed {
        match out.last_mut() {
            Some(last) if s.start <= last.end => {
                if s.end > last.end {
                    last.end = s.end;
                }
            }
            _ => out.push(s),
        }
    }
    Some(out)
}

/// Highest end-value across every span. `None` for an empty / missing list.
/// Used to derive the primary scalar (`chapter` / `volume`) the SQL ORDER BY
/// clauses still rely on.
pub fn primary_value(spans: Option<&Vec<NumericSpan>>) -> Option<f64> {
    let list = spans?;
    list.iter().map(|s| s.end).fold(None, |acc, v| match acc {
        None => Some(v),
        Some(cur) if v > cur => Some(v),
        other => other,
    })
}

/// Per-series ownership signature consumed by the auto-ignore logic in
/// `codex::services::release::auto_ignore`. Produced by
/// `codex::db::repositories::SeriesRepository::get_owned_release_keys_for_series`.
#[derive(Debug, Default, Clone)]
pub struct OwnedReleaseKeys {
    /// `(volume, chapter)` pairs from book metadata, after filtering out
    /// rows with both fields null.
    ///
    /// - `(Some(v), None)` — whole volume `v` owned (no specific chapter).
    /// - `(Some(v), Some(c))` — chapter `c` of volume `v` owned.
    /// - `(None, Some(c))` — chapter `c` owned, volume unknown.
    pub keys: Vec<(Option<i32>, Option<f64>)>,
    /// `true` if at least one book in the series carries volume metadata.
    /// When `false`, callers fall back to [`Self::volumes_owned_count`].
    pub has_any_volume_metadata: bool,
    /// Count of "complete-volume" books (volume IS NOT NULL AND chapter
    /// IS NULL). Only consulted in the count-fallback branch when
    /// [`Self::has_any_volume_metadata`] is `false`.
    pub volumes_owned_count: i64,
}
