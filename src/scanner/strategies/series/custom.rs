//! Custom scanning strategy
//!
//! User-defined regex patterns for series detection:
//! - Pattern must have named capture groups: publisher, series, book
//! - Series name template uses captured groups: "{publisher} - {series}"
//!
//! Note: Volume/chapter extraction from filenames is handled by the book strategy,
//! not the series strategy. Use CustomBookConfig for regex-based volume/chapter parsing.

use anyhow::Result;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::models::{CustomStrategyConfig, SeriesStrategy};

use super::super::common::{DetectedBook, DetectedSeries, SeriesMetadata};
use super::ScanningStrategyImpl;

lazy_static! {
    /// Default pattern for extracting book numbers from filenames
    /// Used internally for initial ordering; actual volume/chapter extraction
    /// is handled by the book naming strategy.
    static ref DEFAULT_BOOK_NUMBER_PATTERN: Regex =
        Regex::new(r"(?:v|#|vol|chapter)\s*(\d+)").unwrap();
}

/// Custom strategy implementation
///
/// User-defined regex patterns for maximum flexibility
pub struct CustomStrategy {
    config: CustomStrategyConfig,
    /// Compiled main pattern
    pattern: Regex,
}

impl CustomStrategy {
    pub fn new(config: CustomStrategyConfig) -> Result<Self> {
        let pattern = Regex::new(&config.pattern)
            .map_err(|e| anyhow::anyhow!("Invalid pattern '{}': {}", config.pattern, e))?;

        Ok(Self { config, pattern })
    }

    /// Apply template substitution with captured groups
    fn apply_template(&self, captures: &regex::Captures, template: &str) -> String {
        let mut result = template.to_string();

        // Replace named groups
        for name in ["publisher", "series", "volume", "book"] {
            if let Some(m) = captures.name(name) {
                result = result.replace(&format!("{{{}}}", name), m.as_str());
            } else {
                result = result.replace(&format!("{{{}}}", name), "");
            }
        }

        // Clean up any leftover braces and trim
        result = result.replace("{", "").replace("}", "");
        result.trim().to_string()
    }

    /// Extract metadata from captures
    fn extract_metadata(&self, captures: &regex::Captures) -> SeriesMetadata {
        let mut metadata = SeriesMetadata::default();

        if let Some(m) = captures.name("publisher") {
            metadata.publisher = Some(m.as_str().to_string());
        }

        // Store all named captures in extra
        for name in ["publisher", "volume", "year", "imprint"] {
            if let Some(m) = captures.name(name) {
                metadata
                    .extra
                    .insert(name.to_string(), m.as_str().to_string());
            }
        }

        metadata
    }

    /// Extract book number from filename using default pattern
    ///
    /// This provides basic ordering during scanning; more precise volume/chapter
    /// extraction is handled by the book naming strategy.
    fn extract_book_number(&self, filename: &str) -> Option<f32> {
        if let Some(caps) = DEFAULT_BOOK_NUMBER_PATTERN.captures(filename) {
            caps.get(1)?.as_str().parse::<f32>().ok()
        } else {
            None
        }
    }
}

impl ScanningStrategyImpl for CustomStrategy {
    fn strategy_type(&self) -> SeriesStrategy {
        SeriesStrategy::Custom
    }

    fn organize_files(
        &self,
        files: &[PathBuf],
        library_path: &Path,
    ) -> Result<HashMap<String, DetectedSeries>> {
        let mut series_map: HashMap<String, DetectedSeries> = HashMap::new();

        for file_path in files {
            // Get relative path for pattern matching
            let relative = file_path
                .strip_prefix(library_path)
                .unwrap_or(file_path)
                .to_string_lossy();

            let (series_name, metadata) = if let Some(caps) = self.pattern.captures(&relative) {
                let name = self.apply_template(&caps, &self.config.series_name_template);
                let meta = self.extract_metadata(&caps);
                (name, meta)
            } else {
                // Fallback: use parent folder name
                let parent = file_path
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Unsorted".to_string());
                (parent, SeriesMetadata::default())
            };

            let series_name = if series_name.is_empty() {
                "Unsorted".to_string()
            } else {
                series_name
            };

            let series = series_map.entry(series_name.clone()).or_insert_with(|| {
                let mut s = DetectedSeries::new(&series_name);
                s.metadata = metadata;
                s
            });

            // Set series path from file path
            if series.path.is_none() {
                if let Some(parent) = file_path.parent() {
                    if let Ok(rel_parent) = parent.strip_prefix(library_path) {
                        series.path = Some(rel_parent.to_string_lossy().to_string());
                    }
                }
            }

            // Extract book number
            let mut book = DetectedBook::new(file_path.clone());
            if let Some(filename) = file_path.file_name() {
                book.number = self.extract_book_number(&filename.to_string_lossy());
            }

            series.add_book(book);
        }

        Ok(series_map)
    }

