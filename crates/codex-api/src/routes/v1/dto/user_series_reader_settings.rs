//! DTOs for a user's per-series reader settings.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use codex_models::reader_settings::SeriesReaderSettings;
use codex_models::reading_direction::ReadingDirection;

use super::patch::PatchValue;

/// Which layer supplied a value the user is inheriting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum InheritedFrom {
    /// The series metadata, which an editor set for everyone
    Series,
    /// The library default, which applies to every series in it
    Library,
}

/// A user's content-setting overrides for one series, and what lies beneath them.
///
/// Sparse: an override field is absent when the user has not set it, and the
/// reader inherits instead. For reading direction that means the series
/// metadata and then the library default.
///
/// The `inherited*` fields report that lower value whether or not an override
/// is present. They exist because a book response carries the direction already
/// resolved, so a client holding an override cannot otherwise see what dropping
/// it would fall back to, and cannot word the offer to drop it.
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

    /// Direction this user would inherit with no override of their own.
    ///
    /// Read-only, and absent when no layer holds a usable value.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "ltr")]
    pub inherited_reading_direction: Option<ReadingDirection>,

    /// Which layer [`Self::inherited_reading_direction`] came from.
    ///
    /// Always present and absent together with it, because the two are set from
    /// one resolved pair.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inherited_reading_direction_source: Option<InheritedFrom>,
}

impl SeriesReaderSettingsResponse {
    /// Build a response from this user's overrides and the layer beneath them.
    ///
    /// `inherited` pairs the value with its source rather than taking two
    /// arguments, so a caller cannot report a direction from one layer while
    /// naming another. This replaces a `From<SeriesReaderSettings>` impl, which
    /// could not see the series or library state the inherited value needs.
    pub fn new(
        overrides: SeriesReaderSettings,
        inherited: Option<(ReadingDirection, InheritedFrom)>,
    ) -> Self {
        Self {
            reading_direction: overrides.reading_direction,
            inherited_reading_direction: inherited.map(|(direction, _)| direction),
            inherited_reading_direction_source: inherited.map(|(_, source)| source),
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
    /// Reading direction: `ltr`, `rtl`, `ttb` or `webtoon`. Null clears the
    /// override so the series metadata or library default applies again.
    ///
    /// Typed as a string rather than as the enum because a `$ref` cannot also
    /// be nullable, and the null is the whole point of the field: a schema that
    /// could not express it would not describe how to stop overriding. The
    /// value is validated in the handler, as it is on the series metadata
    /// endpoints.
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "rtl", nullable = true)]
    pub reading_direction: PatchValue<String>,
}

impl PatchSeriesReaderSettingsRequest {
    /// Apply this patch to the user's existing overrides.
    ///
    /// The merge lives here rather than in the repository because
    /// [`PatchValue`] is what distinguishes absent from null, and it depends on
    /// `sea-orm` and `utoipa` that `codex-models` deliberately does not carry.
    /// `patch_series_metadata` merges at this layer for the same reason.
    /// `direction` is the already-validated value, so this cannot fail.
    pub fn apply_to(
        self,
        mut current: SeriesReaderSettings,
        direction: Option<ReadingDirection>,
    ) -> SeriesReaderSettings {
        // Absent leaves the field alone; null clears it; a value sets it.
        if self.reading_direction.into_nested_option().is_some() {
            current.reading_direction = direction;
        }

        current
    }

    /// Validate the incoming direction, returning what `apply_to` should write.
    ///
    /// `Ok(None)` covers both "not provided" and "explicitly null"; `apply_to`
    /// distinguishes them from the patch itself.
    pub fn validated_direction(&self) -> Result<Option<ReadingDirection>, String> {
        match self.reading_direction.clone().into_nested_option() {
            Some(Some(raw)) => raw.parse::<ReadingDirection>().map(Some),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patch(json: &str) -> PatchSeriesReaderSettingsRequest {
        serde_json::from_str(json).unwrap()
    }

    fn apply(json: &str, current: SeriesReaderSettings) -> SeriesReaderSettings {
        let request = patch(json);
        let direction = request.validated_direction().unwrap();
        request.apply_to(current, direction)
    }

    #[test]
    fn an_absent_field_is_left_alone() {
        let current = SeriesReaderSettings {
            reading_direction: Some(ReadingDirection::Rtl),
        };

        assert_eq!(apply("{}", current), current);
    }

    #[test]
    fn a_value_overrides_the_setting() {
        let merged = apply(
            r#"{"readingDirection":"webtoon"}"#,
            SeriesReaderSettings::default(),
        );

        assert_eq!(merged.reading_direction, Some(ReadingDirection::Webtoon));
    }

    #[test]
    fn a_value_is_canonicalised() {
        let merged = apply(
            r#"{"readingDirection":"RTL"}"#,
            SeriesReaderSettings::default(),
        );

        assert_eq!(merged.reading_direction, Some(ReadingDirection::Rtl));
    }

    #[test]
    fn an_explicit_null_clears_the_override() {
        let current = SeriesReaderSettings {
            reading_direction: Some(ReadingDirection::Rtl),
        };

        let merged = apply(r#"{"readingDirection":null}"#, current);

        assert_eq!(merged.reading_direction, None);
        // The repository turns an empty record into no row at all.
        assert!(merged.is_empty());
    }

    #[test]
    fn a_response_omits_every_absent_field() {
        let response = SeriesReaderSettingsResponse::new(SeriesReaderSettings::default(), None);

        assert_eq!(serde_json::to_string(&response).unwrap(), "{}");
    }

    #[test]
    fn a_response_reports_the_inherited_value_with_its_source() {
        let response = SeriesReaderSettingsResponse::new(
            SeriesReaderSettings::default(),
            Some((ReadingDirection::Rtl, InheritedFrom::Series)),
        );

        let json = serde_json::to_value(response).unwrap();
        assert_eq!(json["inheritedReadingDirection"], "rtl");
        assert_eq!(json["inheritedReadingDirectionSource"], "series");
        // No override of this user's own, so the field stays out.
        assert!(json.get("readingDirection").is_none());
    }

    #[test]
    fn an_override_does_not_hide_the_layer_beneath_it() {
        let response = SeriesReaderSettingsResponse::new(
            SeriesReaderSettings {
                reading_direction: Some(ReadingDirection::Ltr),
            },
            Some((ReadingDirection::Rtl, InheritedFrom::Library)),
        );

        let json = serde_json::to_value(response).unwrap();
        assert_eq!(json["readingDirection"], "ltr");
        assert_eq!(json["inheritedReadingDirection"], "rtl");
        assert_eq!(json["inheritedReadingDirectionSource"], "library");
    }

    #[test]
    fn an_invalid_value_is_rejected_before_it_reaches_the_merge() {
        let request = patch(r#"{"readingDirection":"sideways"}"#);
        let error = request.validated_direction().unwrap_err();
        assert!(error.contains("sideways"));
    }
}
