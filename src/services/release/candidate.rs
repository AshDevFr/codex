//! Wire-format `ReleaseCandidate` and parsing helpers.
//!
//! Plugins emit candidates over `releases/record` (and as the response of
//! `releases/poll` in Phase 4). The host rejects malformed candidates and
//! drops below-threshold candidates before reaching the ledger.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A release candidate emitted by a `release_source` plugin.
///
/// The series match is split out into its own struct so the plugin can
/// communicate *why* it matched (alias hit vs external-ID hit) and *how
/// confident* it is. The host applies the threshold gate against the
/// `series_match.confidence` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseCandidate {
    pub series_match: SeriesMatch,
    /// Stable per-source release identifier (e.g. Nyaa view ID, MU release ID).
    pub external_release_id: String,
    /// Optional chapter number; supports decimals for fractional chapters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapter: Option<f64>,
    /// Optional volume number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<i32>,
    /// ISO 639-1 language code (`"en"`, `"es"`, etc.).
    pub language: String,
    /// Free-form per-source format hints (e.g. `{"jxl": true}`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format_hints: Option<serde_json::Value>,
    /// Group or uploader name for display.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_or_uploader: Option<String>,
    /// URL the user can navigate to in order to acquire/read the release.
    /// Conventionally a human-readable landing page (Nyaa view page,
    /// MangaUpdates release page).
    pub payload_url: String,
    /// Optional second URL describing how to actually fetch the bits — a
    /// `.torrent` file, a magnet link, or a direct download. Set together
    /// with [`Self::media_url_kind`] or leave both unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_url: Option<String>,
    /// Classifies what [`Self::media_url`] points at. Required when
    /// `media_url` is set, must be unset otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_url_kind: Option<MediaUrlKind>,
    /// Optional torrent info hash (enables cross-source dedup).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_hash: Option<String>,
    /// Free-form metadata bag (preserved on the ledger row).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// When the upstream source observed this release.
    pub observed_at: DateTime<Utc>,
}

/// Classifies what [`ReleaseCandidate::media_url`] points at so the UI can
/// pick a kind-specific icon and the host can surface it consistently across
/// sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaUrlKind {
    /// HTTP(S) URL to a `.torrent` file.
    Torrent,
    /// `magnet:` URI.
    Magnet,
    /// HTTP(S) URL to the file itself (DDL host, CDN, etc.).
    Direct,
    /// Anything that doesn't fit the above; UI renders a generic icon.
    Other,
}

impl MediaUrlKind {
    /// Stable, lowercase string used in the database column and on the
    /// wire. Mirrors the serde `rename_all = "lowercase"` representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Torrent => "torrent",
            Self::Magnet => "magnet",
            Self::Direct => "direct",
            Self::Other => "other",
        }
    }
}

/// Match details emitted alongside a candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesMatch {
    /// Codex series ID (UUID).
    pub codex_series_id: Uuid,
    /// `0.0..=1.0`. The host drops candidates below the threshold.
    pub confidence: f64,
    /// Free-form reason string for tracing/debug. e.g. `"alias-exact"`,
    /// `"mangaupdates_id"`, `"normalized-prefix"`.
    pub reason: String,
}

/// Reason a candidate was rejected by [`super::matcher::evaluate`].
#[derive(Debug, Clone, PartialEq)]
pub enum CandidateReject {
    /// `series_match.confidence` is NaN or outside `0.0..=1.0`.
    InvalidConfidence(f64),
    /// `series_match.confidence < threshold`.
    BelowThreshold { confidence: f64, threshold: f64 },
    /// `payload_url` is empty / whitespace.
    EmptyPayloadUrl,
    /// `media_url` is set but empty / whitespace.
    EmptyMediaUrl,
    /// `media_url` is set without `media_url_kind`, or vice versa.
    /// The host requires the two travel together so the UI can always
    /// resolve a kind for the icon.
    MediaUrlPairMismatch,
    /// `external_release_id` is empty / whitespace.
    EmptyExternalReleaseId,
    /// `language` is empty.
    EmptyLanguage,
    /// `chapter` is NaN or non-finite.
    InvalidChapter,
    /// `observed_at` is in the future by more than [`MAX_FUTURE_SKEW_S`] seconds.
    ObservedAtTooFarInFuture,
}

