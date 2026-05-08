//! Decide whether an incoming release matches something the user already
//! owns, so ingestion can mark it `ignored` instead of `announced`.
//!
//! Range-aware. A release expresses its coverage as two span lists
//! (volumes, chapters). For a release to be auto-ignored we require *every*
//! value in *every* span on at least one axis to already be owned — owning
//! one volume of a `v01-10` compilation is no longer enough to hide it.
//!
//! Direct matches only. We do not infer chapter ownership from owned
//! volumes (chapter→volume mapping is unreliable upstream) or vice versa.
//!
//! Inputs come from [`crate::db::repositories::SeriesRepository::get_owned_release_keys_for_series`]:
//! the set of `(volume, chapter)` pairs derived from book metadata, plus
//! a count fallback used only when no book in the series has any volume
//! metadata.
//!
//! Whole-volume ownership is signaled by `chapter = None` in the owned set;
//! chapter ownership by `chapter = Some(_)`. A release for "Vol 3" matches
//! an owned `(Some(3), None)`; a release for "Ch 12" matches an owned
//! `(_, Some(12))` regardless of volume.

use crate::services::release::candidate::NumericSpan;

/// Per-series ownership signature consumed by [`should_auto_ignore`].
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
    /// When `false`, we fall back to [`Self::volumes_owned_count`].
    pub has_any_volume_metadata: bool,
    /// Count of "complete-volume" books (volume IS NOT NULL AND chapter
    /// IS NULL). Only consulted in the count-fallback branch when
    /// [`Self::has_any_volume_metadata`] is `false`.
    pub volumes_owned_count: i64,
}

/// True when the user owns *every* item the release covers.
///
/// We model the release as a set of `(volume, chapter)` items:
///   - Both axes have spans: the cartesian product of every value in
///     every volume span × every value in every chapter span.
///   - Volume axis only: each value `v` becomes the item `(v, _)`.
///   - Chapter axis only: each value `c` becomes the item `(_, c)`.
///   - Neither axis: zero items, never auto-ignored.
///
/// An item is "owned" by the rules below. Auto-ignore fires iff *every*
/// covered item is owned.
///
/// Item ownership rules:
/// - `(v, c)` paired item: at least one of
///   - `(Some(v), Some(c))` — exact pair owned, or
///   - `(Some(v), None)` — whole volume `v` owned (covers every chapter
///     in it), or
///   - `(None, Some(c))` — chapter `c` owned with no volume tag (chapters
///     are unique identifiers across the series; a vol-untagged ch `c` is
///     the same chapter as `(v, c)`).
///   - count fallback for the volume side when no book has volume metadata.
///   - We deliberately do *not* accept `(Some(other_v), Some(c))`. Owning
///     ch 5 of vol 1 does not cover "ch 5 of vol 2" — those are different
///     items even though they share a chapter number, because the volume
///     pin distinguishes them.
/// - `(v, _)` volume-only item: whole-volume key `(Some(v), None)` or count
///   fallback. Specific-chapter ownership of v does *not* count.
/// - `(_, c)` chapter-only item: any owned key with chapter `c`, regardless
///   of volume. Whole-volume ownership does *not* infer chapter ownership
///   (chapter→volume mapping unreliable).
///
/// Range examples:
/// - `v01-10` (vol-only range): auto-ignore iff each of vols 1..=10 is
///   owned as a whole.
/// - `001-050 as v01-10` (paired ranges): cross-product is 500 items, but
///   owning all 10 whole volumes covers every pair (because each pair
///   `(v, c)` gets the whole-vol-v rule). Equivalently, owning all 50
///   no-vol chapter keys covers every pair via the chapter rule.
/// - `v01-04 + v06-09` (disjoint vol range): vol 5 is *not* in the
///   coverage set, so not owning vol 5 doesn't block auto-ignore.
pub fn should_auto_ignore(
    release_volumes: Option<&[NumericSpan]>,
    release_chapters: Option<&[NumericSpan]>,
    owned: &OwnedReleaseKeys,
) -> bool {
    let has_volume_info = release_volumes.is_some_and(|s| !s.is_empty());
    let has_chapter_info = release_chapters.is_some_and(|s| !s.is_empty());
    if !has_volume_info && !has_chapter_info {
        return false;
    }

    let vol_values: Vec<i32> = release_volumes
        .into_iter()
        .flatten()
        .flat_map(span_integer_iter)
        .collect();
    let chap_values: Vec<f64> = release_chapters
        .into_iter()
        .flatten()
        .flat_map(chapter_span_values)
        .collect();

    match (has_volume_info, has_chapter_info) {
        (true, true) => vol_values
            .iter()
            .all(|v| chap_values.iter().all(|c| pair_owned(*v, *c, owned))),
        (true, false) => vol_values.iter().all(|v| volume_owned(*v, owned)),
        (false, true) => chap_values.iter().all(|c| chapter_owned(*c, owned)),
        (false, false) => false,
    }
}

