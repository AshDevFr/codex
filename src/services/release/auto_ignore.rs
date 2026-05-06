//! Decide whether an incoming release matches something the user already
//! owns, so ingestion can mark it `ignored` instead of `announced`.
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

/// True when the release matches a directly-owned key.
///
/// Matching rules:
/// - **Volume + chapter release**: matches an owned `(Some(v), Some(c))`,
///   or an owned whole volume `(Some(v), None)` (whole volume implies all
///   chapters in it).
/// - **Volume-only release**: matches an owned whole volume
///   `(Some(v), None)`. Does NOT match if the user only owns specific
///   chapters of that volume.
/// - **Chapter-only release**: matches any owned key with the same
///   chapter, regardless of volume.
/// - **No volume and no chapter**: never auto-ignored.
///
/// **Count fallback**: only when `has_any_volume_metadata` is false (no
/// book has volume metadata at all). For a volume-N release, treat
/// `1..=volumes_owned_count` as owned. We do not apply the count fallback
/// to chapter-only releases.
pub fn should_auto_ignore(
    release_volume: Option<i32>,
    release_chapter: Option<f64>,
    owned: &OwnedReleaseKeys,
) -> bool {
    match (release_volume, release_chapter) {
        (None, None) => false,

        (Some(v), Some(c)) => owned.keys.iter().any(|(ov, oc)| match (ov, oc) {
            (Some(ov), Some(oc)) => *ov == v && chapter_eq(*oc, c),
            (Some(ov), None) => *ov == v,
            _ => false,
        }),

        (Some(v), None) => {
            let direct = owned
                .keys
                .iter()
                .any(|(ov, oc)| matches!((ov, oc), (Some(ov), None) if *ov == v));
            if direct {
                return true;
            }
            // Count fallback: only when no book has volume metadata.
            if !owned.has_any_volume_metadata && owned.volumes_owned_count > 0 {
                return (v as i64) <= owned.volumes_owned_count;
            }
            false
        }

        (None, Some(c)) => owned
            .keys
            .iter()
            .any(|(_, oc)| matches!(oc, Some(oc) if chapter_eq(*oc, c))),
    }
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

    #[test]
    fn volume_release_owned_as_whole_volume() {
        let o = owned(vec![(Some(1), None), (Some(2), None)]);
        assert!(should_auto_ignore(Some(1), None, &o));
        assert!(should_auto_ignore(Some(2), None, &o));
        assert!(!should_auto_ignore(Some(3), None, &o));
    }

    #[test]
    fn volume_release_not_matched_by_chapter_in_volume() {
        // User only has chapter 5 of volume 1, not the whole volume.
        let o = owned(vec![(Some(1), Some(5.0))]);
        assert!(!should_auto_ignore(Some(1), None, &o));
    }

    #[test]
    fn chapter_release_matches_any_volume() {
        let o = owned(vec![(Some(2), Some(12.0))]);
        // Release "Ch 12, vol unknown" → owned by virtue of having ch 12 of vol 2.
        assert!(should_auto_ignore(None, Some(12.0), &o));
        assert!(!should_auto_ignore(None, Some(13.0), &o));
    }

    #[test]
    fn chapter_release_matches_chapter_only_owned() {
        let o = owned(vec![(None, Some(7.0))]);
        assert!(should_auto_ignore(None, Some(7.0), &o));
        assert!(!should_auto_ignore(None, Some(8.0), &o));
    }

    #[test]
    fn chapter_release_not_matched_by_owned_volume() {
        // User owns volume 1 (whole). Release is "Ch 5".
        // We do NOT infer ch 5 is in vol 1 — chapter→volume mapping unreliable.
        let o = owned(vec![(Some(1), None)]);
        assert!(!should_auto_ignore(None, Some(5.0), &o));
    }

    #[test]
    fn vol_plus_chapter_release_matches_exact_pair() {
        let o = owned(vec![(Some(1), Some(5.0))]);
        assert!(should_auto_ignore(Some(1), Some(5.0), &o));
        assert!(!should_auto_ignore(Some(1), Some(6.0), &o));
        assert!(!should_auto_ignore(Some(2), Some(5.0), &o));
    }

    #[test]
    fn vol_plus_chapter_release_matches_whole_volume() {
        // Whole volume implies all chapters in it.
        let o = owned(vec![(Some(1), None)]);
        assert!(should_auto_ignore(Some(1), Some(5.0), &o));
        assert!(should_auto_ignore(Some(1), Some(99.5), &o));
    }

    #[test]
    fn count_fallback_active_when_no_metadata() {
        // No book has volume metadata, but volumes_owned_count = 2.
        let o = OwnedReleaseKeys {
            keys: vec![],
            has_any_volume_metadata: false,
            volumes_owned_count: 2,
        };
        assert!(should_auto_ignore(Some(1), None, &o));
        assert!(should_auto_ignore(Some(2), None, &o));
        assert!(!should_auto_ignore(Some(3), None, &o));
    }

    #[test]
    fn count_fallback_inactive_when_metadata_present() {
        // User owns vols 3, 5, 7 (with metadata). Count fallback must NOT
        // hide vol 1 — that's the bug the metadata path fixes.
        let o = owned(vec![(Some(3), None), (Some(5), None), (Some(7), None)]);
        assert!(!should_auto_ignore(Some(1), None, &o));
        assert!(should_auto_ignore(Some(3), None, &o));
        assert!(!should_auto_ignore(Some(4), None, &o));
    }

    #[test]
    fn count_fallback_does_not_apply_to_chapter_releases() {
        let o = OwnedReleaseKeys {
            keys: vec![],
            has_any_volume_metadata: false,
            volumes_owned_count: 5,
        };
        assert!(!should_auto_ignore(None, Some(3.0), &o));
    }

    #[test]
    fn release_with_no_volume_or_chapter_never_ignored() {
        let o = owned(vec![(Some(1), None)]);
        assert!(!should_auto_ignore(None, None, &o));
    }

    #[test]
    fn empty_owned_set_never_ignores() {
        let o = OwnedReleaseKeys::default();
        assert!(!should_auto_ignore(Some(1), None, &o));
        assert!(!should_auto_ignore(None, Some(1.0), &o));
        assert!(!should_auto_ignore(Some(1), Some(1.0), &o));
    }

    #[test]
    fn fractional_chapter_matches() {
        let o = owned(vec![(Some(1), Some(12.5))]);
        assert!(should_auto_ignore(None, Some(12.5), &o));
        assert!(!should_auto_ignore(None, Some(12.0), &o));
    }
}
