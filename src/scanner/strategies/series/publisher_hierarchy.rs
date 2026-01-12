//! Publisher Hierarchy scanning strategy
//!
//! Skip first N levels as organizational containers, then apply series_volume rules:
//! - /library/Marvel/Spider-Man/issue1.cbz → Series: "Spider-Man" (skip "Marvel")
//! - /library/DC/2024/Batman/issue1.cbz → Series: "Batman" (skip "DC/2024" with skip_depth=2)

use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::models::{PublisherHierarchyConfig, SeriesStrategy};

use super::super::common::{DetectedBook, DetectedSeries, SeriesMetadata};
use super::ScanningStrategyImpl;

/// Publisher Hierarchy strategy implementation
///
/// Skip organizational levels (publisher/year) before series detection
pub struct PublisherHierarchyStrategy {
    config: PublisherHierarchyConfig,
}

impl PublisherHierarchyStrategy {
    pub fn new(config: PublisherHierarchyConfig) -> Self {
        Self { config }
    }

    /// Extract skipped folder names (e.g., publisher, year)
    fn extract_skipped_levels(&self, file_path: &Path, library_path: &Path) -> Vec<String> {
        let relative = file_path.strip_prefix(library_path).unwrap_or(file_path);
        let components: Vec<_> = relative.components().collect();

        let skip = self.config.skip_depth as usize;
        components
            .iter()
            .take(skip)
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect()
    }
}

impl ScanningStrategyImpl for PublisherHierarchyStrategy {
    fn strategy_type(&self) -> SeriesStrategy {
        SeriesStrategy::PublisherHierarchy
    }

    fn organize_files(
        &self,
        files: &[PathBuf],
        library_path: &Path,
    ) -> Result<HashMap<String, DetectedSeries>> {
        let mut series_map: HashMap<String, DetectedSeries> = HashMap::new();

        for file_path in files {
            let series_name = self.extract_series_name(file_path, library_path);
            let skipped_levels = self.extract_skipped_levels(file_path, library_path);

            let series = series_map.entry(series_name.clone()).or_insert_with(|| {
                let mut s = DetectedSeries::new(&series_name);

                // Store skipped levels as metadata
                if let Some(field_name) = &self.config.store_skipped_as {
                    if !skipped_levels.is_empty() {
                        s.metadata = SeriesMetadata {
                            publisher: if field_name == "publisher" {
                                Some(skipped_levels.join("/"))
                            } else {
                                None
                            },
                            extra: [(field_name.clone(), skipped_levels.join("/"))]
                                .into_iter()
                                .collect(),
                            ..Default::default()
                        };
                    }
                }

                s
            });

            // Set series path if not already set (relative to library, after skipped levels)
            if series.path.is_none() && series_name != "Unsorted" {
                let relative = file_path.strip_prefix(library_path).unwrap_or(file_path);
                let components: Vec<_> = relative.components().collect();
                let skip = self.config.skip_depth as usize;

                if components.len() > skip {
                    let path_components: Vec<String> = components
                        .iter()
                        .take(skip + 1)
                        .map(|c| c.as_os_str().to_string_lossy().to_string())
                        .collect();
                    series.path = Some(path_components.join("/"));
                }
            }

            series.add_book(DetectedBook::new(file_path.clone()));
        }

        Ok(series_map)
    }

