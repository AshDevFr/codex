//! Confidence-threshold gate and dedup-on-record orchestration.
//!
//! The plugin produces a [`ReleaseCandidate`]; the matcher decides whether
//! the host should record it in the ledger. Two-stage check:
//!
//! 1. Validate fields (no NaN, no empty IDs/URLs, sane `observed_at`).
//! 2. Confidence-threshold gate (default 0.7, override via per-series
//!    `confidence_threshold_override`).
//!
//! The actual ledger write goes through
//! [`crate::db::repositories::ReleaseLedgerRepository::record`], which is
//! itself idempotent on `(source_id, external_release_id)` and `info_hash`.

use chrono::Utc;
use uuid::Uuid;

use super::candidate::{CandidateReject, MAX_FUTURE_SKEW_S, ReleaseCandidate};
use crate::db::repositories::NewReleaseEntry;

/// Default confidence threshold (`0.7`).
pub const DEFAULT_CONFIDENCE_THRESHOLD: f64 = 0.7;

/// Validated candidate that has passed the threshold gate. Holds onto the
/// candidate so callers can map it directly into a ledger entry.
#[derive(Debug, Clone)]
pub struct AcceptedCandidate {
    pub candidate: ReleaseCandidate,
}

impl AcceptedCandidate {
    /// Convert into the repository-facing insert payload, attaching the
    /// `source_id` (the host knows which source the candidate came from -
    /// the candidate itself doesn't carry it).
    pub fn into_ledger_entry(self, source_id: Uuid) -> NewReleaseEntry {
        let c = self.candidate;
        let media_url_kind = c.media_url_kind.map(|k| k.as_str().to_string());
        NewReleaseEntry {
            series_id: c.series_match.codex_series_id,
            source_id,
            external_release_id: c.external_release_id,
            info_hash: c.info_hash,
            chapter: c.chapter,
            volume: c.volume,
            language: Some(c.language),
            format_hints: c.format_hints,
            group_or_uploader: c.group_or_uploader,
            payload_url: c.payload_url,
            media_url: c.media_url,
            media_url_kind,
            confidence: c.series_match.confidence,
            metadata: c.metadata,
            observed_at: c.observed_at,
            initial_state: None,
        }
    }
}

/// Validate a candidate and apply the confidence threshold.
///
/// Returns `Ok(AcceptedCandidate)` on accept, `Err(CandidateReject)` on reject.
pub fn evaluate(
    candidate: ReleaseCandidate,
    threshold: f64,
) -> Result<AcceptedCandidate, CandidateReject> {
    // 1. Required-field validation. We do this before the threshold check so
    //    a malformed-but-high-confidence candidate still gets rejected with
    //    the most informative error.
    if candidate.payload_url.trim().is_empty() {
        return Err(CandidateReject::EmptyPayloadUrl);
    }
    // media_url and media_url_kind must travel together. Either both
    // are present (and media_url is non-empty) or both are absent.
    match (&candidate.media_url, &candidate.media_url_kind) {
        (Some(url), Some(_)) if url.trim().is_empty() => {
            return Err(CandidateReject::EmptyMediaUrl);
        }
        (Some(_), None) | (None, Some(_)) => {
            return Err(CandidateReject::MediaUrlPairMismatch);
        }
        _ => {}
    }
    if candidate.external_release_id.trim().is_empty() {
        return Err(CandidateReject::EmptyExternalReleaseId);
    }
    if candidate.language.trim().is_empty() {
        return Err(CandidateReject::EmptyLanguage);
    }
    if let Some(ch) = candidate.chapter
        && !ch.is_finite()
    {
        return Err(CandidateReject::InvalidChapter);
    }

    let now = Utc::now();
    if (candidate.observed_at - now).num_seconds() > MAX_FUTURE_SKEW_S {
        return Err(CandidateReject::ObservedAtTooFarInFuture);
    }

    // 2. Confidence validation + threshold.
    let confidence = candidate.series_match.confidence;
    if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
        return Err(CandidateReject::InvalidConfidence(confidence));
    }
    if confidence < threshold {
        return Err(CandidateReject::BelowThreshold {
            confidence,
            threshold,
        });
    }

    Ok(AcceptedCandidate { candidate })
}