impl std::fmt::Display for CandidateReject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfidence(v) => write!(
                f,
                "invalid confidence value {} (must be a finite number in [0, 1])",
                v
            ),
            Self::BelowThreshold {
                confidence,
                threshold,
            } => write!(f, "confidence {} below threshold {}", confidence, threshold),
            Self::EmptyPayloadUrl => write!(f, "payload_url cannot be empty"),
            Self::EmptyMediaUrl => write!(f, "media_url cannot be empty when set"),
            Self::MediaUrlPairMismatch => write!(
                f,
                "media_url and media_url_kind must be set together (or both unset)"
            ),
            Self::EmptyExternalReleaseId => write!(f, "external_release_id cannot be empty"),
            Self::EmptyLanguage => write!(f, "language cannot be empty"),
            Self::InvalidChapter => write!(f, "chapter must be a finite number"),
            Self::ObservedAtTooFarInFuture => {
                write!(f, "observed_at is too far in the future")
            }
        }
    }
}

/// Maximum allowable skew when validating `observed_at` (1 hour).
///
/// Plugins occasionally see clock skew between their host and the upstream
/// feed. We accept a small grace window so a slightly-future timestamp doesn't
/// drop the candidate, but reject obvious garbage (e.g. year 2099 dates).
pub const MAX_FUTURE_SKEW_S: i64 = 3_600;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn good_candidate() -> ReleaseCandidate {
        ReleaseCandidate {
            series_match: SeriesMatch {
                codex_series_id: Uuid::new_v4(),
                confidence: 0.92,
                reason: "alias-exact".to_string(),
            },
            external_release_id: "rel-123".to_string(),
            chapter: Some(143.0),
            volume: None,
            language: "en".to_string(),
            format_hints: Some(json!({"jxl": true})),
            group_or_uploader: Some("tsuna69".to_string()),
            payload_url: "https://nyaa.si/view/12345".to_string(),
            media_url: Some("https://nyaa.si/download/12345.torrent".to_string()),
            media_url_kind: Some(MediaUrlKind::Torrent),
            info_hash: Some("deadbeef".to_string()),
            metadata: None,
            observed_at: Utc::now(),
        }
    }

    #[test]
    fn round_trips_camel_case_json() {
        let cand = good_candidate();
        let json = serde_json::to_value(&cand).unwrap();
        // Field naming sanity checks.
        assert!(json["seriesMatch"].is_object());
        assert_eq!(
            json["seriesMatch"]["codexSeriesId"],
            json!(cand.series_match.codex_series_id)
        );
        assert_eq!(json["externalReleaseId"], "rel-123");
        assert_eq!(json["payloadUrl"], "https://nyaa.si/view/12345");
        let back: ReleaseCandidate = serde_json::from_value(json).unwrap();
        assert_eq!(back.external_release_id, cand.external_release_id);
        assert_eq!(back.series_match.confidence, cand.series_match.confidence);
    }

    #[test]
    fn optional_fields_are_skipped_when_none() {
        let mut cand = good_candidate();
        cand.chapter = None;
        cand.volume = None;
        cand.format_hints = None;
        cand.info_hash = None;
        cand.metadata = None;
        cand.group_or_uploader = None;
        cand.media_url = None;
        cand.media_url_kind = None;
        let json = serde_json::to_value(&cand).unwrap();
        let obj = json.as_object().unwrap();
        for key in [
            "chapter",
            "volume",
            "formatHints",
            "infoHash",
            "metadata",
            "groupOrUploader",
            "mediaUrl",
            "mediaUrlKind",
        ] {
            assert!(!obj.contains_key(key), "expected `{}` to be skipped", key);
        }
    }

    #[test]
    fn media_url_kind_serializes_lowercase() {
        let cand = good_candidate();
        let json = serde_json::to_value(&cand).unwrap();
        assert_eq!(json["mediaUrl"], "https://nyaa.si/download/12345.torrent");
        assert_eq!(json["mediaUrlKind"], "torrent");
    }

    #[test]
    fn media_url_kind_round_trips_via_json() {
        for (kind, expected) in [
            (MediaUrlKind::Torrent, "torrent"),
            (MediaUrlKind::Magnet, "magnet"),
            (MediaUrlKind::Direct, "direct"),
            (MediaUrlKind::Other, "other"),
        ] {
            let json = serde_json::to_value(kind).unwrap();
            assert_eq!(json, serde_json::Value::String(expected.to_string()));
            let back: MediaUrlKind = serde_json::from_value(json).unwrap();
            assert_eq!(back, kind);
            assert_eq!(kind.as_str(), expected);
        }
    }
}
