//! Reading direction for series and libraries.
//!
//! Lives in `models` because the value is written through the api layer,
//! stored by db repositories, and produced by metadata providers in the
//! services layer. All three need the same notion of what a valid direction is.
//!
//! The database columns remain `String`: existing rows predate this type and
//! may hold values it cannot parse. Writes are validated at the API boundary,
//! and reads go through [`ReadingDirection::parse_stored`], which treats an
//! unparseable stored value as absent so resolution falls through to the next
//! layer instead of surfacing junk to a reader.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Direction a book's pages are read in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ReadingDirection {
    /// Left to right, the western default
    #[default]
    Ltr,
    /// Right to left, as manga is read
    Rtl,
    /// Top to bottom, paged
    Ttb,
    /// Top to bottom, continuously scrolled with no page boundaries
    Webtoon,
}

impl ReadingDirection {
    /// Canonical stored representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            ReadingDirection::Ltr => "ltr",
            ReadingDirection::Rtl => "rtl",
            ReadingDirection::Ttb => "ttb",
            ReadingDirection::Webtoon => "webtoon",
        }
    }

    /// Every valid direction, for error messages and API documentation.
    pub fn all() -> &'static [ReadingDirection] {
        &[
            ReadingDirection::Ltr,
            ReadingDirection::Rtl,
            ReadingDirection::Ttb,
            ReadingDirection::Webtoon,
        ]
    }

    /// Comma-separated list of valid values, for error messages.
    pub fn valid_values() -> String {
        Self::all()
            .iter()
            .map(|d| d.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Parse a value read back from storage.
    ///
    /// Returns `None` for both a missing value and an unparseable one. Rows
    /// written before this type existed were never validated, so a stored
    /// value that no longer parses is treated as no value at all: resolution
    /// falls through to the next layer rather than handing the reader
    /// something it cannot render.
    pub fn parse_stored(value: Option<&str>) -> Option<ReadingDirection> {
        value.and_then(|v| v.parse().ok())
    }

    /// Resolve a direction from layered sources, most specific first.
    ///
    /// Callers pass the layers in precedence order (per-user, then series, then
    /// library default). The first layer holding a parseable value wins. A layer
    /// that is absent *or* holds an unparseable legacy value is skipped, so one
    /// bad row degrades to the next layer rather than to a reader that cannot
    /// render the book.
    pub fn resolve(layers: &[Option<&str>]) -> Option<ReadingDirection> {
        layers.iter().find_map(|layer| Self::parse_stored(*layer))
    }
}

impl fmt::Display for ReadingDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ReadingDirection {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "ltr" => Ok(ReadingDirection::Ltr),
            "rtl" => Ok(ReadingDirection::Rtl),
            "ttb" => Ok(ReadingDirection::Ttb),
            "webtoon" => Ok(ReadingDirection::Webtoon),
            _ => Err(format!(
                "Invalid reading direction: {}. Valid values: {}",
                s,
                Self::valid_values()
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_valid_value() {
        for direction in ReadingDirection::all() {
            assert_eq!(
                ReadingDirection::from_str(direction.as_str()).unwrap(),
                *direction
            );
        }
    }

    #[test]
    fn parsing_is_case_and_whitespace_insensitive() {
        assert_eq!(
            ReadingDirection::from_str("  RTL ").unwrap(),
            ReadingDirection::Rtl
        );
        assert_eq!(
            ReadingDirection::from_str("Webtoon").unwrap(),
            ReadingDirection::Webtoon
        );
    }

    #[test]
    fn rejects_unknown_values() {
        let err = ReadingDirection::from_str("sideways").unwrap_err();
        assert!(err.contains("sideways"));
        assert!(err.contains("ltr, rtl, ttb, webtoon"));

        assert!(ReadingDirection::from_str("").is_err());
        // Komga's wire format is not our storage format
        assert!(ReadingDirection::from_str("LEFT_TO_RIGHT").is_err());
    }

    #[test]
    fn round_trips_through_display() {
        for direction in ReadingDirection::all() {
            assert_eq!(
                ReadingDirection::from_str(&direction.to_string()).unwrap(),
                *direction
            );
        }
    }

    #[test]
    fn serializes_to_the_stored_representation() {
        let json = serde_json::to_string(&ReadingDirection::Rtl).unwrap();
        assert_eq!(json, "\"rtl\"");

        let parsed: ReadingDirection = serde_json::from_str("\"webtoon\"").unwrap();
        assert_eq!(parsed, ReadingDirection::Webtoon);

        assert!(serde_json::from_str::<ReadingDirection>("\"sideways\"").is_err());
    }

    #[test]
    fn parse_stored_treats_junk_as_absent() {
        assert_eq!(
            ReadingDirection::parse_stored(Some("rtl")),
            Some(ReadingDirection::Rtl)
        );
        // A legacy row holding an unvalidated value must fall through to the
        // next resolution layer, not break the reader.
        assert_eq!(ReadingDirection::parse_stored(Some("sideways")), None);
        assert_eq!(ReadingDirection::parse_stored(Some("")), None);
        assert_eq!(ReadingDirection::parse_stored(None), None);
    }

    #[test]
    fn resolve_takes_the_first_parseable_layer() {
        // Most specific layer wins
        assert_eq!(
            ReadingDirection::resolve(&[Some("rtl"), Some("ltr"), Some("ttb")]),
            Some(ReadingDirection::Rtl)
        );
        // Absent layers are skipped
        assert_eq!(
            ReadingDirection::resolve(&[None, None, Some("webtoon")]),
            Some(ReadingDirection::Webtoon)
        );
        // A junk layer is skipped rather than winning
        assert_eq!(
            ReadingDirection::resolve(&[Some("sideways"), Some("rtl")]),
            Some(ReadingDirection::Rtl)
        );
        // Nothing usable anywhere
        assert_eq!(
            ReadingDirection::resolve(&[None, Some("sideways"), None]),
            None
        );
        assert_eq!(ReadingDirection::resolve(&[]), None);
    }

    #[test]
    fn defaults_to_left_to_right() {
        assert_eq!(ReadingDirection::default(), ReadingDirection::Ltr);
    }
}
