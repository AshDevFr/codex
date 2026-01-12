//! Series-Volume scanning strategy
//!
//! This is the default strategy, compatible with Komga behavior:
//! - Direct child folders of library = series
//! - Files in those folders = books in that series
//! - Files directly in library root = "Unsorted" series

use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::models::SeriesStrategy;

use super::super::common::{DetectedBook, DetectedSeries};
use super::ScanningStrategyImpl;

/// Series-Volume strategy implementation
///
/// Direct child folders of library = series (Komga-compatible default)
pub struct SeriesVolumeStrategy;

impl SeriesVolumeStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SeriesVolumeStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl ScanningStrategyImpl for SeriesVolumeStrategy {
    fn strategy_type(&self) -> SeriesStrategy {
        SeriesStrategy::SeriesVolume
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

            // Set series path if not already set
            if series.path.is_none() && series_name != "Unsorted" {
                series.path = Some(series_name.clone());
            }

            series.add_book(DetectedBook::new(file_path.clone()));
        }

        Ok(series_map)
    }

    fn extract_series_name(&self, file_path: &Path, library_path: &Path) -> String {
        // Get relative path from library root
        let relative = file_path.strip_prefix(library_path).unwrap_or(file_path);

        // Get first component (direct child folder)
        let components: Vec<_> = relative.components().collect();

        if components.len() > 1 {
            // Use first folder as series name
            components[0].as_os_str().to_string_lossy().to_string()
        } else {
            // File is directly in library root
            "Unsorted".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strategy() -> SeriesVolumeStrategy {
        SeriesVolumeStrategy::new()
    }

    #[test]
    fn test_extract_series_name_from_folder() {
        let library = Path::new("/library");
        let strategy = strategy();

        let path = PathBuf::from("/library/Batman/issue1.cbz");
        assert_eq!(strategy.extract_series_name(&path, library), "Batman");
    }

    #[test]
    fn test_extract_series_name_nested_folder() {
        let library = Path::new("/library");
        let strategy = strategy();

        // Nested folders should still use the first level as series
        let path = PathBuf::from("/library/Batman/Year One/issue1.cbz");
        assert_eq!(strategy.extract_series_name(&path, library), "Batman");
    }

    #[test]
    fn test_extract_series_name_library_root() {
        let library = Path::new("/library");
        let strategy = strategy();

        // Files directly in library root should be "Unsorted"
        let path = PathBuf::from("/library/standalone.cbz");
        assert_eq!(strategy.extract_series_name(&path, library), "Unsorted");
    }

    #[test]
    fn test_organize_files() {
        let library = Path::new("/library");
        let strategy = strategy();

        let files = vec![
            PathBuf::from("/library/Batman/issue1.cbz"),
            PathBuf::from("/library/Batman/issue2.cbz"),
            PathBuf::from("/library/Superman/issue1.cbz"),
            PathBuf::from("/library/standalone.cbz"),
        ];

        let result = strategy.organize_files(&files, library).unwrap();

        assert_eq!(result.len(), 3);
        assert!(result.contains_key("Batman"));
        assert!(result.contains_key("Superman"));
        assert!(result.contains_key("Unsorted"));

        assert_eq!(result["Batman"].books.len(), 2);
        assert_eq!(result["Superman"].books.len(), 1);
        assert_eq!(result["Unsorted"].books.len(), 1);
    }

    #[test]
    fn test_organize_files_preserves_series_path() {
        let library = Path::new("/library");
        let strategy = strategy();

        let files = vec![
            PathBuf::from("/library/My Comics/issue1.cbz"),
            PathBuf::from("/library/standalone.cbz"),
        ];

        let result = strategy.organize_files(&files, library).unwrap();

        assert_eq!(result["My Comics"].path, Some("My Comics".to_string()));
        assert_eq!(result["Unsorted"].path, None);
    }

    #[test]
    fn test_strategy_type() {
        assert_eq!(strategy().strategy_type(), SeriesStrategy::SeriesVolume);
    }
}
