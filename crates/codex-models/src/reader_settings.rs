//! Per-user, per-series reader settings.
//!
//! These are the reader settings that vary by content rather than by taste:
//! manga reads right to left, a webtoon scrolls continuously, an old two-page
//! scan wants a different layout than a modern single-page one. A user can
//! override them for one series without touching the series metadata that
//! every other user sees.
//!
//! The record is **sparse**: only the keys a user actually changed are stored.
//! Anything absent keeps inheriting, so a later correction to the series
//! metadata or the library default still reaches them. Storing a dense
//! snapshot would freeze all seven settings the moment one was touched.
//!
//! Only [`reading_direction`](SeriesReaderSettings::reading_direction) takes
//! part in server-side resolution, because it is the only one with series and
//! library layers beneath it. The rest are client-side rendering preferences
//! that ride along so one fetch covers everything the reader needs.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::reading_direction::ReadingDirection;

/// How a page is scaled to the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FitMode {
    Screen,
    Width,
    WidthShrink,
    Height,
    Original,
}

/// Fit mode for the webtoon reader, where only width and original make sense.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum WebtoonFitMode {
    Width,
    Original,
}

/// How many pages are shown at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum PageLayout {
    Single,
    Double,
    Continuous,
}

/// Backdrop behind the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundColor {
    Black,
    Gray,
    White,
}

/// A user's overrides for one series.
///
/// Every field is optional and `None` means "inherit". Unknown keys are
/// ignored rather than rejected, so a row written by a newer version does not
/// break a rollback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct SeriesReaderSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fit_mode: Option<FitMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webtoon_fit_mode: Option<WebtoonFitMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_layout: Option<PageLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reading_direction: Option<ReadingDirection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<BackgroundColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub double_page_show_wide_alone: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub double_page_start_on_odd: Option<bool>,
}

impl SeriesReaderSettings {
    /// Whether every setting is inherited.
    ///
    /// An empty record carries no information, so the repository deletes the
    /// row rather than storing `{}`: a present row means "this user has
    /// overrides here", and an empty one would lie to every later query.
    pub fn is_empty(&self) -> bool {
        self.fit_mode.is_none()
            && self.webtoon_fit_mode.is_none()
            && self.page_layout.is_none()
            && self.reading_direction.is_none()
            && self.background_color.is_none()
            && self.double_page_show_wide_alone.is_none()
            && self.double_page_start_on_odd.is_none()
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
            ..Default::default()
        };

        assert!(!settings.is_empty());
        // A sparse record: the six untouched settings keep inheriting rather
        // than being frozen at whatever they happened to be.
        assert_eq!(
            serde_json::to_string(&settings).unwrap(),
            r#"{"readingDirection":"rtl"}"#
        );
    }

    #[test]
    fn round_trips_through_json() {
        let settings = SeriesReaderSettings {
            fit_mode: Some(FitMode::WidthShrink),
            webtoon_fit_mode: Some(WebtoonFitMode::Original),
            page_layout: Some(PageLayout::Continuous),
            reading_direction: Some(ReadingDirection::Webtoon),
            background_color: Some(BackgroundColor::Gray),
            double_page_show_wide_alone: Some(true),
            double_page_start_on_odd: Some(false),
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
            fit_mode: Some(FitMode::Screen),
            double_page_start_on_odd: Some(true),
            ..Default::default()
        };

        let json = serde_json::to_value(settings).unwrap();
        assert_eq!(json["fitMode"], "screen");
        assert_eq!(json["doublePageStartOnOdd"], true);
    }

    #[test]
    fn fit_mode_uses_the_hyphenated_wire_value() {
        assert_eq!(
            serde_json::to_string(&FitMode::WidthShrink).unwrap(),
            r#""width-shrink""#
        );
    }

    #[test]
    fn unknown_keys_are_ignored_rather_than_rejected() {
        // A row written by a newer version must not break a rollback.
        let parsed: SeriesReaderSettings =
            serde_json::from_str(r#"{"readingDirection":"rtl","futureSetting":"whatever"}"#)
                .unwrap();
        assert_eq!(parsed.reading_direction, Some(ReadingDirection::Rtl));
    }

    #[test]
    fn invalid_values_are_rejected() {
        assert!(
            serde_json::from_str::<SeriesReaderSettings>(r#"{"readingDirection":"sideways"}"#)
                .is_err()
        );
        assert!(
            serde_json::from_str::<SeriesReaderSettings>(r#"{"backgroundColor":"chartreuse"}"#)
                .is_err()
        );
    }
}
