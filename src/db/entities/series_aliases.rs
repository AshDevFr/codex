//! `SeaORM` entity for the `series_aliases` table.
//!
//! Title aliases used by release-source plugins that match by title (e.g.
//! Nyaa). Distinct from `series_alternate_titles`, which is purpose-built for
//! labelled localized titles (Japanese / Romaji / English / Korean) with a
//! unique-per-label constraint - aliases here are arbitrary strings, normalized
//! for matching.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "series_aliases")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub series_id: Uuid,
    /// The alias as displayed (preserves casing/punctuation for UI).
    pub alias: String,
    /// Lowercased + punctuation-stripped, used for matcher equality.
    pub normalized: String,
    /// 'metadata' | 'manual'.
    pub source: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::series::Entity",
        from = "Column::SeriesId",
        to = "super::series::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Series,
}

impl Related<super::series::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Series.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Canonical strings for `source`.
pub mod alias_source {
    pub const METADATA: &str = "metadata";
    pub const MANUAL: &str = "manual";

    pub fn is_valid(s: &str) -> bool {
        matches!(s, METADATA | MANUAL)
    }
}

/// Normalize an alias for matching: lowercase, strip non-alphanumeric, collapse whitespace.
///
/// The normalization is intentionally aggressive: a release titled
/// `"My Series, Vol. 1 (Digital)"` and an alias stored as `"My Series"` should
/// share a common `normalized` prefix so a parser can match against the
/// normalized form. The raw `alias` field preserves the user's input for UI.
pub fn normalize_alias(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_space = false;
    for ch in input.chars() {
        if ch.is_alphanumeric() {
            for lc in ch.to_lowercase() {
                out.push(lc);
            }
            last_was_space = false;
        } else if ch.is_whitespace() && !out.is_empty() && !last_was_space {
            out.push(' ');
            last_was_space = true;
        }
        // Any other punctuation/symbols get dropped.
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_lowercases_and_strips_punctuation() {
        assert_eq!(normalize_alias("My Hero Academia"), "my hero academia");
        assert_eq!(normalize_alias("My Hero Academia!"), "my hero academia");
        assert_eq!(
            normalize_alias("Re:Zero - Starting Life in Another World"),
            "rezero starting life in another world"
        );
    }

    #[test]
    fn normalize_collapses_whitespace() {
        assert_eq!(normalize_alias("  Lots   of    spaces  "), "lots of spaces");
        assert_eq!(normalize_alias("Tab\tand\nnewline"), "tab and newline");
    }

    #[test]
    fn normalize_strips_digital_suffix_marker() {
        // Tag suffixes commonly seen in Nyaa titles.
        assert_eq!(
            normalize_alias("My Series v01 (Digital)"),
            "my series v01 digital"
        );
    }

    #[test]
    fn normalize_handles_unicode_lowercase() {
        // Unicode lowercase round-trip (Greek, German).
        assert_eq!(normalize_alias("ÄÖÜ"), "äöü");
    }

    #[test]
    fn normalize_empty_input() {
        assert_eq!(normalize_alias(""), "");
        assert_eq!(normalize_alias("   "), "");
        assert_eq!(normalize_alias("!!!---!!!"), "");
    }

    #[test]
    fn alias_source_validates_known_values() {
        assert!(alias_source::is_valid("metadata"));
        assert!(alias_source::is_valid("manual"));
        assert!(!alias_source::is_valid("auto"));
        assert!(!alias_source::is_valid(""));
    }
}
