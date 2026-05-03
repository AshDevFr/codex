//! User-facing field-group taxonomy for the scheduled metadata refresh.
//!
//! The scheduled refresh exposes a small set of named groups (Ratings,
//! Status, Counts, etc.) instead of asking the user to pick from the ~20
//! camelCase field names that [`crate::services::metadata::MetadataApplier`]
//! understands. This module is the authoritative mapping between the two
//! vocabularies.
//!
//! ## Vocabulary
//!
//! - **Field name**: a camelCase string the applier recognises in
//!   `ApplyOptions::fields_filter` (e.g. `"rating"`, `"totalVolumeCount"`).
//!   Source of truth: `should_apply_field(...)` call sites in
//!   `services/metadata/apply.rs`.
//! - **Group name**: a snake_case string stored in
//!   `MetadataRefreshConfig::field_groups` (e.g. `"ratings"`, `"counts"`).
//!   This is what the UI persists.
//!
//! ## Responsibilities
//!
//! - [`FieldGroup`] enum — closed set of groups, parseable from snake_case.
//! - [`fields_for_group`] — single group → its field set.
//! - [`fields_for_groups`] — slice of groups → deduplicated field set
//!   (returns an `Option<HashSet<String>>` mirroring `ApplyOptions`).
//! - [`group_for_field`] — reverse lookup, used by the UI for display.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::str::FromStr;

/// Closed taxonomy of refresh field groups.
///
/// Stored values are snake_case strings — see [`FieldGroup::as_str`] /
/// [`FieldGroup::from_str`]. The mapping to concrete field names is
/// intentionally narrow and conservative: each group covers fields that
/// "move together" semantically, so refreshing a group rarely surprises
/// the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldGroup {
    /// Title, title sort, alternate titles.
    Identifiers,
    /// Summary and structured author info.
    Descriptive,
    /// Publication status and publication year.
    Status,
    /// Total volume count and total chapter count.
    Counts,
    /// Primary rating and full external-ratings table.
    Ratings,
    /// Series cover image URL.
    Cover,
    /// Free-form tags.
    Tags,
    /// Genres.
    Genres,
    /// Age rating.
    AgeRating,
    /// Language and reading direction.
    Classification,
    /// Publisher and imprint.
    Publisher,
    /// Cross-references to other services and editorial links.
    ExternalRefs,
}

impl FieldGroup {
    /// Snake_case identifier used in storage and over the wire.
    pub fn as_str(&self) -> &'static str {
        match self {
            FieldGroup::Identifiers => "identifiers",
            FieldGroup::Descriptive => "descriptive",
            FieldGroup::Status => "status",
            FieldGroup::Counts => "counts",
            FieldGroup::Ratings => "ratings",
            FieldGroup::Cover => "cover",
            FieldGroup::Tags => "tags",
            FieldGroup::Genres => "genres",
            FieldGroup::AgeRating => "age_rating",
            FieldGroup::Classification => "classification",
            FieldGroup::Publisher => "publisher",
            FieldGroup::ExternalRefs => "external_refs",
        }
    }

    /// All groups, in display order. Used by the public field-group endpoint.
    pub fn all() -> &'static [FieldGroup] {
        &[
            FieldGroup::Identifiers,
            FieldGroup::Descriptive,
            FieldGroup::Status,
            FieldGroup::Counts,
            FieldGroup::Ratings,
            FieldGroup::Cover,
            FieldGroup::Tags,
            FieldGroup::Genres,
            FieldGroup::AgeRating,
            FieldGroup::Classification,
            FieldGroup::Publisher,
            FieldGroup::ExternalRefs,
        ]
    }
}

impl FromStr for FieldGroup {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "identifiers" => Ok(FieldGroup::Identifiers),
            "descriptive" => Ok(FieldGroup::Descriptive),
            "status" => Ok(FieldGroup::Status),
            "counts" => Ok(FieldGroup::Counts),
            "ratings" => Ok(FieldGroup::Ratings),
            "cover" => Ok(FieldGroup::Cover),
            "tags" => Ok(FieldGroup::Tags),
            "genres" => Ok(FieldGroup::Genres),
            "age_rating" => Ok(FieldGroup::AgeRating),
            "classification" => Ok(FieldGroup::Classification),
            "publisher" => Ok(FieldGroup::Publisher),
            "external_refs" => Ok(FieldGroup::ExternalRefs),
            other => Err(format!("Unknown field group '{}'", other)),
        }
    }
}

