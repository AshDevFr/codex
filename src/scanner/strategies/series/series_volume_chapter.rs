//! Series-Volume-Chapter scanning strategy
//!
//! For chapter-based manga and web comics:
//! - Immediate parent folder = series (chapter folder)
//! - Second-to-last folder = volume/arc (if present)
//! - Files in the same folder = books in that series

use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::models::SeriesStrategy;

use super::super::common::{DetectedBook, DetectedSeries};
use super::ScanningStrategyImpl;

/// Series-Volume-Chapter strategy implementation
///
/// Immediate parent folder = series, second-to-last = volume/arc
pub struct SeriesVolumeChapterStrategy {
    /// Pattern to extract volume number from folder name
    volume_pattern: Regex,
    /// Pattern to extract chapter number from filename
    chapter_pattern: Regex,
}

impl SeriesVolumeChapterStrategy {
    pub fn new() -> Self {
        Self {
            // Matches: "Volume 01", "Vol. 1", "Vol 01", "v01", "V1", etc.
            volume_pattern: Regex::new(r"(?i)(?:volume|vol\.?|v)\s*(\d+)").unwrap(),
            // Matches: "Chapter 001", "Ch. 1", "ch01", "c1", "#001", etc.
            chapter_pattern: Regex::new(r"(?i)(?:chapter|ch\.?|c|#)\s*(\d+(?:\.\d+)?)").unwrap(),
        }
    }

    /// Extract volume identifier from folder name
    fn extract_volume(&self, folder_name: &str) -> Option<String> {
        if let Some(caps) = self.volume_pattern.captures(folder_name) {
            Some(format!("Vol. {}", caps.get(1).unwrap().as_str()))
        } else {
            // Use folder name as-is if no pattern match
            Some(folder_name.to_string())
        }
    }

