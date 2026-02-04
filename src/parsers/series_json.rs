//! Mylar series.json parser
//!
//! Parses Mylar's `series.json` sidecar files (schema version 1.0.2) to extract
//! series-level metadata such as publisher, year, description, and status.

use crate::utils::{CodexError, Result};
use serde::Deserialize;
use std::path::Path;

/// Top-level series.json wrapper
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct MylarSeriesJson {
    #[serde(default)]
    pub version: Option<String>,
    pub metadata: MylarSeriesMetadata,
}

/// Metadata object within series.json
#[derive(Debug, Clone, Deserialize, Default)]
#[allow(dead_code)]
pub struct MylarSeriesMetadata {
    #[serde(default, rename = "type")]
    pub series_type: Option<String>,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub imprint: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub comicid: Option<i64>,
    #[serde(default)]
    pub year: Option<i32>,
    #[serde(default)]
    pub description_text: Option<String>,
    #[serde(default)]
    pub description_formatted: Option<String>,
    #[serde(default)]
    pub volume: Option<i32>,
    #[serde(default)]
    pub booktype: Option<String>,
    #[serde(default)]
    pub age_rating: Option<String>,
    #[serde(default)]
    pub collects: Option<Vec<MylarCollects>>,
    #[serde(default)]
    pub comic_image: Option<String>,
    #[serde(default)]
    pub total_issues: Option<i32>,
    #[serde(default)]
    pub publication_run: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

/// Collected issues entry (for TPB/GN types)
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct MylarCollects {
    #[serde(default)]
    pub series: Option<String>,
    #[serde(default)]
    pub comicid: Option<String>,
    #[serde(default)]
    pub issueid: Option<String>,
    #[serde(default)]
    pub issues: Option<String>,
}

/// Parse series.json content from a string.
pub fn parse_series_json(content: &str) -> Result<MylarSeriesMetadata> {
    let wrapper: MylarSeriesJson = serde_json::from_str(content)
        .map_err(|e| CodexError::ParseError(format!("Failed to parse series.json: {}", e)))?;
    Ok(wrapper.metadata)
}

/// Read and parse a series.json file from disk.
pub fn parse_series_json_file(path: &Path) -> Result<MylarSeriesMetadata> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        CodexError::ParseError(format!(
            "Failed to read series.json file {}: {}",
            path.display(),
            e
        ))
    })?;
    parse_series_json(&content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    const FULL_SERIES_JSON: &str = r##"{
        "version": "1.0.2",
        "metadata": {
            "type": "comicSeries",
            "publisher": "DC Comics",
            "imprint": null,
            "name": "Aquaman",
            "comicid": 43022,
            "year": 2011,
            "description_text": "A \"New 52\" initiative title starring Aquaman and Mera.",
            "description_formatted": null,
            "volume": 5,
            "booktype": "Print",
            "age_rating": null,
            "collects": null,
            "comic_image": "https://comicvine.gamespot.com/a/uploads/scale_large/6/67663/2083416-01.jpg",
            "total_issues": 55,
            "publication_run": "November 2011 - July 2016",
            "status": "Ended"
        }
    }"##;

    const TPB_WITH_COLLECTS: &str = r##"{
        "version": "1.0.2",
        "metadata": {
            "type": "comicSeries",
            "publisher": "DC Comics",
            "imprint": null,
            "name": "Superman: Zero Hour",
            "comicid": 111701,
            "year": 2018,
            "description_text": "Trade paperback collecting various issues.",
            "description_formatted": "Trade paperback collecting various issues.",
            "volume": null,
            "booktype": "TPB",
            "age_rating": null,
            "collects": [
                {
                    "series": "Action Comics",
                    "comicid": "4050-18005",
                    "issueid": null,
                    "issues": "#0 & 703"
                },
                {
                    "series": "Steel",
                    "comicid": "4050-5260",
                    "issueid": null,
                    "issues": "#0 & 8"
                }
            ],
            "comic_image": "https://example.com/image.jpg",
            "total_issues": 1,
            "publication_run": "June 2018 - June 2018",
            "status": "Ended"
        }
    }"##;

    #[test]
    fn test_parse_valid_series_json() {
        let meta = parse_series_json(FULL_SERIES_JSON).unwrap();
        assert_eq!(meta.name.as_deref(), Some("Aquaman"));
        assert_eq!(meta.publisher.as_deref(), Some("DC Comics"));
        assert_eq!(meta.year, Some(2011));
        assert_eq!(
            meta.description_text.as_deref(),
            Some("A \"New 52\" initiative title starring Aquaman and Mera.")
        );
        assert_eq!(meta.volume, Some(5));
        assert_eq!(meta.booktype.as_deref(), Some("Print"));
        assert_eq!(meta.total_issues, Some(55));
        assert_eq!(
            meta.publication_run.as_deref(),
            Some("November 2011 - July 2016")
        );
        assert_eq!(meta.status.as_deref(), Some("Ended"));
        assert_eq!(meta.comicid, Some(43022));
        assert!(meta.comic_image.is_some());
        assert_eq!(meta.series_type.as_deref(), Some("comicSeries"));
    }

    #[test]
    fn test_parse_null_fields() {
        let meta = parse_series_json(FULL_SERIES_JSON).unwrap();
        assert!(meta.imprint.is_none());
        assert!(meta.age_rating.is_none());
        assert!(meta.collects.is_none());
        assert!(meta.description_formatted.is_none());
    }

    #[test]
    fn test_parse_tpb_with_collects() {
        let meta = parse_series_json(TPB_WITH_COLLECTS).unwrap();
        assert_eq!(meta.name.as_deref(), Some("Superman: Zero Hour"));
        assert_eq!(meta.booktype.as_deref(), Some("TPB"));

        let collects = meta.collects.as_ref().unwrap();
        assert_eq!(collects.len(), 2);
        assert_eq!(collects[0].series.as_deref(), Some("Action Comics"));
        assert_eq!(collects[0].comicid.as_deref(), Some("4050-18005"));
        assert!(collects[0].issueid.is_none());
        assert_eq!(collects[0].issues.as_deref(), Some("#0 & 703"));
        assert_eq!(collects[1].series.as_deref(), Some("Steel"));
    }

    #[test]
    fn test_parse_missing_fields() {
        // Minimal document — only required wrapper structure
        let json = r#"{"metadata": {"name": "Minimal Series"}}"#;
        let meta = parse_series_json(json).unwrap();
        assert_eq!(meta.name.as_deref(), Some("Minimal Series"));
        assert!(meta.publisher.is_none());
        assert!(meta.year.is_none());
        assert!(meta.description_text.is_none());
        assert!(meta.status.is_none());
        assert!(meta.comicid.is_none());
        assert!(meta.volume.is_none());
        assert!(meta.booktype.is_none());
        assert!(meta.total_issues.is_none());
        assert!(meta.collects.is_none());
        // version is on the wrapper, not metadata — just verify parse succeeds
    }

    #[test]
    fn test_parse_unknown_fields() {
        // Extra fields should not cause errors
        let json = r#"{
            "version": "1.0.2",
            "metadata": {
                "name": "Test",
                "publisher": "Test Publisher",
                "unknown_field": "should be ignored",
                "another_extra": 42
            }
        }"#;
        let meta = parse_series_json(json).unwrap();
        assert_eq!(meta.name.as_deref(), Some("Test"));
        assert_eq!(meta.publisher.as_deref(), Some("Test Publisher"));
    }

    #[test]
    fn test_parse_invalid_json() {
        let result = parse_series_json("not valid json");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Failed to parse series.json"));
    }

    #[test]
    fn test_parse_series_json_file() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", FULL_SERIES_JSON).unwrap();

        let meta = parse_series_json_file(file.path()).unwrap();
        assert_eq!(meta.name.as_deref(), Some("Aquaman"));
        assert_eq!(meta.publisher.as_deref(), Some("DC Comics"));
        assert_eq!(meta.year, Some(2011));
    }

    #[test]
    fn test_parse_series_json_file_not_found() {
        let result = parse_series_json_file(Path::new("/nonexistent/series.json"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_continuing_series() {
        let json = r#"{
            "version": "1.0.2",
            "metadata": {
                "type": "comicSeries",
                "publisher": "DC Comics",
                "name": "Batman/Fortnite: Zero Point",
                "comicid": 135499,
                "year": 2021,
                "description_text": "Six issue crossover mini-series",
                "booktype": "Print",
                "comic_image": "https://example.com/image.jpg",
                "total_issues": 1,
                "publication_run": "June 2021 - Present",
                "status": "Continuing"
            }
        }"#;
        let meta = parse_series_json(json).unwrap();
        assert_eq!(meta.status.as_deref(), Some("Continuing"));
        assert_eq!(meta.publication_run.as_deref(), Some("June 2021 - Present"));
    }
}