/// Concrete field names (camelCase) covered by a single group.
///
/// Field names match the strings the [`MetadataApplier`] checks via
/// `should_apply_field`. Adding a field here without a matching applier
/// branch silently does nothing — there's a unit test that asserts every
/// returned field is one the applier actually knows about.
///
/// [`MetadataApplier`]: crate::services::metadata::MetadataApplier
pub fn fields_for_group(group: FieldGroup) -> &'static [&'static str] {
    match group {
        FieldGroup::Identifiers => &["title", "titleSort", "alternateTitles"],
        FieldGroup::Descriptive => &["summary", "authors"],
        FieldGroup::Status => &["status", "year"],
        FieldGroup::Counts => &["totalVolumeCount", "totalChapterCount"],
        FieldGroup::Ratings => &["rating", "externalRatings"],
        FieldGroup::Cover => &["coverUrl"],
        FieldGroup::Tags => &["tags"],
        FieldGroup::Genres => &["genres"],
        FieldGroup::AgeRating => &["ageRating"],
        FieldGroup::Classification => &["language", "readingDirection"],
        FieldGroup::Publisher => &["publisher"],
        FieldGroup::ExternalRefs => &["externalIds", "externalLinks"],
    }
}

/// Expand a slice of group names into a deduplicated set of field names.
///
/// Unknown group strings are silently ignored — callers that want strict
/// validation should call [`FieldGroup::from_str`] up front (the PATCH
/// endpoint will, in Phase 6).
///
/// Returns `None` when both `groups` and `extras` are empty, matching the
/// "no filter, apply everything" semantics of
/// [`crate::services::metadata::ApplyOptions::fields_filter`].
pub fn fields_for_groups<S: AsRef<str>>(groups: &[S], extras: &[S]) -> Option<HashSet<String>> {
    if groups.is_empty() && extras.is_empty() {
        return None;
    }
    let mut out: HashSet<String> = HashSet::new();
    for g in groups {
        if let Ok(group) = FieldGroup::from_str(g.as_ref()) {
            for field in fields_for_group(group) {
                out.insert((*field).to_string());
            }
        }
    }
    for f in extras {
        out.insert(f.as_ref().to_string());
    }
    Some(out)
}

