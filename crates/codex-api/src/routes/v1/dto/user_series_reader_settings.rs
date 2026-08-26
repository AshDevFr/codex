//! DTOs for a user's per-series reader settings.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use codex_models::reader_settings::{
    BackgroundColor, FitMode, PageLayout, SeriesReaderSettings, WebtoonFitMode,
};
use codex_models::reading_direction::ReadingDirection;

use super::patch::PatchValue;

/// A user's reader overrides for one series.
///
/// Sparse: a field is absent when the user has not overridden it, and the
/// reader inherits from the series metadata or the library default instead.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesReaderSettingsResponse {
    /// How a page is scaled to the viewport
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "width")]
    pub fit_mode: Option<FitMode>,

    /// Fit mode for the webtoon reader
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "width")]
    pub webtoon_fit_mode: Option<WebtoonFitMode>,

    /// How many pages are shown at once
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "single")]
    pub page_layout: Option<PageLayout>,

    /// Reading direction for this series, for this user only
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "rtl")]
    pub reading_direction: Option<ReadingDirection>,

    /// Backdrop behind the page
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "black")]
    pub background_color: Option<BackgroundColor>,

    /// Whether a wide page is shown alone in double-page layout
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = true)]
    pub double_page_show_wide_alone: Option<bool>,

    /// Whether double-page layout starts on an odd page
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub double_page_start_on_odd: Option<bool>,
}

impl From<SeriesReaderSettings> for SeriesReaderSettingsResponse {
    fn from(settings: SeriesReaderSettings) -> Self {
        Self {
            fit_mode: settings.fit_mode,
            webtoon_fit_mode: settings.webtoon_fit_mode,
            page_layout: settings.page_layout,
            reading_direction: settings.reading_direction,
            background_color: settings.background_color,
            double_page_show_wide_alone: settings.double_page_show_wide_alone,
            double_page_start_on_odd: settings.double_page_start_on_odd,
        }
    }
}

/// Partial update to a user's reader overrides for one series.
///
/// Each field has three states, the usual PATCH semantics:
///
/// - absent: leave the setting as it is
/// - `null`: clear the override, so the setting inherits again
/// - a value: override the setting with it
///
/// Per-key clearing matters because the record is sparse. Undoing one
/// override must not require wiping the rest and re-setting them; `DELETE`
/// on the same path is the all-at-once reset.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatchSeriesReaderSettingsRequest {
    #[serde(default)]
    #[schema(value_type = Option<FitMode>, nullable = true)]
    pub fit_mode: PatchValue<FitMode>,

    #[serde(default)]
    #[schema(value_type = Option<WebtoonFitMode>, nullable = true)]
    pub webtoon_fit_mode: PatchValue<WebtoonFitMode>,

    #[serde(default)]
    #[schema(value_type = Option<PageLayout>, nullable = true)]
    pub page_layout: PatchValue<PageLayout>,

    #[serde(default)]
    #[schema(value_type = Option<ReadingDirection>, nullable = true)]
    pub reading_direction: PatchValue<ReadingDirection>,

    #[serde(default)]
    #[schema(value_type = Option<BackgroundColor>, nullable = true)]
    pub background_color: PatchValue<BackgroundColor>,

    #[serde(default)]
    #[schema(value_type = Option<bool>, nullable = true)]
    pub double_page_show_wide_alone: PatchValue<bool>,

    #[serde(default)]
    #[schema(value_type = Option<bool>, nullable = true)]
    pub double_page_start_on_odd: PatchValue<bool>,
}

impl PatchSeriesReaderSettingsRequest {
    /// Apply this patch to the user's existing overrides.
    ///
    /// The merge lives here rather than in the repository because
    /// [`PatchValue`] is what distinguishes absent from null, and it depends on
    /// `sea-orm` and `utoipa` that `codex-models` deliberately does not carry.
    /// `patch_series_metadata` merges at this layer for the same reason.
    pub fn apply_to(self, mut current: SeriesReaderSettings) -> SeriesReaderSettings {
        /// Absent leaves the field alone; null clears it; a value sets it.
        fn merge<T>(field: &mut Option<T>, patch: PatchValue<T>) {
            if let Some(value) = patch.into_nested_option() {
                *field = value;
            }
        }

        merge(&mut current.fit_mode, self.fit_mode);
        merge(&mut current.webtoon_fit_mode, self.webtoon_fit_mode);
        merge(&mut current.page_layout, self.page_layout);
        merge(&mut current.reading_direction, self.reading_direction);
        merge(&mut current.background_color, self.background_color);
        merge(
            &mut current.double_page_show_wide_alone,
            self.double_page_show_wide_alone,
        );
        merge(
            &mut current.double_page_start_on_odd,
            self.double_page_start_on_odd,
        );

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
    fn absent_fields_are_left_alone() {
        let current = SeriesReaderSettings {
            reading_direction: Some(ReadingDirection::Rtl),
            background_color: Some(BackgroundColor::Black),
            ..Default::default()
        };

        let merged = patch(r#"{"fitMode":"width"}"#).apply_to(current);

        assert_eq!(merged.fit_mode, Some(FitMode::Width));
        assert_eq!(merged.reading_direction, Some(ReadingDirection::Rtl));
        assert_eq!(merged.background_color, Some(BackgroundColor::Black));
    }

    #[test]
    fn an_explicit_null_clears_one_setting() {
        let current = SeriesReaderSettings {
            reading_direction: Some(ReadingDirection::Rtl),
            background_color: Some(BackgroundColor::Black),
            ..Default::default()
        };

        // Clearing one override must leave the others alone; wiping the record
        // is what DELETE is for.
        let merged = patch(r#"{"backgroundColor":null}"#).apply_to(current);

        assert_eq!(merged.background_color, None);
        assert_eq!(merged.reading_direction, Some(ReadingDirection::Rtl));
    }

    #[test]
    fn clearing_the_last_override_empties_the_record() {
        let current = SeriesReaderSettings {
            reading_direction: Some(ReadingDirection::Rtl),
            ..Default::default()
        };

        let merged = patch(r#"{"readingDirection":null}"#).apply_to(current);

        // The repository turns an empty record into no row at all.
        assert!(merged.is_empty());
    }

    #[test]
    fn an_empty_patch_changes_nothing() {
        let current = SeriesReaderSettings {
            reading_direction: Some(ReadingDirection::Rtl),
            ..Default::default()
        };

        assert_eq!(patch("{}").apply_to(current), current);
    }

    #[test]
    fn every_field_merges() {
        let merged = patch(
            r#"{
                "fitMode":"width-shrink",
                "webtoonFitMode":"original",
                "pageLayout":"continuous",
                "readingDirection":"webtoon",
                "backgroundColor":"gray",
                "doublePageShowWideAlone":true,
                "doublePageStartOnOdd":false
            }"#,
        )
        .apply_to(SeriesReaderSettings::default());

        assert_eq!(merged.fit_mode, Some(FitMode::WidthShrink));
        assert_eq!(merged.webtoon_fit_mode, Some(WebtoonFitMode::Original));
        assert_eq!(merged.page_layout, Some(PageLayout::Continuous));
        assert_eq!(merged.reading_direction, Some(ReadingDirection::Webtoon));
        assert_eq!(merged.background_color, Some(BackgroundColor::Gray));
        assert_eq!(merged.double_page_show_wide_alone, Some(true));
        assert_eq!(merged.double_page_start_on_odd, Some(false));
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
