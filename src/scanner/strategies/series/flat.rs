//! Flat scanning strategy
//!
//! All files at library root level, series detected from filename patterns or metadata:
//! - [One Piece] v01.cbz → Series: "One Piece"
//! - One Piece - v01.cbz → Series: "One Piece"

use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::models::{FlatStrategyConfig, SeriesStrategy};

use super::super::common::{DetectedBook, DetectedSeries};
use super::ScanningStrategyImpl;

/// Flat structure strategy implementation
///
/// All files at library root, series detected from filename or metadata
pub struct FlatStrategy {
    config: FlatStrategyConfig,
    /// Compiled regex patterns for series extraction
    patterns: Vec<Regex>,
    /// Pattern to extract book number from filename
    number_pattern: Regex,
}

impl FlatStrategy {
    pub fn new(config: FlatStrategyConfig) -> Self {
        // Compile the regex patterns
        let patterns = config
            .filename_patterns
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        Self {
            config,
            patterns,
            number_pattern: Regex::new(
                r"(?i)(?:v|vol\.?|volume|#|ch\.?|chapter)\s*(\d+(?:\.\d+)?)",
            )
            .unwrap(),
        }
    }

    /// Extract series name from filename using configured patterns
    fn extract_from_filename(&self, filename: &str) -> Option<String> {
        for pattern in &self.patterns {
            if let Some(caps) = pattern.captures(filename) {
                if let Some(m) = caps.get(1) {
                    let name = m.as_str().trim().to_string();
                    if !name.is_empty() {
                        return Some(name);
                    }
                }
            }
        }
        None
    }

    /// Extract book number from filename
    fn extract_number(&self, filename: &str) -> Option<f32> {
        if let Some(caps) = self.number_pattern.captures(filename) {
            caps.get(1)?.as_str().parse::<f32>().ok()
        } else {
            // Try to extract any number
            let num_pattern = Regex::new(r"(\d+(?:\.\d+)?)").unwrap();
            if let Some(caps) = num_pattern.captures(filename) {
                caps.get(1)?.as_str().parse::<f32>().ok()
            } else {
                None
            }
        }
    }

    /// Fallback series name extraction when no pattern matches
    fn fallback_series_name(&self, filename: &str) -> String {
        // Remove extension
        let name = filename
            .rsplit_once('.')
            .map(|(n, _)| n)
            .unwrap_or(filename);

        // Try to get series from first words (before numbers)
        let word_pattern = Regex::new(r"^([A-Za-z][A-Za-z\s]*?)(?:\s*[\d#\[\(]|$)").unwrap();
        if let Some(caps) = word_pattern.captures(name) {
            let series = caps.get(1).unwrap().as_str().trim();
            if !series.is_empty() {
                return series.to_string();
            }
        }

        // Last resort: use filename without extension
        name.to_string()
    }
}

impl ScanningStrategyImpl for FlatStrategy {
    fn strategy_type(&self) -> SeriesStrategy {
        SeriesStrategy::Flat
    }

    fn organize_files(
        &self,
        files: &[PathBuf],
        library_path: &Path,
    ) -> Result<HashMap<String, DetectedSeries>> {
        let mut series_map: HashMap<String, DetectedSeries> = HashMap::new();

        for file_path in files {
            let series_name = self.extract_series_name(file_path, library_path);

            let series = series_map
                .entry(series_name.clone())
                .or_insert_with(|| DetectedSeries::new(&series_name));

            let mut book = DetectedBook::new(file_path.clone());

            // Extract book number from filename
            if let Some(filename) = file_path.file_name() {
                book.number = self.extract_number(&filename.to_string_lossy());
            }

            series.add_book(book);
        }

        Ok(series_map)
    }

