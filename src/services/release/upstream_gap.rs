//! Upstream-publication gap signal (Phase 5 of release-tracking).
//!
//! Computes the per-series delta between *original-language* publication
//! counts (from MangaBaka / AniList / etc., stored as
//! `series_metadata.total_chapter_count` / `total_volume_count`) and
//! *local* counts (the highest classified `book_metadata.chapter|volume`
//! across the series, surfaced as `local_max_chapter` / `local_max_volume`).
//!
//! The gap is purely a UI signal — it does **not** write `release_ledger`
//! rows and does **not** advance `series_tracking.latest_known_*`. Original-
//! language publication facts are not the same category as
//! translation/scanlation releases (which Phase 6's MangaUpdates plugin
//! handles). See the `release-tracking` plan, Key Technical Decisions, for
//! the three-signal separation.

use crate::db::entities::series_external_ids::Model as SeriesExternalId;
use crate::db::entities::series_tracking::Model as SeriesTrackingRow;

/// Computed gap between upstream publication and local content for a series.
///
/// `None` fields collapse the corresponding badge in the UI: untracked
/// series, axis-disabled series (`track_chapters: false`), missing provider
/// counts, and zero/negative gaps all yield `None` for that axis.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UpstreamGap {
    pub chapter_gap: Option<f32>,
    pub volume_gap: Option<i32>,
    /// Display name of the metadata provider that supplied the upstream
    /// counts (e.g., "MangaBaka", "AniList"). Populated whenever at least
    /// one axis has a positive gap; set to `None` when both axes are `None`
    /// or when no recognized provider external ID is associated with the
    /// series.
    pub provider: Option<String>,
}

impl UpstreamGap {
    /// Returns `true` when neither axis has a positive gap.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.chapter_gap.is_none() && self.volume_gap.is_none()
    }
}

/// Inputs for computing the upstream gap. All inputs are already loaded by
/// the series DTO build path (no new query required).
pub struct UpstreamGapInputs<'a> {
    pub tracking: Option<&'a SeriesTrackingRow>,
    pub total_chapter_count: Option<f32>,
    pub total_volume_count: Option<i32>,
    pub local_max_chapter: Option<f32>,
    pub local_max_volume: Option<i32>,
    pub external_ids: &'a [SeriesExternalId],
}

/// Compute the upstream gap for a series given preloaded inputs.
///
/// Returns an empty `UpstreamGap` when:
/// - the series is not tracked (no row, or `tracked = false`);
/// - the corresponding `track_*` axis is disabled;
/// - the provider count is `None`;
/// - the local max is `None` (chapter axis only — for the volume axis a
///   missing local max is treated as `0` so a brand-new tracked series with
///   `total_volume_count = 14` and no local books shows "+14 vol upstream").
///
/// Float chapter math is rounded to 1 decimal place to suppress
/// `145.0 - 144.9999 = 0.0001`-style noise.
pub fn compute_upstream_gap(inputs: &UpstreamGapInputs<'_>) -> UpstreamGap {
    let tracking = match inputs.tracking {
        Some(t) if t.tracked => t,
        _ => return UpstreamGap::default(),
    };

    let chapter_gap = if tracking.track_chapters {
        compute_chapter_gap(inputs.total_chapter_count, inputs.local_max_chapter)
    } else {
        None
    };

    let volume_gap = if tracking.track_volumes {
        compute_volume_gap(inputs.total_volume_count, inputs.local_max_volume)
    } else {
        None
    };

    let provider = if chapter_gap.is_some() || volume_gap.is_some() {
        pick_provider(inputs.external_ids)
    } else {
        None
    };

    UpstreamGap {
        chapter_gap,
        volume_gap,
        provider,
    }
}

fn compute_chapter_gap(total: Option<f32>, local_max: Option<f32>) -> Option<f32> {
    let total = total?;
    // Treat a missing local max as 0 so newly-tracked series surface the
    // full upstream count rather than silently hiding it.
    let local = local_max.unwrap_or(0.0);
    let raw = total - local;
    let rounded = (raw * 10.0).round() / 10.0;
    if rounded > 0.0 { Some(rounded) } else { None }
}

