//! Series-Volume scanning strategy
//!
//! This is the default strategy:
//! - Immediate parent folder of each file = series
//! - Files in the same folder = books in that series
//! - Files directly in library root = individual oneshot series (one per file, named after the file)

use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::models::SeriesStrategy;

use super::super::common::{DetectedBook, DetectedSeries};
use super::ScanningStrategyImpl;

/// Series-Volume strategy implementation
///
/// Immediate parent folder = series (file's containing folder)
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

            // Get the relative path to the series folder (parent of the file)
            // For root-level files, this will be an empty string
            let series_path = file_path
                .parent()
                .and_then(|p| p.strip_prefix(library_path).ok())
                .map(|p| p.to_string_lossy().to_string());

            let series = series_map
                .entry(series_name.clone())
                .or_insert_with(|| DetectedSeries::new(&series_name));

            // Set series path if not already set
            // For root-level files, the series path will be an empty string
            if series.path.is_none() {
                series.path = series_path;
            }

            series.add_book(DetectedBook::new(file_path.clone()));
        }

        Ok(series_map)
    }

    fn extract_series_name(&self, file_path: &Path, library_path: &Path) -> String {
        // Get the immediate parent folder (containing folder) as the series name
        if let Some(parent) = file_path.parent() {
            // Check if parent is the library root
            if parent == library_path {
                // For root-level books, use the filename (without extension) as series name
                // This creates individual oneshot series for each standalone book
                return file_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Unsorted".to_string());
            }

            // Use the folder name (not the full path) as series name
            if let Some(folder_name) = parent.file_name() {
                return folder_name.to_string_lossy().to_string();
            }
        }

        "Unsorted".to_string()
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

        // Nested folders should use the immediate parent folder as series
        let path = PathBuf::from("/library/Batman/Year One/issue1.cbz");
        assert_eq!(strategy.extract_series_name(&path, library), "Year One");
    }

    #[test]
    fn test_extract_series_name_deeply_nested() {
        let library = Path::new("/library");
        let strategy = strategy();

        // Deeply nested: immediate parent folder is the series
        let path = PathBuf::from("/library/_to_filter/Say Hello to Black Jack/book.cbz");
        assert_eq!(
            strategy.extract_series_name(&path, library),
            "Say Hello to Black Jack"
        );
    }

    #[test]
    fn test_extract_series_name_library_root() {
        let library = Path::new("/library");
        let strategy = strategy();

        // Files directly in library root should use filename (without extension) as series name
        // This creates individual oneshot series for each standalone book
        let path = PathBuf::from("/library/standalone.cbz");
        assert_eq!(strategy.extract_series_name(&path, library), "standalone");

        // Another example with a book title
        let path = PathBuf::from("/library/The Martian.epub");
        assert_eq!(strategy.extract_series_name(&path, library), "The Martian");
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

        // Root-level files now create their own series (named after the file)
        assert_eq!(result.len(), 3);
        assert!(result.contains_key("Batman"));
        assert!(result.contains_key("Superman"));
        assert!(result.contains_key("standalone")); // Now uses filename as series name

        assert_eq!(result["Batman"].books.len(), 2);
        assert_eq!(result["Superman"].books.len(), 1);
        assert_eq!(result["standalone"].books.len(), 1); // Each root file = 1 book
    }

    #[test]
    fn test_organize_files_nested_creates_separate_series() {
        let library = Path::new("/library");
        let strategy = strategy();

        // Files in different immediate parent folders = different series
        let files = vec![
            PathBuf::from("/library/Say Hello to Black Jack/book.cbz"),
            PathBuf::from("/library/_to_filter/Say Hello to Black Jack Filter/book.cbz"),
        ];

        let result = strategy.organize_files(&files, library).unwrap();

        // Should create 2 separate series based on immediate parent folder
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("Say Hello to Black Jack"));
        assert!(result.contains_key("Say Hello to Black Jack Filter"));

        assert_eq!(result["Say Hello to Black Jack"].books.len(), 1);
        assert_eq!(result["Say Hello to Black Jack Filter"].books.len(), 1);
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
        // Root-level files now create their own series (named after the file)
        // The series path is empty for files at the library root
        assert_eq!(result["standalone"].path, Some(String::new()));
    }

    #[test]
    fn test_organize_files_nested_preserves_full_path() {
        let library = Path::new("/library");
        let strategy = strategy();

        let files = vec![PathBuf::from(
            "/library/_to_filter/Say Hello to Black Jack/book.cbz",
        )];

        let result = strategy.organize_files(&files, library).unwrap();

        // Series path should be the full relative path to the series folder
        assert_eq!(
            result["Say Hello to Black Jack"].path,
            Some("_to_filter/Say Hello to Black Jack".to_string())
        );
    }

    #[test]
    fn test_strategy_type() {
        assert_eq!(strategy().strategy_type(), SeriesStrategy::SeriesVolume);
    }

    #[test]
    fn test_organize_files_multiple_root_level_books() {
        let library = Path::new("/library");
        let strategy = strategy();

        // Multiple root-level files should each create their own oneshot series
        let files = vec![
            PathBuf::from("/library/The Martian.epub"),
            PathBuf::from("/library/Dune.epub"),
            PathBuf::from("/library/Project Hail Mary.epub"),
        ];

        let result = strategy.organize_files(&files, library).unwrap();

        // Each root-level file creates its own series
        assert_eq!(result.len(), 3);
        assert!(result.contains_key("The Martian"));
        assert!(result.contains_key("Dune"));
        assert!(result.contains_key("Project Hail Mary"));

        // Each series has exactly one book (oneshot)
        assert_eq!(result["The Martian"].books.len(), 1);
        assert_eq!(result["Dune"].books.len(), 1);
        assert_eq!(result["Project Hail Mary"].books.len(), 1);

        // Series paths should be empty for root-level files
        assert_eq!(result["The Martian"].path, Some(String::new()));
        assert_eq!(result["Dune"].path, Some(String::new()));
        assert_eq!(result["Project Hail Mary"].path, Some(String::new()));
    }

    #[test]
    fn test_organize_files_mixed_root_and_folder() {
        let library = Path::new("/library");
        let strategy = strategy();

        // Mix of root-level files and folder-based series
        let files = vec![
            PathBuf::from("/library/Standalone Novel.epub"),
            PathBuf::from("/library/One Piece/vol01.cbz"),
            PathBuf::from("/library/One Piece/vol02.cbz"),
            PathBuf::from("/library/One Piece/vol03.cbz"),
            PathBuf::from("/library/Another Standalone.pdf"),
        ];

        let result = strategy.organize_files(&files, library).unwrap();

        // 2 oneshot series + 1 multi-volume series
        assert_eq!(result.len(), 3);
        assert!(result.contains_key("Standalone Novel"));
        assert!(result.contains_key("Another Standalone"));
        assert!(result.contains_key("One Piece"));

        // Oneshots have 1 book each
        assert_eq!(result["Standalone Novel"].books.len(), 1);
        assert_eq!(result["Another Standalone"].books.len(), 1);
        // Multi-volume series has 3 books
        assert_eq!(result["One Piece"].books.len(), 3);

        // Root-level series have empty path
        assert_eq!(result["Standalone Novel"].path, Some(String::new()));
        assert_eq!(result["Another Standalone"].path, Some(String::new()));
        // Folder-based series have folder path
        assert_eq!(result["One Piece"].path, Some("One Piece".to_string()));
    }
}