    fn extract_series_name(&self, file_path: &Path, _library_path: &Path) -> String {
        let filename = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Try pattern extraction
        if let Some(name) = self.extract_from_filename(&filename) {
            return name;
        }

        // Fallback if require_metadata is false
        if self.config.require_metadata {
            "Unknown".to_string()
        } else {
            self.fallback_series_name(&filename)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strategy() -> FlatStrategy {
        FlatStrategy::new(FlatStrategyConfig::default())
    }

    #[test]
    fn test_extract_bracket_pattern() {
        let strategy = strategy();

        let path = PathBuf::from("/library/[One Piece] v01.cbz");
        assert_eq!(
            strategy.extract_series_name(&path, Path::new("/library")),
            "One Piece"
        );
    }

    #[test]
    fn test_extract_dash_pattern() {
        let strategy = strategy();

        let path = PathBuf::from("/library/Batman - The Long Halloween #01.cbz");
        assert_eq!(
            strategy.extract_series_name(&path, Path::new("/library")),
            "Batman"
        );
    }

    #[test]
    fn test_extract_underscore_pattern() {
        let strategy = strategy();

        let path = PathBuf::from("/library/Spider-Man_001.cbz");
        assert_eq!(
            strategy.extract_series_name(&path, Path::new("/library")),
            "Spider-Man"
        );
    }

    #[test]
    fn test_extract_fallback() {
        let strategy = strategy();

        // No bracket, dash, or underscore pattern - use fallback
        let path = PathBuf::from("/library/Batman 001.cbz");
        let name = strategy.extract_series_name(&path, Path::new("/library"));
        assert_eq!(name, "Batman");
    }

    #[test]
    fn test_extract_number_volume() {
        let strategy = strategy();

        assert_eq!(strategy.extract_number("One Piece v01.cbz"), Some(1.0));
        assert_eq!(strategy.extract_number("One Piece Vol. 15.cbz"), Some(15.0));
        assert_eq!(strategy.extract_number("Batman #42.cbz"), Some(42.0));
    }

    #[test]
    fn test_extract_number_chapter() {
        let strategy = strategy();

        assert_eq!(
            strategy.extract_number("One Piece Chapter 500.cbz"),
            Some(500.0)
        );
        assert_eq!(
            strategy.extract_number("One Piece Ch. 15.5.cbz"),
            Some(15.5)
        );
    }

    #[test]
    fn test_organize_files() {
        let library = Path::new("/library");
        let strategy = strategy();

        let files = vec![
            PathBuf::from("/library/[One Piece] v01.cbz"),
            PathBuf::from("/library/[One Piece] v02.cbz"),
            PathBuf::from("/library/[Naruto] Chapter 001.cbz"),
            PathBuf::from("/library/Batman - #001.cbz"),
        ];

        let result = strategy.organize_files(&files, library).unwrap();

        assert_eq!(result.len(), 3);
        assert!(result.contains_key("One Piece"));
        assert!(result.contains_key("Naruto"));
        assert!(result.contains_key("Batman"));

        assert_eq!(result["One Piece"].books.len(), 2);
        assert_eq!(result["One Piece"].books[0].number, Some(1.0));
        assert_eq!(result["One Piece"].books[1].number, Some(2.0));
    }

    #[test]
    fn test_require_metadata_mode() {
        let config = FlatStrategyConfig {
            require_metadata: true,
            ..Default::default()
        };
        let strategy = FlatStrategy::new(config);

        // File with no matching pattern and require_metadata = true
        // Note: filename must not match any default patterns:
        // - \[([^\]]+)\]  (bracket format)
        // - ^([^-]+) -    (dash format)
        // - ^([^_]+)_     (underscore format)
        let path = PathBuf::from("/library/somebook01.cbz");
        assert_eq!(
            strategy.extract_series_name(&path, Path::new("/library")),
            "Unknown"
        );
    }

    #[test]
    fn test_custom_patterns() {
        let config = FlatStrategyConfig {
            filename_patterns: vec![
                r"^(.+?)_v\d+".to_string(), // SeriesName_v01 format
            ],
            require_metadata: false,
        };
        let strategy = FlatStrategy::new(config);

        let path = PathBuf::from("/library/MyManga_v01.cbz");
        assert_eq!(
            strategy.extract_series_name(&path, Path::new("/library")),
            "MyManga"
        );
    }

    #[test]
    fn test_strategy_type() {
        assert_eq!(strategy().strategy_type(), SeriesStrategy::Flat);
    }
}
