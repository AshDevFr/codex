//! Per-user, per-series reader settings.
//!
//! Only settings that describe the *content* live here. Reading direction is a
//! fact about how a book was made: a manga reads right to left on every screen
//! its reader owns, so the correction has to follow them between devices.
//!
//! Settings that describe the *device* deliberately stay client-side, in the
//! reader's own storage. Fit mode, background, and page layout are about the
//! screen in front of you rather than the file: double-page is natural on a
//! desktop and unusable on a phone, so syncing it between the two would be
//! wrong rather than convenient. The two double-page flags follow page layout
//! for the same reason, since they only take effect in that mode.
//!
//! The record is **sparse**, and stored as JSON rather than as columns, so a
//! later content-shaped setting can join it without a schema migration. An
//! absent key means the setting is inherited, not that it is unset: for reading
//! direction the layers beneath are the series metadata and then the library
//! default.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::reading_direction::ReadingDirection;

/// A user's content-setting overrides for one series.
///
/// Every field is optional and `None` means "inherit". Unknown keys are
/// ignored rather than rejected, so a row written by a newer version does not
/// break a rollback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct SeriesReaderSettings {
    /// Reading direction for this series, for this user only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reading_direction: Option<ReadingDirection>,
}

impl SeriesReaderSettings {
    /// Whether every setting is inherited.
    ///
    /// An empty record carries no information, so the repository deletes the
    /// row rather than storing `{}`: a present row means "this user has
    /// overrides here", and an empty one would lie to every later query.
    pub fn is_empty(&self) -> bool {
        self.reading_direction.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_fully_inherited() {
        let settings = SeriesReaderSettings::default();
        assert!(settings.is_empty());
        assert_eq!(serde_json::to_string(&settings).unwrap(), "{}");
    }

    #[test]
    fn only_set_keys_are_stored() {
        let settings = SeriesReaderSettings {
            reading_direction: Some(ReadingDirection::Rtl),
        };

        assert!(!settings.is_empty());
        assert_eq!(
            serde_json::to_string(&settings).unwrap(),
            r#"{"readingDirection":"rtl"}"#
        );
    }

    #[test]
    fn round_trips_through_json() {
        let settings = SeriesReaderSettings {
            reading_direction: Some(ReadingDirection::Webtoon),
        };

        let json = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            serde_json::from_str::<SeriesReaderSettings>(&json).unwrap(),
            settings
        );
    }

    #[test]
    fn keys_are_camel_case_to_match_the_api() {
        let settings = SeriesReaderSettings {
            reading_direction: Some(ReadingDirection::Ttb),
        };

        let json = serde_json::to_value(settings).unwrap();
        assert_eq!(json["readingDirection"], "ttb");
    }

    #[test]
    fn unknown_keys_are_ignored_rather_than_rejected() {
        // Two cases at once: a row written by a newer version that added a
        // content setting, and a row written before the device settings were
        // moved back out of this record.
        let parsed: SeriesReaderSettings = serde_json::from_str(
            r#"{"readingDirection":"rtl","fitMode":"width","futureSetting":"whatever"}"#,
        )
        .unwrap();
        assert_eq!(parsed.reading_direction, Some(ReadingDirection::Rtl));
    }

    #[test]
    fn invalid_values_are_rejected() {
        assert!(
            serde_json::from_str::<SeriesReaderSettings>(r#"{"readingDirection":"sideways"}"#)
                .is_err()
        );
    }
}
