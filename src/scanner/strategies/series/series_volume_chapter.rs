//! Series-Volume-Chapter scanning strategy
//!
//! For chapter-based manga and web comics:
//! - Level 1 folders = series
//! - Level 2 folders = volumes/arcs (organizational containers)
//! - All files at any depth under series folder = books in that series

use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::models::SeriesStrategy;

use super::super::common::{DetectedBook, DetectedSeries};
use super::ScanningStrategyImpl;

/// Series-Volume-Chapter strategy implementation
///
/// Parent folder = series, child folders = volumes/arcs
/// All files at any depth under series folder = books in that series
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

            let series = series_map
                .entry(series_name.clone())
                .or_insert_with(|| DetectedSeries::new(&series_name));

            // Set series path if not already set
            if series.path.is_none() && series_name != "Unsorted" {
                series.path = Some(series_name.clone());
            }

            // Extract volume and chapter info
            let relative = file_path.strip_prefix(library_path).unwrap_or(file_path);
            let components: Vec<_> = relative.components().collect();

            let mut book = DetectedBook::new(file_path.clone());

            // If there's a volume folder (depth >= 2), extract volume info
            if components.len() > 2 {
                // Second component is the volume folder
                let volume_folder = components[1].as_os_str().to_string_lossy();
                book.volume = self.extract_volume(&volume_folder);
                book.relative_path = Some(volume_folder.to_string());
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
        // Get relative path from library root
        let relative = file_path.strip_prefix(library_path).unwrap_or(file_path);

        // Get first component (series folder)
        let components: Vec<_> = relative.components().collect();

        if !components.is_empty() {
            // First folder is always the series
            let first = components[0].as_os_str().to_string_lossy().to_string();
            // If file is directly in first folder or deeper, use first folder as series
            if components.len() > 1 {
                first
            } else {
                // File is directly in library root
                "Unsorted".to_string()
            }
        } else {
            "Unsorted".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strategy() -> SeriesVolumeChapterStrategy {
        SeriesVolumeChapterStrategy::new()
    }

    #[test]
    fn test_extract_series_name_from_volume_folder() {
        let library = Path::new("/library");
        let strategy = strategy();

        // File in Volume subfolder should still use parent as series
        let path = PathBuf::from("/library/One Piece/Volume 01/Chapter 001.cbz");
        assert_eq!(strategy.extract_series_name(&path, library), "One Piece");
    }

    #[test]
    fn test_extract_series_name_nested_deeply() {
        let library = Path::new("/library");
        let strategy = strategy();

        // Deeply nested files still use first folder as series
        let path = PathBuf::from("/library/One Piece/Volume 01/Extras/Cover.cbz");
        assert_eq!(strategy.extract_series_name(&path, library), "One Piece");
    }

    #[test]
    fn test_extract_series_name_direct_in_series() {
        let library = Path::new("/library");
        let strategy = strategy();

        // File directly in series folder (no volume subfolder)
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

        let files = vec![
            PathBuf::from("/library/One Piece/Volume 01/Chapter 001.cbz"),
            PathBuf::from("/library/One Piece/Volume 01/Chapter 002.cbz"),
            PathBuf::from("/library/One Piece/Volume 02/Chapter 010.cbz"),
            PathBuf::from("/library/Naruto/Volume 01/Chapter 001.cbz"),
        ];

        let result = strategy.organize_files(&files, library).unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.contains_key("One Piece"));
        assert!(result.contains_key("Naruto"));

        let one_piece = &result["One Piece"];
        assert_eq!(one_piece.books.len(), 3);
        assert_eq!(one_piece.books[0].volume, Some("Vol. 01".to_string()));
        assert_eq!(one_piece.books[0].number, Some(1.0));
        assert_eq!(one_piece.books[2].volume, Some("Vol. 02".to_string()));
        assert_eq!(one_piece.books[2].number, Some(10.0));
    }

    #[test]
    fn test_organize_files_mixed_structure() {
        let library = Path::new("/library");
        let strategy = strategy();

        let files = vec![
            // With volume folder
            PathBuf::from("/library/One Piece/Volume 01/Chapter 001.cbz"),
            // Without volume folder (directly in series)
            PathBuf::from("/library/One Piece/Oneshot.cbz"),
        ];

        let result = strategy.organize_files(&files, library).unwrap();

        assert_eq!(result.len(), 1);
        let one_piece = &result["One Piece"];
        assert_eq!(one_piece.books.len(), 2);

        // First book has volume info
        assert!(one_piece.books[0].volume.is_some());
        // Second book doesn't have volume info (directly in series folder)
        assert!(one_piece.books[1].volume.is_none());
    }

    #[test]
    fn test_strategy_type() {
        assert_eq!(
            strategy().strategy_type(),
            SeriesStrategy::SeriesVolumeChapter
        );
    }
}