/// Enumerate the integer values an integer-bounded volume span covers.
/// Volume spans are always integer in the schema; we cast through `i32`.
fn span_integer_iter(span: &NumericSpan) -> std::ops::RangeInclusive<i32> {
    let start = span.start.ceil() as i32;
    let end = span.end.floor() as i32;
    start..=end
}

/// Enumerate the chapter values a chapter span covers. Single-point spans
/// (`{12.5, 12.5}`) yield exactly the start (so decimals survive). Range
/// spans enumerate the integers from ceil(start)..=floor(end), and append
/// the start/end if they're non-integer to avoid silently accepting
/// integer-only coverage as ownership of `{1.5, 9.5}`.
fn chapter_span_values(span: &NumericSpan) -> Vec<f64> {
    if span.start == span.end {
        return vec![span.start];
    }
    let start_i = span.start.ceil() as i64;
    let end_i = span.end.floor() as i64;
    let mut out: Vec<f64> = (start_i..=end_i).map(|c| c as f64).collect();
    if span.start.fract() != 0.0 {
        out.push(span.start);
    }
    if span.end.fract() != 0.0 {
        out.push(span.end);
    }
    out
}

/// True when the user "owns" the `(v, c)` item (see the rules table on
/// [`should_auto_ignore`]).
fn pair_owned(v: i32, c: f64, owned: &OwnedReleaseKeys) -> bool {
    for (ov, oc) in &owned.keys {
        match (ov, oc) {
            // Exact pair.
            (Some(ov), Some(oc)) if *ov == v && chapter_eq(*oc, c) => return true,
            // Whole volume v.
            (Some(ov), None) if *ov == v => return true,
            // No-vol chapter c.
            (None, Some(oc)) if chapter_eq(*oc, c) => return true,
            _ => {}
        }
    }
    // Count fallback (volume side): no book has any volume metadata, but
    // we know at least N volumes are owned and v is within that count.
    if !owned.has_any_volume_metadata && owned.volumes_owned_count > 0 {
        return (v as i64) <= owned.volumes_owned_count;
    }
    false
}

/// True when the user owns whole volume `v`, or the count fallback applies.
fn volume_owned(v: i32, owned: &OwnedReleaseKeys) -> bool {
    let direct = owned
        .keys
        .iter()
        .any(|(ov, oc)| matches!((ov, oc), (Some(ov), None) if *ov == v));
    if direct {
        return true;
    }
    if !owned.has_any_volume_metadata && owned.volumes_owned_count > 0 {
        return (v as i64) <= owned.volumes_owned_count;
    }
    false
}

/// True when the user owns chapter `c` (any volume tag).
fn chapter_owned(c: f64, owned: &OwnedReleaseKeys) -> bool {
    owned
        .keys
        .iter()
        .any(|(_, oc)| matches!(oc, Some(oc) if chapter_eq(*oc, c)))
}