    fn extract_series_name(&self, file_path: &Path, library_path: &Path) -> String {
        let relative = file_path
            .strip_prefix(library_path)
            .unwrap_or(file_path)
            .to_string_lossy();

        if let Some(caps) = self.pattern.captures(&relative) {
            let name = self.apply_template(&caps, &self.config.series_name_template);
            if !name.is_empty() {
                return name;
            }
        }

        // Fallback
        file_path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unsorted".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strategy_simple() -> CustomStrategy {
        CustomStrategy::new(CustomStrategyConfig {
            pattern: r"^(?P<series>[^/]+)/(?P<book>.+)\.(cbz|cbr|epub|pdf)$".to_string(),
            series_name_template: "{series}".to_string(),
        })
        .unwrap()
    }

    fn strategy_with_publisher() -> CustomStrategy {
        CustomStrategy::new(CustomStrategyConfig {
            pattern: r"^(?P<publisher>[^/]+)/(?P<series>[^/]+)/(?P<book>.+)\.(cbz|cbr|epub|pdf)$"
                .to_string(),
            series_name_template: "{publisher} - {series}".to_string(),
        })
        .unwrap()
    }

    #[test]
    fn test_simple_pattern() {
        let library = Path::new("/library");
        let strategy = strategy_simple();

        let path = PathBuf::from("/library/Batman/issue1.cbz");
        assert_eq!(strategy.extract_series_name(&path, library), "Batman");
    }

    #[test]
    fn test_publisher_pattern() {
        let library = Path::new("/library");
        let strategy = strategy_with_publisher();

        let path = PathBuf::from("/library/Marvel/Spider-Man/issue1.cbz");
        assert_eq!(
            strategy.extract_series_name(&path, library),
            "Marvel - Spider-Man"
        );
    }

    #[test]
    fn test_apply_template() {
        let strategy = strategy_with_publisher();
        let pattern = &strategy.pattern;

        let text = "Marvel/Spider-Man/issue1.cbz";
        let caps = pattern.captures(text).unwrap();

        assert_eq!(
            strategy.apply_template(&caps, "{publisher} - {series}"),
            "Marvel - Spider-Man"
        );
        assert_eq!(
            strategy.apply_template(&caps, "{series} ({publisher})"),
            "Spider-Man (Marvel)"
        );
    }

    #[test]
    fn test_extract_metadata() {
        let library = Path::new("/library");
        let strategy = strategy_with_publisher();

        let files = vec![PathBuf::from("/library/Marvel/Spider-Man/issue1.cbz")];

        let result = strategy.organize_files(&files, library).unwrap();

        assert_eq!(
            result["Marvel - Spider-Man"].metadata.publisher,
            Some("Marvel".to_string())
        );
    }

    #[test]
    fn test_extract_book_number() {
        let strategy = strategy_simple();

        assert_eq!(strategy.extract_book_number("Batman #001.cbz"), Some(1.0));
        assert_eq!(strategy.extract_book_number("Batman v05.cbz"), Some(5.0));
        assert_eq!(
            strategy.extract_book_number("Batman chapter 15.cbz"),
            Some(15.0)
        );
    }

    #[test]
    fn test_fallback_when_no_match() {
        let library = Path::new("/library");
        let strategy = strategy_simple();

        // File that doesn't match pattern (no extension match)
        let path = PathBuf::from("/library/Some Folder/random.txt");
        let name = strategy.extract_series_name(&path, library);

        // Should fallback to parent folder name
        assert_eq!(name, "Some Folder");
    }

    #[test]
    fn test_organize_files() {
        let library = Path::new("/library");
        let strategy = strategy_simple();

        let files = vec![
            PathBuf::from("/library/Batman/issue #1.cbz"),
            PathBuf::from("/library/Batman/issue #2.cbz"),
            PathBuf::from("/library/Superman/issue #1.cbz"),
        ];

        let result = strategy.organize_files(&files, library).unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.contains_key("Batman"));
        assert!(result.contains_key("Superman"));

        assert_eq!(result["Batman"].books.len(), 2);
        assert_eq!(result["Batman"].books[0].number, Some(1.0));
    }

    #[test]
    fn test_invalid_pattern() {
        let result = CustomStrategy::new(CustomStrategyConfig {
            pattern: r"[invalid".to_string(), // Unclosed bracket
            ..Default::default()
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_strategy_type() {
        assert_eq!(strategy_simple().strategy_type(), SeriesStrategy::Custom);
    }
}