    fn extract_series_name(&self, file_path: &Path, library_path: &Path) -> String {
        let relative = file_path.strip_prefix(library_path).unwrap_or(file_path);
        let components: Vec<_> = relative.components().collect();

        let skip = self.config.skip_depth as usize;

        // After skipping N levels, the next folder is the series
        if components.len() > skip + 1 {
            // File is in a series folder (after skipped levels)
            components[skip].as_os_str().to_string_lossy().to_string()
        } else if components.len() == skip + 1 {
            // File is directly in a skipped folder (no series subfolder)
            "Unsorted".to_string()
        } else {
            // File is at a level above what we expect
            "Unsorted".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strategy() -> PublisherHierarchyStrategy {
        PublisherHierarchyStrategy::new(PublisherHierarchyConfig::default())
    }

    fn strategy_with_depth(depth: u32) -> PublisherHierarchyStrategy {
        PublisherHierarchyStrategy::new(PublisherHierarchyConfig {
            skip_depth: depth,
            store_skipped_as: Some("publisher".to_string()),
        })
    }

    #[test]
    fn test_extract_series_name_skip_one() {
        let library = Path::new("/library");
        let strategy = strategy(); // Default skip_depth = 1

        // Skip publisher, use series folder
        let path = PathBuf::from("/library/Marvel/Spider-Man/issue1.cbz");
        assert_eq!(strategy.extract_series_name(&path, library), "Spider-Man");
    }

    #[test]
    fn test_extract_series_name_skip_two() {
        let library = Path::new("/library");
        let strategy = strategy_with_depth(2);

        // Skip publisher and year, use series folder
        let path = PathBuf::from("/library/DC/2024/Batman/issue1.cbz");
        assert_eq!(strategy.extract_series_name(&path, library), "Batman");
    }

    #[test]
    fn test_extract_series_name_not_enough_depth() {
        let library = Path::new("/library");
        let strategy = strategy();

        // File directly in publisher folder (no series subfolder)
        let path = PathBuf::from("/library/Marvel/standalone.cbz");
        assert_eq!(strategy.extract_series_name(&path, library), "Unsorted");
    }

    #[test]
    fn test_extract_skipped_levels() {
        let library = Path::new("/library");
        let strategy = strategy_with_depth(2);

        let path = PathBuf::from("/library/DC/2024/Batman/issue1.cbz");
        let skipped = strategy.extract_skipped_levels(&path, library);

        assert_eq!(skipped, vec!["DC", "2024"]);
    }

    #[test]
    fn test_organize_files_with_publisher_metadata() {
        let library = Path::new("/library");
        let strategy = strategy();

        let files = vec![
            PathBuf::from("/library/Marvel/Spider-Man/issue1.cbz"),
            PathBuf::from("/library/Marvel/Spider-Man/issue2.cbz"),
            PathBuf::from("/library/DC/Batman/issue1.cbz"),
        ];

        let result = strategy.organize_files(&files, library).unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.contains_key("Spider-Man"));
        assert!(result.contains_key("Batman"));

        // Check publisher metadata
        assert_eq!(
            result["Spider-Man"].metadata.publisher,
            Some("Marvel".to_string())
        );
        assert_eq!(result["Batman"].metadata.publisher, Some("DC".to_string()));
    }

    #[test]
    fn test_organize_files_skip_two() {
        let library = Path::new("/library");
        let strategy = strategy_with_depth(2);

        let files = vec![
            PathBuf::from("/library/DC/2024/Batman/issue1.cbz"),
            PathBuf::from("/library/DC/2024/Superman/issue1.cbz"),
            PathBuf::from("/library/Marvel/2023/Avengers/issue1.cbz"),
        ];

        let result = strategy.organize_files(&files, library).unwrap();

        assert_eq!(result.len(), 3);
        assert!(result.contains_key("Batman"));
        assert!(result.contains_key("Superman"));
        assert!(result.contains_key("Avengers"));

        // Check publisher metadata includes both skipped levels
        assert_eq!(
            result["Batman"].metadata.publisher,
            Some("DC/2024".to_string())
        );
        assert_eq!(
            result["Avengers"].metadata.publisher,
            Some("Marvel/2023".to_string())
        );
    }

    #[test]
    fn test_series_path_includes_skipped_levels() {
        let library = Path::new("/library");
        let strategy = strategy();

        let files = vec![PathBuf::from("/library/Marvel/Spider-Man/issue1.cbz")];

        let result = strategy.organize_files(&files, library).unwrap();

        // Path should include the full path from library root
        assert_eq!(
            result["Spider-Man"].path,
            Some("Marvel/Spider-Man".to_string())
        );
    }

    #[test]
    fn test_strategy_type() {
        assert_eq!(
            strategy().strategy_type(),
            SeriesStrategy::PublisherHierarchy
        );
    }
}