/// Reverse lookup: given a field name, return the group it belongs to.
///
/// Used by the UI to render "this field belongs to group X" in the
/// dry-run preview. Returns `None` for fields that aren't part of any
/// group (e.g. `customMetadata`, which the scheduled refresh doesn't
/// touch).
#[allow(dead_code)]
pub fn group_for_field(field: &str) -> Option<FieldGroup> {
    for group in FieldGroup::all() {
        if fields_for_group(*group).contains(&field) {
            return Some(*group);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every field name returned by the resolver must be one the applier
    /// actually checks. Otherwise we're silently no-op'ing the user's
    /// selection.
    ///
    /// Source of truth: `should_apply_field("...")` call sites in
    /// `services/metadata/apply.rs`.
    const APPLIER_KNOWN_FIELDS: &[&str] = &[
        "title",
        "titleSort",
        "alternateTitles",
        "summary",
        "year",
        "status",
        "publisher",
        "ageRating",
        "language",
        "readingDirection",
        "totalVolumeCount",
        "totalChapterCount",
        "genres",
        "tags",
        "authors",
        "externalLinks",
        "externalIds",
        "rating",
        "externalRatings",
        "coverUrl",
    ];

    #[test]
    fn every_field_in_every_group_is_known_to_the_applier() {
        for group in FieldGroup::all() {
            for field in fields_for_group(*group) {
                assert!(
                    APPLIER_KNOWN_FIELDS.contains(field),
                    "field '{}' (group {:?}) is not recognized by MetadataApplier",
                    field,
                    group
                );
            }
        }
    }

    #[test]
    fn from_str_round_trips() {
        for group in FieldGroup::all() {
            let s = group.as_str();
            let parsed = FieldGroup::from_str(s).expect("should parse");
            assert_eq!(parsed, *group);
        }
    }

    #[test]
    fn from_str_rejects_unknown() {
        assert!(FieldGroup::from_str("not_a_group").is_err());
        assert!(FieldGroup::from_str("").is_err());
    }

    #[test]
    fn ratings_group_maps_to_rating_fields() {
        let fields = fields_for_group(FieldGroup::Ratings);
        assert_eq!(fields, &["rating", "externalRatings"]);
    }

    #[test]
    fn counts_group_includes_both_counts() {
        let fields = fields_for_group(FieldGroup::Counts);
        assert!(fields.contains(&"totalVolumeCount"));
        assert!(fields.contains(&"totalChapterCount"));
    }

    #[test]
    fn fields_for_groups_returns_none_when_empty() {
        let groups: Vec<&str> = vec![];
        let extras: Vec<&str> = vec![];
        assert!(fields_for_groups(&groups, &extras).is_none());
    }

    #[test]
    fn fields_for_groups_expands_groups() {
        let groups = ["ratings", "status"];
        let extras: Vec<&str> = vec![];
        let out = fields_for_groups(&groups, &extras).unwrap();
        assert!(out.contains("rating"));
        assert!(out.contains("externalRatings"));
        assert!(out.contains("status"));
        assert!(out.contains("year"));
        assert_eq!(out.len(), 4);
    }

    #[test]
    fn fields_for_groups_dedupes_overlapping_groups() {
        // Identifiers and Descriptive don't overlap, but extras can.
        let groups = ["identifiers"];
        let extras = ["title", "summary"];
        let out = fields_for_groups(&groups, &extras).unwrap();
        // identifiers brings title + titleSort + alternateTitles; extras add summary.
        // 'title' is duplicated by extras; should appear once.
        assert!(out.contains("title"));
        assert!(out.contains("titleSort"));
        assert!(out.contains("alternateTitles"));
        assert!(out.contains("summary"));
        assert_eq!(out.len(), 4);
    }

    #[test]
    fn fields_for_groups_silently_ignores_unknown_groups() {
        let groups = ["ratings", "made_up_group"];
        let extras: Vec<&str> = vec![];
        let out = fields_for_groups(&groups, &extras).unwrap();
        assert_eq!(out.len(), 2);
        assert!(out.contains("rating"));
        assert!(out.contains("externalRatings"));
    }

    #[test]
    fn fields_for_groups_returns_only_extras_when_groups_empty() {
        let groups: Vec<&str> = vec![];
        let extras = ["language", "publisher"];
        let out = fields_for_groups(&groups, &extras).unwrap();
        assert_eq!(out.len(), 2);
        assert!(out.contains("language"));
        assert!(out.contains("publisher"));
    }

    #[test]
    fn group_for_field_finds_group() {
        assert_eq!(group_for_field("rating"), Some(FieldGroup::Ratings));
        assert_eq!(
            group_for_field("externalRatings"),
            Some(FieldGroup::Ratings)
        );
        assert_eq!(
            group_for_field("totalVolumeCount"),
            Some(FieldGroup::Counts)
        );
        assert_eq!(group_for_field("status"), Some(FieldGroup::Status));
        assert_eq!(group_for_field("year"), Some(FieldGroup::Status));
        assert_eq!(group_for_field("coverUrl"), Some(FieldGroup::Cover));
    }

    #[test]
    fn group_for_field_returns_none_for_unknown_field() {
        assert_eq!(group_for_field("notARealField"), None);
        assert_eq!(group_for_field(""), None);
    }

    #[test]
    fn all_groups_are_listed_in_all() {
        // FieldGroup::all() should match the variants in the enum. If you
        // add a variant, add it to all() too.
        let listed: HashSet<FieldGroup> = FieldGroup::all().iter().copied().collect();
        assert!(listed.contains(&FieldGroup::Identifiers));
        assert!(listed.contains(&FieldGroup::Descriptive));
        assert!(listed.contains(&FieldGroup::Status));
        assert!(listed.contains(&FieldGroup::Counts));
        assert!(listed.contains(&FieldGroup::Ratings));
        assert!(listed.contains(&FieldGroup::Cover));
        assert!(listed.contains(&FieldGroup::Tags));
        assert!(listed.contains(&FieldGroup::Genres));
        assert!(listed.contains(&FieldGroup::AgeRating));
        assert!(listed.contains(&FieldGroup::Classification));
        assert!(listed.contains(&FieldGroup::Publisher));
        assert!(listed.contains(&FieldGroup::ExternalRefs));
        assert_eq!(listed.len(), 12);
    }
}