    /// Extract chapter number from filename
    fn extract_chapter_number(&self, filename: &str) -> Option<f32> {
        if let Some(caps) = self.chapter_pattern.captures(filename) {
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
}

impl Default for SeriesVolumeChapterStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl ScanningStrategyImpl for SeriesVolumeChapterStrategy {
    fn strategy_type(&self) -> SeriesStrategy {
        SeriesStrategy::SeriesVolumeChapter
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
            let series_path = file_path
                .parent()
                .and_then(|p| p.strip_prefix(library_path).ok())
                .map(|p| p.to_string_lossy().to_string());

            let series = series_map
                .entry(series_name.clone())
                .or_insert_with(|| DetectedSeries::new(&series_name));

            // Set series path if not already set
            if series.path.is_none() && series_name != "Unsorted" {
                series.path = series_path;
            }

            let mut book = DetectedBook::new(file_path.clone());

            // Extract volume info from the second-to-last folder (grandparent of file)
            // e.g., /library/part1/part2/volume_folder/series_folder/file.cbz
            // -> volume_folder is grandparent, series_folder is parent
            if let Some(parent) = file_path.parent() {
                if let Some(grandparent) = parent.parent() {
                    // Only extract volume if grandparent is not the library root
                    if grandparent != library_path {
                        if let Some(volume_folder_name) = grandparent.file_name() {
                            let volume_str = volume_folder_name.to_string_lossy();
                            book.volume = self.extract_volume(&volume_str);
                            book.relative_path = Some(volume_str.to_string());
                        }
                    }
                }
            }

            // Extract chapter number from filename
            if let Some(filename) = file_path.file_name() {
                book.number = self.extract_chapter_number(&filename.to_string_lossy());
            }

            series.add_book(book);
        }

        Ok(series_map)
    }

    fn extract_series_name(&self, file_path: &Path, library_path: &Path) -> String {
        // Get the immediate parent folder (containing folder) as the series name
        if let Some(parent) = file_path.parent() {
            // Check if parent is the library root
            if parent == library_path {
                return "Unsorted".to_string();
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

    fn strategy() -> SeriesVolumeChapterStrategy {
        SeriesVolumeChapterStrategy::new()
    }

    #[test]
    fn test_extract_series_name_from_chapter_folder() {
        let library = Path::new("/library");
        let strategy = strategy();

        // Immediate parent folder is the series name
        // /library/Volume 01/Chapter 001/file.cbz -> series = "Chapter 001"
        let path = PathBuf::from("/library/Volume 01/Chapter 001/file.cbz");
        assert_eq!(strategy.extract_series_name(&path, library), "Chapter 001");
    }

    #[test]
    fn test_extract_series_name_nested_deeply() {
        let library = Path::new("/library");
        let strategy = strategy();

        // Deeply nested: immediate parent folder is the series
        let path = PathBuf::from("/library/Manga/One Piece/Volume 01/Extras/file.cbz");
        assert_eq!(strategy.extract_series_name(&path, library), "Extras");
    }

    #[test]
    fn test_extract_series_name_direct_in_folder() {
        let library = Path::new("/library");
        let strategy = strategy();

        // File directly in a folder (one level deep)
        let path = PathBuf::from("/library/One Piece/Chapter 001.cbz");
        assert_eq!(strategy.extract_series_name(&path, library), "One Piece");
    }

    #[test]
    fn test_extract_series_name_library_root() {
        let library = Path::new("/library");
        let strategy = strategy();

        let path = PathBuf::from("/library/standalone.cbz");
        assert_eq!(strategy.extract_series_name(&path, library), "Unsorted");
    }

    #[test]
    fn test_extract_volume_pattern() {
        let strategy = strategy();

        assert_eq!(
            strategy.extract_volume("Volume 01"),
            Some("Vol. 01".to_string())
        );
        assert_eq!(
            strategy.extract_volume("Vol. 1"),
            Some("Vol. 1".to_string())
        );
        assert_eq!(strategy.extract_volume("v02"), Some("Vol. 02".to_string()));
        assert_eq!(
            strategy.extract_volume("Extras"),
            Some("Extras".to_string())
        );
    }

    #[test]
    fn test_extract_chapter_number() {
        let strategy = strategy();

        assert_eq!(
            strategy.extract_chapter_number("Chapter 001.cbz"),
            Some(1.0)
        );
        assert_eq!(strategy.extract_chapter_number("Ch. 15.5.cbz"), Some(15.5));
        assert_eq!(strategy.extract_chapter_number("#42.cbz"), Some(42.0));
        assert_eq!(
            strategy.extract_chapter_number("One Piece 500.cbz"),
            Some(500.0)
        );
    }

    #[test]
    fn test_organize_files_volume_chapter() {
        let library = Path::new("/library");
        let strategy = strategy();

        // Structure: /library/Volume XX/Chapter YY/file.cbz
        // - Immediate parent (Chapter YY) = series
        // - Grandparent (Volume XX) = volume
        let files = vec![
            PathBuf::from("/library/Volume 01/Chapter 001/file.cbz"),
            PathBuf::from("/library/Volume 01/Chapter 002/file.cbz"),
            PathBuf::from("/library/Volume 02/Chapter 010/file.cbz"),
        ];

        let result = strategy.organize_files(&files, library).unwrap();

        // Each chapter folder is a separate series
        assert_eq!(result.len(), 3);
        assert!(result.contains_key("Chapter 001"));
        assert!(result.contains_key("Chapter 002"));
        assert!(result.contains_key("Chapter 010"));

        // Each series should have volume info from grandparent
        assert_eq!(
            result["Chapter 001"].books[0].volume,
            Some("Vol. 01".to_string())
        );
        assert_eq!(
            result["Chapter 010"].books[0].volume,
            Some("Vol. 02".to_string())
        );
    }

    #[test]
    fn test_organize_files_preserves_full_series_path() {
        let library = Path::new("/library");
        let strategy = strategy();

        let files = vec![PathBuf::from(
            "/library/Manga/One Piece/Volume 01/Chapter 001/file.cbz",
        )];

        let result = strategy.organize_files(&files, library).unwrap();

        // Series name is immediate parent
        assert!(result.contains_key("Chapter 001"));
        // Series path should be the full relative path to the series folder
        assert_eq!(
            result["Chapter 001"].path,
            Some("Manga/One Piece/Volume 01/Chapter 001".to_string())
        );
        // Volume should be extracted from grandparent
        assert_eq!(
            result["Chapter 001"].books[0].volume,
            Some("Vol. 01".to_string())
        );
    }

    #[test]
    fn test_organize_files_no_volume_when_shallow() {
        let library = Path::new("/library");
        let strategy = strategy();

        // File directly in a folder (only one level deep)
        // No grandparent above library root, so no volume
        let files = vec![PathBuf::from("/library/One Piece/Oneshot.cbz")];

        let result = strategy.organize_files(&files, library).unwrap();

        assert_eq!(result.len(), 1);
        let one_piece = &result["One Piece"];
        assert_eq!(one_piece.books.len(), 1);

        // No volume info (grandparent is library root)
        assert!(one_piece.books[0].volume.is_none());
    }

    #[test]
    fn test_strategy_type() {
        assert_eq!(
            strategy().strategy_type(),
            SeriesStrategy::SeriesVolumeChapter
        );
    }
}