/// Resolve the active confidence threshold: per-series override wins, then
/// the global default.
pub fn resolve_threshold(per_series_override: Option<f64>) -> f64 {
    match per_series_override {
        Some(v) if v.is_finite() && (0.0..=1.0).contains(&v) => v,
        _ => DEFAULT_CONFIDENCE_THRESHOLD,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::release::candidate::SeriesMatch;
    use chrono::Duration;

    fn make_candidate(confidence: f64) -> ReleaseCandidate {
        ReleaseCandidate {
            series_match: SeriesMatch {
                codex_series_id: Uuid::new_v4(),
                confidence,
                reason: "test".to_string(),
            },
            external_release_id: "rel-1".to_string(),
            chapter: Some(143.0),
            volume: None,
            language: "en".to_string(),
            format_hints: None,
            group_or_uploader: None,
            payload_url: "https://example.com/r/1".to_string(),
            media_url: None,
            media_url_kind: None,
            info_hash: None,
            metadata: None,
            observed_at: Utc::now(),
        }
    }

    #[test]
    fn accepts_candidate_at_threshold() {
        let cand = make_candidate(0.7);
        let result = evaluate(cand, DEFAULT_CONFIDENCE_THRESHOLD);
        assert!(result.is_ok());
    }

    #[test]
    fn drops_below_threshold_candidate() {
        let cand = make_candidate(0.5);
        let err = evaluate(cand, DEFAULT_CONFIDENCE_THRESHOLD).unwrap_err();
        assert!(matches!(err, CandidateReject::BelowThreshold { .. }));
    }

    #[test]
    fn rejects_nan_confidence() {
        let cand = make_candidate(f64::NAN);
        let err = evaluate(cand, DEFAULT_CONFIDENCE_THRESHOLD).unwrap_err();
        assert!(matches!(err, CandidateReject::InvalidConfidence(_)));
    }

    #[test]
    fn rejects_out_of_range_confidence() {
        let cand = make_candidate(1.5);
        let err = evaluate(cand, DEFAULT_CONFIDENCE_THRESHOLD).unwrap_err();
        assert!(matches!(err, CandidateReject::InvalidConfidence(_)));
    }

    #[test]
    fn rejects_empty_payload_url() {
        let mut cand = make_candidate(0.95);
        cand.payload_url = "  ".to_string();
        let err = evaluate(cand, DEFAULT_CONFIDENCE_THRESHOLD).unwrap_err();
        assert_eq!(err, CandidateReject::EmptyPayloadUrl);
    }

    #[test]
    fn rejects_empty_external_release_id() {
        let mut cand = make_candidate(0.95);
        cand.external_release_id = "".to_string();
        let err = evaluate(cand, DEFAULT_CONFIDENCE_THRESHOLD).unwrap_err();
        assert_eq!(err, CandidateReject::EmptyExternalReleaseId);
    }

    #[test]
    fn rejects_empty_language() {
        let mut cand = make_candidate(0.95);
        cand.language = "".to_string();
        let err = evaluate(cand, DEFAULT_CONFIDENCE_THRESHOLD).unwrap_err();
        assert_eq!(err, CandidateReject::EmptyLanguage);
    }

    #[test]
    fn rejects_invalid_chapter() {
        let mut cand = make_candidate(0.95);
        cand.chapter = Some(f64::INFINITY);
        let err = evaluate(cand, DEFAULT_CONFIDENCE_THRESHOLD).unwrap_err();
        assert_eq!(err, CandidateReject::InvalidChapter);
    }

    #[test]
    fn rejects_far_future_observed_at() {
        let mut cand = make_candidate(0.95);
        cand.observed_at = Utc::now() + Duration::days(2);
        let err = evaluate(cand, DEFAULT_CONFIDENCE_THRESHOLD).unwrap_err();
        assert_eq!(err, CandidateReject::ObservedAtTooFarInFuture);
    }

    #[test]
    fn accepts_candidate_within_clock_skew() {
        let mut cand = make_candidate(0.95);
        // Within MAX_FUTURE_SKEW_S grace.
        cand.observed_at = Utc::now() + Duration::seconds(60);
        assert!(evaluate(cand, DEFAULT_CONFIDENCE_THRESHOLD).is_ok());
    }

    #[test]
    fn into_ledger_entry_carries_all_fields() {
        let cand = make_candidate(0.85);
        let series_id = cand.series_match.codex_series_id;
        let accepted = evaluate(cand, DEFAULT_CONFIDENCE_THRESHOLD).unwrap();
        let source_id = Uuid::new_v4();
        let entry = accepted.into_ledger_entry(source_id);
        assert_eq!(entry.series_id, series_id);
        assert_eq!(entry.source_id, source_id);
        assert_eq!(entry.external_release_id, "rel-1");
        assert_eq!(entry.confidence, 0.85);
        assert_eq!(entry.language.as_deref(), Some("en"));
        assert!(entry.media_url.is_none());
        assert!(entry.media_url_kind.is_none());
    }

    #[test]
    fn into_ledger_entry_carries_media_url_pair() {
        use crate::services::release::candidate::MediaUrlKind;
        let mut cand = make_candidate(0.9);
        cand.media_url = Some("https://nyaa.si/download/1.torrent".to_string());
        cand.media_url_kind = Some(MediaUrlKind::Torrent);
        let entry = evaluate(cand, DEFAULT_CONFIDENCE_THRESHOLD)
            .unwrap()
            .into_ledger_entry(Uuid::new_v4());
        assert_eq!(
            entry.media_url.as_deref(),
            Some("https://nyaa.si/download/1.torrent")
        );
        assert_eq!(entry.media_url_kind.as_deref(), Some("torrent"));
    }

    #[test]
    fn rejects_media_url_without_kind() {
        let mut cand = make_candidate(0.95);
        cand.media_url = Some("https://example.com/x.torrent".to_string());
        cand.media_url_kind = None;
        let err = evaluate(cand, DEFAULT_CONFIDENCE_THRESHOLD).unwrap_err();
        assert_eq!(err, CandidateReject::MediaUrlPairMismatch);
    }

    #[test]
    fn rejects_kind_without_media_url() {
        use crate::services::release::candidate::MediaUrlKind;
        let mut cand = make_candidate(0.95);
        cand.media_url = None;
        cand.media_url_kind = Some(MediaUrlKind::Torrent);
        let err = evaluate(cand, DEFAULT_CONFIDENCE_THRESHOLD).unwrap_err();
        assert_eq!(err, CandidateReject::MediaUrlPairMismatch);
    }

    #[test]
    fn rejects_empty_media_url() {
        use crate::services::release::candidate::MediaUrlKind;
        let mut cand = make_candidate(0.95);
        cand.media_url = Some("   ".to_string());
        cand.media_url_kind = Some(MediaUrlKind::Torrent);
        let err = evaluate(cand, DEFAULT_CONFIDENCE_THRESHOLD).unwrap_err();
        assert_eq!(err, CandidateReject::EmptyMediaUrl);
    }

    #[test]
    fn resolve_threshold_uses_default_when_override_is_none() {
        assert_eq!(resolve_threshold(None), DEFAULT_CONFIDENCE_THRESHOLD);
    }

    #[test]
    fn resolve_threshold_uses_override_when_valid() {
        assert_eq!(resolve_threshold(Some(0.5)), 0.5);
    }

    #[test]
    fn resolve_threshold_falls_back_for_invalid_override() {
        assert_eq!(
            resolve_threshold(Some(f64::NAN)),
            DEFAULT_CONFIDENCE_THRESHOLD
        );
        assert_eq!(resolve_threshold(Some(1.5)), DEFAULT_CONFIDENCE_THRESHOLD);
        assert_eq!(resolve_threshold(Some(-0.1)), DEFAULT_CONFIDENCE_THRESHOLD);
    }
}
