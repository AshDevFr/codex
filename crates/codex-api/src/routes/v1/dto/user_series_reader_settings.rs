//! DTOs for a user's per-series reader settings.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use codex_models::reader_settings::SeriesReaderSettings;
use codex_models::reading_direction::ReadingDirection;

use super::patch::PatchValue;

/// A user's content-setting overrides for one series.
///
/// Sparse: a field is absent when the user has not overridden it, and the
/// reader inherits instead. For reading direction that means the series
/// metadata and then the library default.
///
/// Settings that describe the device rather than the content, such as fit mode
/// and page layout, are deliberately not here. They belong to the screen in
/// front of the reader, not to the file, and each client keeps its own.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesReaderSettingsResponse {
    /// Reading direction for this series, for this user only
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "rtl")]
    pub reading_direction: Option<ReadingDirection>,
}

impl From<SeriesReaderSettings> for SeriesReaderSettingsResponse {
    fn from(settings: SeriesReaderSettings) -> Self {
        Self {
            reading_direction: settings.reading_direction,
        }
    }
}

/// Partial update to a user's content-setting overrides for one series.
///
/// Each field has three states, the usual PATCH semantics:
///
/// - absent: leave the setting as it is
/// - `null`: clear the override, so the setting inherits again
/// - a value: override the setting with it
///
/// Clearing the last override removes the stored record entirely, which is the
/// same end state as `DELETE` on the same path.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatchSeriesReaderSettingsRequest {
    #[serde(default)]
    #[schema(value_type = Option<ReadingDirection>, nullable = true)]
    pub reading_direction: PatchValue<ReadingDirection>,
}

impl PatchSeriesReaderSettingsRequest {
    /// Apply this patch to the user's existing overrides.
    ///
    /// The merge lives here rather than in the repository because
    /// [`PatchValue`] is what distinguishes absent from null, and it depends on
    /// `sea-orm` and `utoipa` that `codex-models` deliberately does not carry.
    /// `patch_series_metadata` merges at this layer for the same reason.
    pub fn apply_to(self, mut current: SeriesReaderSettings) -> SeriesReaderSettings {
        // Absent leaves the field alone; null clears it; a value sets it.
        if let Some(value) = self.reading_direction.into_nested_option() {
            current.reading_direction = value;
        }

        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patch(json: &str) -> PatchSeriesReaderSettingsRequest {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn an_absent_field_is_left_alone() {
        let current = SeriesReaderSettings {
            reading_direction: Some(ReadingDirection::Rtl),
        };

        assert_eq!(patch("{}").apply_to(current), current);
    }

    #[test]
    fn a_value_overrides_the_setting() {
        let merged =
            patch(r#"{"readingDirection":"webtoon"}"#).apply_to(SeriesReaderSettings::default());

        assert_eq!(merged.reading_direction, Some(ReadingDirection::Webtoon));
    }

    #[test]
    fn an_explicit_null_clears_the_override() {
        let current = SeriesReaderSettings {
            reading_direction: Some(ReadingDirection::Rtl),
        };

        let merged = patch(r#"{"readingDirection":null}"#).apply_to(current);

        assert_eq!(merged.reading_direction, None);
        // The repository turns an empty record into no row at all.
        assert!(merged.is_empty());
    }

    #[test]
    fn an_invalid_value_is_rejected_before_it_reaches_the_merge() {
        assert!(
            serde_json::from_str::<PatchSeriesReaderSettingsRequest>(
                r#"{"readingDirection":"sideways"}"#
            )
            .is_err()
        );
    }
}