/// Tolerant equality for chapter numbers. `f64` because both sides come
/// from DB columns; the values are typically small decimals (e.g. `12.5`)
/// and exact equality is fine for the realistic range.
fn chapter_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(keys: Vec<(Option<i32>, Option<f64>)>) -> OwnedReleaseKeys {
        let has_any_volume_metadata = keys.iter().any(|(v, _)| v.is_some());
        let volumes_owned_count = keys
            .iter()
            .filter(|(v, c)| v.is_some() && c.is_none())
            .count() as i64;
        OwnedReleaseKeys {
            keys,
            has_any_volume_metadata,
            volumes_owned_count,
        }
    }

    /// Wrap a single integer volume value as a one-element span list.
    fn vol(v: i32) -> Vec<NumericSpan> {
        vec![NumericSpan {
            start: v as f64,
            end: v as f64,
        }]
    }

    /// Wrap a single chapter value as a one-element span list.
    fn chap(c: f64) -> Vec<NumericSpan> {
        vec![NumericSpan { start: c, end: c }]
    }

    /// Inclusive volume range as a single span.
    fn vol_range(start: i32, end: i32) -> Vec<NumericSpan> {
        vec![NumericSpan {
            start: start as f64,
            end: end as f64,
        }]
    }

    /// Inclusive chapter range as a single span.
    fn chap_range(start: f64, end: f64) -> Vec<NumericSpan> {
        vec![NumericSpan { start, end }]
    }

    /// Run `should_auto_ignore` against borrowed slices.
    fn ignore(
        v: Option<&Vec<NumericSpan>>,
        c: Option<&Vec<NumericSpan>>,
        o: &OwnedReleaseKeys,
    ) -> bool {
        should_auto_ignore(v.map(|s| s.as_slice()), c.map(|s| s.as_slice()), o)
    }

    // ---------- single-value (point span) backwards-compatibility tests ------

    #[test]
    fn volume_release_owned_as_whole_volume() {
        let o = owned(vec![(Some(1), None), (Some(2), None)]);
        assert!(ignore(Some(&vol(1)), None, &o));
        assert!(ignore(Some(&vol(2)), None, &o));
        assert!(!ignore(Some(&vol(3)), None, &o));
    }

    #[test]
    fn volume_release_not_matched_by_chapter_in_volume() {
        // User only has chapter 5 of volume 1, not the whole volume.
        let o = owned(vec![(Some(1), Some(5.0))]);
        assert!(!ignore(Some(&vol(1)), None, &o));
    }

    #[test]
    fn chapter_release_matches_any_volume() {
        let o = owned(vec![(Some(2), Some(12.0))]);
        // Release "Ch 12, vol unknown" → owned by virtue of having ch 12 of vol 2.
        assert!(ignore(None, Some(&chap(12.0)), &o));
        assert!(!ignore(None, Some(&chap(13.0)), &o));
    }

    #[test]
    fn chapter_release_matches_chapter_only_owned() {
        let o = owned(vec![(None, Some(7.0))]);
        assert!(ignore(None, Some(&chap(7.0)), &o));
        assert!(!ignore(None, Some(&chap(8.0)), &o));
    }

    #[test]
    fn chapter_release_not_matched_by_owned_volume() {
        // User owns volume 1 (whole). Release is "Ch 5".
        // We do NOT infer ch 5 is in vol 1 — chapter→volume mapping unreliable.
        let o = owned(vec![(Some(1), None)]);
        assert!(!ignore(None, Some(&chap(5.0)), &o));
    }

    #[test]
    fn vol_plus_chapter_release_matches_exact_pair() {
        // OR semantics: release {vol=1, chap=5} auto-ignores when EITHER
        // axis is fully owned. Owning chapter 5 (alone) covers the chapter
        // axis, and "matches an exact pair" is one way to satisfy that.
        let o = owned(vec![(Some(1), Some(5.0))]);
        assert!(ignore(Some(&vol(1)), Some(&chap(5.0)), &o));
        // No vol 1 ownership and no ch 6 ownership → not ignored.
        assert!(!ignore(Some(&vol(1)), Some(&chap(6.0)), &o));
        // No vol 2 ownership and no ch 5 ownership without vol 1 → not ignored.
        assert!(!ignore(Some(&vol(2)), Some(&chap(5.0)), &o));
    }

    #[test]
    fn vol_plus_chapter_release_matches_whole_volume() {
        // Whole volume satisfies the volume axis on its own.
        let o = owned(vec![(Some(1), None)]);
        assert!(ignore(Some(&vol(1)), Some(&chap(5.0)), &o));
        assert!(ignore(Some(&vol(1)), Some(&chap(99.5)), &o));
    }

    #[test]
    fn count_fallback_active_when_no_metadata() {
        // No book has volume metadata, but volumes_owned_count = 2.
        let o = OwnedReleaseKeys {
            keys: vec![],
            has_any_volume_metadata: false,
            volumes_owned_count: 2,
        };
        assert!(ignore(Some(&vol(1)), None, &o));
        assert!(ignore(Some(&vol(2)), None, &o));
        assert!(!ignore(Some(&vol(3)), None, &o));
    }

    #[test]
    fn count_fallback_inactive_when_metadata_present() {
        let o = owned(vec![(Some(3), None), (Some(5), None), (Some(7), None)]);
        assert!(!ignore(Some(&vol(1)), None, &o));
        assert!(ignore(Some(&vol(3)), None, &o));
        assert!(!ignore(Some(&vol(4)), None, &o));
    }

    #[test]
    fn count_fallback_does_not_apply_to_chapter_releases() {
        let o = OwnedReleaseKeys {
            keys: vec![],
            has_any_volume_metadata: false,
            volumes_owned_count: 5,
        };
        assert!(!ignore(None, Some(&chap(3.0)), &o));
    }

    #[test]
    fn release_with_no_volume_or_chapter_never_ignored() {
        let o = owned(vec![(Some(1), None)]);
        assert!(!ignore(None, None, &o));
    }

    #[test]
    fn empty_span_lists_treated_as_no_info() {
        let o = owned(vec![(Some(1), None)]);
        let empty: Vec<NumericSpan> = vec![];
        assert!(!ignore(Some(&empty), Some(&empty), &o));
    }

    #[test]
    fn empty_owned_set_never_ignores() {
        let o = OwnedReleaseKeys::default();
        assert!(!ignore(Some(&vol(1)), None, &o));
        assert!(!ignore(None, Some(&chap(1.0)), &o));
        assert!(!ignore(Some(&vol(1)), Some(&chap(1.0)), &o));
    }

    #[test]
    fn fractional_chapter_matches() {
        let o = owned(vec![(Some(1), Some(12.5))]);
        assert!(ignore(None, Some(&chap(12.5)), &o));
        assert!(!ignore(None, Some(&chap(12.0)), &o));
    }

    // ---------- range-aware tests --------------------------------------------

    #[test]
    fn volume_range_requires_full_ownership() {
        // Release v01-09. Owning vol 1 alone is not enough.
        let owns_only_vol_1 = owned(vec![(Some(1), None)]);
        assert!(!ignore(Some(&vol_range(1, 9)), None, &owns_only_vol_1));

        // Owning vols 1-9 satisfies the axis.
        let all = owned((1..=9).map(|v| (Some(v), None)).collect::<Vec<_>>());
        assert!(ignore(Some(&vol_range(1, 9)), None, &all));

        // Missing exactly one (vol 5) → not ignored.
        let missing_5 = owned(
            (1..=9)
                .filter(|v| *v != 5)
                .map(|v| (Some(v), None))
                .collect::<Vec<_>>(),
        );
        assert!(!ignore(Some(&vol_range(1, 9)), None, &missing_5));
    }

    #[test]
    fn disjoint_volume_spans_skip_the_gap() {
        // Release v01-04 + v06-09 (vol 5 not in the bundle).
        let spans = vec![
            NumericSpan {
                start: 1.0,
                end: 4.0,
            },
            NumericSpan {
                start: 6.0,
                end: 9.0,
            },
        ];
        // Owning vols 1-4 + 6-9 (without vol 5) covers exactly the bundle.
        let exact = owned(
            (1..=4)
                .chain(6..=9)
                .map(|v| (Some(v), None))
                .collect::<Vec<_>>(),
        );
        assert!(ignore(Some(&spans), None, &exact));

        // Owning vols 1-4 only → still missing 6-9 → not ignored.
        let half = owned((1..=4).map(|v| (Some(v), None)).collect::<Vec<_>>());
        assert!(!ignore(Some(&spans), None, &half));
    }

    #[test]
    fn chapter_range_requires_full_ownership() {
        // Release c031-037. Owning ch 31 alone isn't enough.
        let only_31 = owned(vec![(None, Some(31.0))]);
        assert!(!ignore(None, Some(&chap_range(31.0, 37.0)), &only_31));

        // Owning ch 31..=37 satisfies.
        let all = owned(
            (31..=37)
                .map(|c| (None, Some(c as f64)))
                .collect::<Vec<_>>(),
        );
        assert!(ignore(None, Some(&chap_range(31.0, 37.0)), &all));
    }

    #[test]
    fn either_axis_full_ownership_suffices_for_compilation() {
        // Release `001-050 as v01-10`: covers chs 1..=50 AND vols 1..=10.
        // OR-of-axes means owning all 10 volumes is enough, even when no
        // chapter-tagged ownership rows exist.
        let vols = vol_range(1, 10);
        let chaps = chap_range(1.0, 50.0);
        let owns_all_vols = owned((1..=10).map(|v| (Some(v), None)).collect::<Vec<_>>());
        assert!(ignore(Some(&vols), Some(&chaps), &owns_all_vols));

        // And owning all 50 chapters (no volume rows) is also enough.
        let owns_all_chaps = owned((1..=50).map(|c| (None, Some(c as f64))).collect::<Vec<_>>());
        assert!(ignore(Some(&vols), Some(&chaps), &owns_all_chaps));

        // Owning only vol 1 → neither axis full → not ignored.
        let owns_partial = owned(vec![(Some(1), None)]);
        assert!(!ignore(Some(&vols), Some(&chaps), &owns_partial));
    }

    #[test]
    fn count_fallback_works_against_volume_ranges() {
        // No metadata, but user has 5 volumes counted. Release v01-05 → ignored.
        let o = OwnedReleaseKeys {
            keys: vec![],
            has_any_volume_metadata: false,
            volumes_owned_count: 5,
        };
        assert!(ignore(Some(&vol_range(1, 5)), None, &o));
        // Release v01-06 → vol 6 not covered by count → not ignored.
        assert!(!ignore(Some(&vol_range(1, 6)), None, &o));
    }
}
