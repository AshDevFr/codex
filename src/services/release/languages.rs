//! Language-preference resolution for release-tracking.
//!
//! Aggregation feeds (e.g. MangaUpdates) emit candidates in many languages.
//! Plugins filter client-side using a per-series preference list, falling back
//! to a server-wide default and finally to a hardcoded `["en"]` fallback if
//! the setting is absent.
//!
//! This module is the single canonical resolver so the API, the
//! `releases/list_tracked` reverse-RPC, and the matcher all agree on the
//! effective list.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::db::repositories::SettingsRepository;

/// Settings key for the server-wide default language list.
pub const SERVER_DEFAULT_LANGUAGES_KEY: &str = "release_tracking.default_languages";

/// Hardcoded fallback when the server-wide setting is missing or unparseable.
/// Mirrors the migration seed.
pub fn hardcoded_default_languages() -> Vec<String> {
    vec!["en".to_string()]
}

/// Read the server-wide default-languages setting. Returns the hardcoded
/// fallback (`["en"]`) if the setting is missing or malformed.
pub async fn server_default_languages(db: &DatabaseConnection) -> Vec<String> {
    match SettingsRepository::get_value::<Vec<String>>(db, SERVER_DEFAULT_LANGUAGES_KEY).await {
        Ok(Some(langs)) if !langs.is_empty() => normalize(langs),
        // Missing setting, empty list, or parse error — fall back to ["en"].
        // Empty-list as fallback is a footgun (would silently hide everything),
        // so we treat it as "use the hardcoded default."
        _ => hardcoded_default_languages(),
    }
}

/// Resolve the effective language list for a single tracked series.
///
/// Precedence: per-series override (if non-empty) → server-wide default →
/// hardcoded `["en"]`.
pub fn effective_languages(per_series: Option<&Value>, server_default: &[String]) -> Vec<String> {
    if let Some(v) = per_series
        && let Some(arr) = v.as_array()
    {
        let langs: Vec<String> = arr
            .iter()
            .filter_map(|item| item.as_str().map(|s| s.to_string()))
            .collect();
        if !langs.is_empty() {
            return normalize(langs);
        }
    }
    if !server_default.is_empty() {
        return server_default.to_vec();
    }
    hardcoded_default_languages()
}

/// Whether a given candidate language is included in the effective list.
/// Case-insensitive on the language code.
pub fn includes(effective: &[String], language: &str) -> bool {
    let lang = language.trim().to_lowercase();
    if lang.is_empty() {
        return false;
    }
    effective.iter().any(|l| l.eq_ignore_ascii_case(&lang))
}

/// Normalize: trim, lowercase, drop empties, dedup-preserving-order.
fn normalize(langs: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(langs.len());
    for raw in langs {
        let lang = raw.trim().to_lowercase();
        if lang.is_empty() {
            continue;
        }
        if !out.iter().any(|existing| existing == &lang) {
            out.push(lang);
        }
    }
    out
}

/// Convenience: read both the per-series and server-default lists, return the
/// effective list. Used by the API and the reverse-RPC `list_tracked` handler.
pub async fn resolve_for_series(
    db: &DatabaseConnection,
    per_series: Option<&Value>,
) -> Result<Vec<String>> {
    let server_default = server_default_languages(db).await;
    Ok(effective_languages(per_series, &server_default))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hardcoded_default_is_english() {
        assert_eq!(hardcoded_default_languages(), vec!["en".to_string()]);
    }

    #[test]
    fn per_series_overrides_server_default() {
        let per_series = json!(["es", "fr"]);
        let server = vec!["en".to_string()];
        assert_eq!(
            effective_languages(Some(&per_series), &server),
            vec!["es".to_string(), "fr".to_string()]
        );
    }

    #[test]
    fn null_per_series_falls_back_to_server_default() {
        let server = vec!["en".to_string(), "es".to_string()];
        assert_eq!(
            effective_languages(None, &server),
            vec!["en".to_string(), "es".to_string()]
        );
    }

    #[test]
    fn empty_per_series_falls_back_to_server_default() {
        let per_series = json!([]);
        let server = vec!["en".to_string()];
        assert_eq!(
            effective_languages(Some(&per_series), &server),
            vec!["en".to_string()]
        );
    }

    #[test]
    fn empty_server_default_falls_back_to_hardcoded() {
        let server: Vec<String> = vec![];
        assert_eq!(effective_languages(None, &server), vec!["en".to_string()]);
    }

    #[test]
    fn normalizes_case_and_whitespace() {
        let per_series = json!(["EN", " es ", "FR"]);
        let server = vec!["en".to_string()];
        assert_eq!(
            effective_languages(Some(&per_series), &server),
            vec!["en".to_string(), "es".to_string(), "fr".to_string()]
        );
    }

    #[test]
    fn dedups_preserving_order() {
        let per_series = json!(["en", "es", "EN", "es"]);
        let server = vec!["en".to_string()];
        assert_eq!(
            effective_languages(Some(&per_series), &server),
            vec!["en".to_string(), "es".to_string()]
        );
    }

    #[test]
    fn ignores_non_string_entries() {
        let per_series = json!(["en", 42, null, "es"]);
        let server = vec!["en".to_string()];
        assert_eq!(
            effective_languages(Some(&per_series), &server),
            vec!["en".to_string(), "es".to_string()]
        );
    }

    #[test]
    fn includes_is_case_insensitive() {
        let effective = vec!["en".to_string(), "es".to_string()];
        assert!(includes(&effective, "en"));
        assert!(includes(&effective, "EN"));
        assert!(includes(&effective, " es "));
        assert!(!includes(&effective, "fr"));
    }

    #[test]
    fn includes_rejects_empty_language() {
        let effective = vec!["en".to_string()];
        assert!(!includes(&effective, ""));
        assert!(!includes(&effective, "   "));
    }
}
