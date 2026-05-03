//! Stable skip-reason taxonomy for the scheduled metadata refresh.
//!
//! Used by the dry-run preview, the task summary JSON, and the planner. The
//! string forms (returned by [`RefreshSkipReason::as_str`]) are part of the
//! public API surface — they show up in HTTP responses and stored task
//! results. Don't rename them without a migration story.
//!
//! The planner currently owns its own narrower [`super::refresh_planner::SkipReason`]
//! enum that covers reasons knowable at planning time (no external ID, recently
//! synced, provider unavailable). This module is a *superset* that also names
//! reasons only knowable mid-run (all fields locked, all fields filtered out,
//! plugin call failed). Both enums use the same string identifiers so the
//! union counts in the task summary stay coherent.
//!
//! [`From<super::refresh_planner::SkipReason>`] is provided for the planner
//! reasons. Run-time reasons are constructed directly by the handler.

use serde::{Deserialize, Serialize};

use super::refresh_planner::SkipReason as PlannerSkipReason;

/// One stable reason a `(series, provider)` pair was skipped.
///
/// Variants are flat and self-describing so the JSON form is friendly to
/// consume from the frontend. The discriminant strings (also returned by
/// [`Self::as_str`]) are stable.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RefreshSkipReason {
    /// `existing_source_ids_only = true` and the series has no stored
    /// external ID for the provider. Surfaced by the planner.
    NoExternalId,
    /// `last_synced_at` for the (series, provider) pair is younger than
    /// `skip_recently_synced_within_s`. Surfaced by the planner.
    RecentlySynced,
    /// Provider configuration references a plugin that isn't installed or
    /// is disabled. Recorded once per plan rather than per series, but the
    /// taxonomy lists it for completeness.
    ProviderUnavailable,
    /// Re-match attempt found no candidate above the confidence threshold.
    /// Only reachable in `AllowReMatch` mode.
    NoMatchCandidate,
    /// The applier reported every field as locked (none written). The pair
    /// was technically processed but nothing changed; surfacing it as a
    /// distinct reason avoids inflating "succeeded" counts when in fact
    /// the user's locks blocked the run.
    AllFieldsLocked,
    /// `fields_filter` excluded every field the provider returned. Distinct
    /// from `AllFieldsLocked` because the cause is config, not user locks.
    NoFieldsAfterFilter,
    /// Plugin returned an error or timed out. The handler still increments
    /// the `failed` counter; this variant exists so the dry-run preview can
    /// label the row instead of dropping it silently.
    PluginCallFailed,
}

impl RefreshSkipReason {
    /// Stable string identifier. Used as the JSON key in task-summary
    /// `skipped` maps and as the value of the `kind` discriminant.
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoExternalId => "no_external_id",
            Self::RecentlySynced => "recently_synced",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::NoMatchCandidate => "no_match_candidate",
            Self::AllFieldsLocked => "all_fields_locked",
            Self::NoFieldsAfterFilter => "no_fields_after_filter",
            Self::PluginCallFailed => "plugin_call_failed",
        }
    }
}

impl From<PlannerSkipReason> for RefreshSkipReason {
    fn from(value: PlannerSkipReason) -> Self {
        match value {
            PlannerSkipReason::NoExternalId => Self::NoExternalId,
            PlannerSkipReason::RecentlySynced { .. } => Self::RecentlySynced,
            PlannerSkipReason::ProviderUnavailable { .. } => Self::ProviderUnavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_covers_every_variant() {
        // Every variant returns a non-empty stable identifier.
        for r in [
            RefreshSkipReason::NoExternalId,
            RefreshSkipReason::RecentlySynced,
            RefreshSkipReason::ProviderUnavailable,
            RefreshSkipReason::NoMatchCandidate,
            RefreshSkipReason::AllFieldsLocked,
            RefreshSkipReason::NoFieldsAfterFilter,
            RefreshSkipReason::PluginCallFailed,
        ] {
            assert!(!r.as_str().is_empty());
        }
    }

    #[test]
    fn no_external_id_has_stable_identifier() {
        assert_eq!(RefreshSkipReason::NoExternalId.as_str(), "no_external_id");
    }

    #[test]
    fn recently_synced_has_stable_identifier() {
        assert_eq!(
            RefreshSkipReason::RecentlySynced.as_str(),
            "recently_synced"
        );
    }

    #[test]
    fn provider_unavailable_has_stable_identifier() {
        assert_eq!(
            RefreshSkipReason::ProviderUnavailable.as_str(),
            "provider_unavailable"
        );
    }

    #[test]
    fn no_match_candidate_has_stable_identifier() {
        assert_eq!(
            RefreshSkipReason::NoMatchCandidate.as_str(),
            "no_match_candidate"
        );
    }

    #[test]
    fn all_fields_locked_has_stable_identifier() {
        assert_eq!(
            RefreshSkipReason::AllFieldsLocked.as_str(),
            "all_fields_locked"
        );
    }

    #[test]
    fn no_fields_after_filter_has_stable_identifier() {
        assert_eq!(
            RefreshSkipReason::NoFieldsAfterFilter.as_str(),
            "no_fields_after_filter"
        );
    }

    #[test]
    fn plugin_call_failed_has_stable_identifier() {
        assert_eq!(
            RefreshSkipReason::PluginCallFailed.as_str(),
            "plugin_call_failed"
        );
    }

    #[test]
    fn from_planner_no_external_id() {
        assert_eq!(
            RefreshSkipReason::from(PlannerSkipReason::NoExternalId),
            RefreshSkipReason::NoExternalId
        );
    }

    #[test]
    fn from_planner_recently_synced_drops_timestamp() {
        let r = RefreshSkipReason::from(PlannerSkipReason::RecentlySynced {
            last_synced_at: chrono::Utc::now(),
        });
        assert_eq!(r, RefreshSkipReason::RecentlySynced);
    }

    #[test]
    fn from_planner_provider_unavailable_drops_provider() {
        let r = RefreshSkipReason::from(PlannerSkipReason::ProviderUnavailable {
            provider: "plugin:gone".to_string(),
        });
        assert_eq!(r, RefreshSkipReason::ProviderUnavailable);
    }

    #[test]
    fn serde_round_trip_uses_kind_discriminant() {
        let r = RefreshSkipReason::NoExternalId;
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json, serde_json::json!({"kind": "no_external_id"}));
        let back: RefreshSkipReason = serde_json::from_value(json).unwrap();
        assert_eq!(back, r);
    }
}