fn compute_volume_gap(total: Option<i32>, local_max: Option<i32>) -> Option<i32> {
    let total = total?;
    let local = local_max.unwrap_or(0);
    let gap = total - local;
    if gap > 0 { Some(gap) } else { None }
}

/// Pick the provider display name to attribute the gap to.
///
/// We have no per-field provenance on `series_metadata.total_*_count`
/// (every metadata-provider plugin merges into the same column). This
/// helper falls back to a fixed priority order keyed off the series'
/// external IDs — MangaBaka first (it's the primary count source for
/// manga), then AniList, MAL, MangaDex, and finally any other plugin
/// source. Manual / file-derived sources (`comicinfo`, `epub`, `manual`)
/// are not displayed as providers because they don't supply upstream
/// counts.
///
/// Returns `None` when no recognized provider source is attached to the
/// series; the badge tooltip in Phase 7 then falls back to a generic
/// message.
fn pick_provider(external_ids: &[SeriesExternalId]) -> Option<String> {
    const PRIORITY: &[(&str, &str)] = &[
        ("plugin:mangabaka", "MangaBaka"),
        ("plugin:anilist", "AniList"),
        ("plugin:myanimelist", "MyAnimeList"),
        ("plugin:mangadex", "MangaDex"),
        ("plugin:kitsu", "Kitsu"),
        ("plugin:comicvine", "ComicVine"),
        ("plugin:openlibrary", "OpenLibrary"),
    ];

    for (source_key, display) in PRIORITY {
        if external_ids.iter().any(|x| x.source == *source_key) {
            return Some((*display).to_string());
        }
    }

    // Fallback: any plugin source that wasn't in the priority list.
    external_ids
        .iter()
        .find_map(|x| x.plugin_name().map(capitalize))
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().chain(chars).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn tracking_row(tracked: bool, track_chapters: bool, track_volumes: bool) -> SeriesTrackingRow {
        SeriesTrackingRow {
            series_id: Uuid::new_v4(),
            tracked,
            tracking_status: "ongoing".to_string(),
            track_chapters,
            track_volumes,
            latest_known_chapter: None,
            latest_known_volume: None,
            volume_chapter_map: None,
            poll_interval_override_s: None,
            confidence_threshold_override: None,
            languages: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn ext_id(source: &str) -> SeriesExternalId {
        SeriesExternalId {
            id: Uuid::new_v4(),
            series_id: Uuid::new_v4(),
            source: source.to_string(),
            external_id: "1234".to_string(),
            external_url: None,
            metadata_hash: None,
            last_synced_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn untracked_series_has_no_gap() {
        let tracking = tracking_row(false, true, true);
        let inputs = UpstreamGapInputs {
            tracking: Some(&tracking),
            total_chapter_count: Some(145.0),
            total_volume_count: Some(15),
            local_max_chapter: Some(142.0),
            local_max_volume: Some(14),
            external_ids: &[ext_id("plugin:mangabaka")],
        };
        let gap = compute_upstream_gap(&inputs);
        assert!(gap.is_empty());
        assert_eq!(gap.provider, None);
    }

    #[test]
    fn no_tracking_row_has_no_gap() {
        let inputs = UpstreamGapInputs {
            tracking: None,
            total_chapter_count: Some(145.0),
            total_volume_count: Some(15),
            local_max_chapter: Some(142.0),
            local_max_volume: Some(14),
            external_ids: &[ext_id("plugin:mangabaka")],
        };
        let gap = compute_upstream_gap(&inputs);
        assert!(gap.is_empty());
    }

    #[test]
    fn tracked_series_with_provider_ahead_returns_positive_gap() {
        let tracking = tracking_row(true, true, true);
        let inputs = UpstreamGapInputs {
            tracking: Some(&tracking),
            total_chapter_count: Some(145.0),
            total_volume_count: Some(15),
            local_max_chapter: Some(142.0),
            local_max_volume: Some(14),
            external_ids: &[ext_id("plugin:mangabaka")],
        };
        let gap = compute_upstream_gap(&inputs);
        assert_eq!(gap.chapter_gap, Some(3.0));
        assert_eq!(gap.volume_gap, Some(1));
        assert_eq!(gap.provider.as_deref(), Some("MangaBaka"));
    }

    #[test]
    fn track_chapters_false_suppresses_chapter_gap_only() {
        let tracking = tracking_row(true, false, true);
        let inputs = UpstreamGapInputs {
            tracking: Some(&tracking),
            total_chapter_count: Some(145.0),
            total_volume_count: Some(15),
            local_max_chapter: Some(142.0),
            local_max_volume: Some(14),
            external_ids: &[ext_id("plugin:mangabaka")],
        };
        let gap = compute_upstream_gap(&inputs);
        assert_eq!(gap.chapter_gap, None);
        assert_eq!(gap.volume_gap, Some(1));
        assert_eq!(gap.provider.as_deref(), Some("MangaBaka"));
    }

    #[test]
    fn track_volumes_false_suppresses_volume_gap_only() {
        let tracking = tracking_row(true, true, false);
        let inputs = UpstreamGapInputs {
            tracking: Some(&tracking),
            total_chapter_count: Some(145.0),
            total_volume_count: Some(15),
            local_max_chapter: Some(142.0),
            local_max_volume: Some(14),
            external_ids: &[ext_id("plugin:mangabaka")],
        };
        let gap = compute_upstream_gap(&inputs);
        assert_eq!(gap.chapter_gap, Some(3.0));
        assert_eq!(gap.volume_gap, None);
    }

    #[test]
    fn missing_provider_count_suppresses_axis() {
        let tracking = tracking_row(true, true, true);
        let inputs = UpstreamGapInputs {
            tracking: Some(&tracking),
            total_chapter_count: None,
            total_volume_count: Some(15),
            local_max_chapter: Some(142.0),
            local_max_volume: Some(14),
            external_ids: &[ext_id("plugin:mangabaka")],
        };
        let gap = compute_upstream_gap(&inputs);
        assert_eq!(gap.chapter_gap, None);
        assert_eq!(gap.volume_gap, Some(1));
    }

    #[test]
    fn zero_or_negative_gap_yields_none() {
        let tracking = tracking_row(true, true, true);
        let inputs = UpstreamGapInputs {
            tracking: Some(&tracking),
            total_chapter_count: Some(142.0),
            total_volume_count: Some(14),
            local_max_chapter: Some(142.0),
            local_max_volume: Some(14),
            external_ids: &[ext_id("plugin:mangabaka")],
        };
        let gap = compute_upstream_gap(&inputs);
        assert!(gap.is_empty());

        let inputs_local_ahead = UpstreamGapInputs {
            tracking: Some(&tracking),
            total_chapter_count: Some(140.0),
            total_volume_count: Some(13),
            local_max_chapter: Some(142.0),
            local_max_volume: Some(14),
            external_ids: &[ext_id("plugin:mangabaka")],
        };
        assert!(compute_upstream_gap(&inputs_local_ahead).is_empty());
    }

    #[test]
    fn float_noise_within_one_decimal_collapses_to_no_gap() {
        let tracking = tracking_row(true, true, true);
        let inputs = UpstreamGapInputs {
            tracking: Some(&tracking),
            total_chapter_count: Some(145.0),
            total_volume_count: None,
            local_max_chapter: Some(144.9999),
            local_max_volume: None,
            external_ids: &[ext_id("plugin:mangabaka")],
        };
        let gap = compute_upstream_gap(&inputs);
        // 145.0 - 144.9999 = 0.0001 -> rounds to 0.0 -> None.
        assert_eq!(gap.chapter_gap, None);
    }

    #[test]
    fn fractional_chapter_gap_rounds_to_one_decimal() {
        let tracking = tracking_row(true, true, true);
        let inputs = UpstreamGapInputs {
            tracking: Some(&tracking),
            total_chapter_count: Some(145.5),
            total_volume_count: None,
            local_max_chapter: Some(143.0),
            local_max_volume: None,
            external_ids: &[ext_id("plugin:mangabaka")],
        };
        let gap = compute_upstream_gap(&inputs);
        assert_eq!(gap.chapter_gap, Some(2.5));
    }

    #[test]
    fn missing_local_max_chapter_treats_local_as_zero() {
        let tracking = tracking_row(true, true, true);
        let inputs = UpstreamGapInputs {
            tracking: Some(&tracking),
            total_chapter_count: Some(10.0),
            total_volume_count: None,
            local_max_chapter: None,
            local_max_volume: None,
            external_ids: &[ext_id("plugin:mangabaka")],
        };
        let gap = compute_upstream_gap(&inputs);
        assert_eq!(gap.chapter_gap, Some(10.0));
    }

    #[test]
    fn provider_priority_prefers_mangabaka_over_anilist() {
        let tracking = tracking_row(true, true, true);
        let inputs = UpstreamGapInputs {
            tracking: Some(&tracking),
            total_chapter_count: Some(145.0),
            total_volume_count: None,
            local_max_chapter: Some(142.0),
            local_max_volume: None,
            external_ids: &[ext_id("plugin:anilist"), ext_id("plugin:mangabaka")],
        };
        let gap = compute_upstream_gap(&inputs);
        assert_eq!(gap.provider.as_deref(), Some("MangaBaka"));
    }

    #[test]
    fn provider_falls_back_to_anilist_when_no_mangabaka() {
        let tracking = tracking_row(true, true, true);
        let inputs = UpstreamGapInputs {
            tracking: Some(&tracking),
            total_chapter_count: Some(145.0),
            total_volume_count: None,
            local_max_chapter: Some(142.0),
            local_max_volume: None,
            external_ids: &[ext_id("plugin:anilist")],
        };
        let gap = compute_upstream_gap(&inputs);
        assert_eq!(gap.provider.as_deref(), Some("AniList"));
    }

    #[test]
    fn provider_uses_unknown_plugin_as_capitalized_fallback() {
        let tracking = tracking_row(true, true, true);
        let inputs = UpstreamGapInputs {
            tracking: Some(&tracking),
            total_chapter_count: Some(145.0),
            total_volume_count: None,
            local_max_chapter: Some(142.0),
            local_max_volume: None,
            external_ids: &[ext_id("plugin:newprovider")],
        };
        let gap = compute_upstream_gap(&inputs);
        assert_eq!(gap.provider.as_deref(), Some("Newprovider"));
    }

    #[test]
    fn non_plugin_sources_yield_no_provider() {
        let tracking = tracking_row(true, true, true);
        let inputs = UpstreamGapInputs {
            tracking: Some(&tracking),
            total_chapter_count: Some(145.0),
            total_volume_count: None,
            local_max_chapter: Some(142.0),
            local_max_volume: None,
            external_ids: &[ext_id("comicinfo"), ext_id("manual")],
        };
        let gap = compute_upstream_gap(&inputs);
        assert_eq!(gap.chapter_gap, Some(3.0));
        assert_eq!(gap.provider, None);
    }

    #[test]
    fn provider_omitted_when_both_axes_have_no_gap() {
        let tracking = tracking_row(true, true, true);
        let inputs = UpstreamGapInputs {
            tracking: Some(&tracking),
            total_chapter_count: Some(142.0),
            total_volume_count: Some(14),
            local_max_chapter: Some(142.0),
            local_max_volume: Some(14),
            external_ids: &[ext_id("plugin:mangabaka")],
        };
        let gap = compute_upstream_gap(&inputs);
        assert!(gap.is_empty());
        assert_eq!(gap.provider, None);
    }
}
