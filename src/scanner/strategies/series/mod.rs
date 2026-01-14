//! Series detection strategy implementations
//!
//! Series strategies determine how files are organized into series based on
//! the library's folder structure, metadata, or custom patterns.
//!
//! TODO: Remove allow(dead_code) once all series strategy features are fully integrated

#![allow(dead_code)]

mod calibre;
mod custom;
mod flat;
mod publisher_hierarchy;
mod series_volume;
mod series_volume_chapter;

pub use calibre::CalibreStrategy;
pub use custom::CustomStrategy;
pub use flat::FlatStrategy;
pub use publisher_hierarchy::PublisherHierarchyStrategy;
pub use series_volume::SeriesVolumeStrategy;
pub use series_volume_chapter::SeriesVolumeChapterStrategy;

use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::models::{
    CalibreStrategyConfig, CustomStrategyConfig, FlatStrategyConfig, PublisherHierarchyConfig,
    SeriesStrategy,
};

use super::common::DetectedSeries;

/// Trait for implementing scanning strategies
///
/// Each strategy must implement how to organize files into series based on
/// the library's folder structure or file metadata.
pub trait ScanningStrategyImpl: Send + Sync {
    /// Get the strategy type
    fn strategy_type(&self) -> SeriesStrategy;

    /// Organize discovered files into series
    ///
    /// Takes a list of file paths (already filtered for supported formats)
    /// and returns a mapping of series name to detected series information.
    fn organize_files(
        &self,
        files: &[PathBuf],
        library_path: &Path,
    ) -> Result<HashMap<String, DetectedSeries>>;

    /// Extract series name from a file path
    ///
    /// Returns the series name for the given file based on this strategy's rules.
    fn extract_series_name(&self, file_path: &Path, library_path: &Path) -> String;

    /// Validate if this strategy can be used with the given library path
    ///
    /// Returns Ok(()) if valid, or an error describing why the path is not compatible.
    fn validate(&self, library_path: &Path) -> Result<()> {
        if !library_path.exists() {
            anyhow::bail!("Library path does not exist: {}", library_path.display());
        }
        if !library_path.is_dir() {
            anyhow::bail!(
                "Library path is not a directory: {}",
                library_path.display()
            );
        }
        Ok(())
    }
}

/// Create a strategy implementation from configuration
pub fn create_strategy(
    strategy: SeriesStrategy,
    config: Option<&str>,
) -> Result<Box<dyn ScanningStrategyImpl>> {
    match strategy {
        SeriesStrategy::SeriesVolume => Ok(Box::new(SeriesVolumeStrategy::new())),

        SeriesStrategy::SeriesVolumeChapter => Ok(Box::new(SeriesVolumeChapterStrategy::new())),

        SeriesStrategy::Flat => {
            let flat_config: FlatStrategyConfig = if let Some(json) = config {
                serde_json::from_str(json)?
            } else {
                FlatStrategyConfig::default()
            };
            Ok(Box::new(FlatStrategy::new(flat_config)))
        }

        SeriesStrategy::PublisherHierarchy => {
            let pub_config: PublisherHierarchyConfig = if let Some(json) = config {
                serde_json::from_str(json)?
            } else {
                PublisherHierarchyConfig::default()
            };
            Ok(Box::new(PublisherHierarchyStrategy::new(pub_config)))
        }

        SeriesStrategy::Calibre => {
            let cal_config: CalibreStrategyConfig = if let Some(json) = config {
                serde_json::from_str(json)?
            } else {
                CalibreStrategyConfig::default()
            };
            Ok(Box::new(CalibreStrategy::new(cal_config)))
        }

        SeriesStrategy::Custom => {
            let custom_config: CustomStrategyConfig = if let Some(json) = config {
                serde_json::from_str(json)?
            } else {
                anyhow::bail!("Custom strategy requires configuration with a pattern");
            };
            Ok(Box::new(CustomStrategy::new(custom_config)?))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_series_volume_strategy() {
        let strategy = create_strategy(SeriesStrategy::SeriesVolume, None).unwrap();
        assert_eq!(strategy.strategy_type(), SeriesStrategy::SeriesVolume);
    }

    #[test]
    fn test_create_series_volume_chapter_strategy() {
        let strategy = create_strategy(SeriesStrategy::SeriesVolumeChapter, None).unwrap();
        assert_eq!(
            strategy.strategy_type(),
            SeriesStrategy::SeriesVolumeChapter
        );
    }

    #[test]
    fn test_create_flat_strategy_default() {
        let strategy = create_strategy(SeriesStrategy::Flat, None).unwrap();
        assert_eq!(strategy.strategy_type(), SeriesStrategy::Flat);
    }

    #[test]
    fn test_create_flat_strategy_with_config() {
        let config = r#"{"filenamePatterns":["\\[([^\\]]+)\\]"]}"#;
        let strategy = create_strategy(SeriesStrategy::Flat, Some(config)).unwrap();
        assert_eq!(strategy.strategy_type(), SeriesStrategy::Flat);
    }

    #[test]
    fn test_create_publisher_hierarchy_strategy() {
        let strategy = create_strategy(SeriesStrategy::PublisherHierarchy, None).unwrap();
        assert_eq!(strategy.strategy_type(), SeriesStrategy::PublisherHierarchy);
    }

    #[test]
    fn test_create_calibre_strategy() {
        let strategy = create_strategy(SeriesStrategy::Calibre, None).unwrap();
        assert_eq!(strategy.strategy_type(), SeriesStrategy::Calibre);
    }

    #[test]
    fn test_create_custom_strategy_requires_config() {
        let result = create_strategy(SeriesStrategy::Custom, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_custom_strategy_with_config() {
        let config = r#"{"pattern":"^(?P<series>[^/]+)/(?P<book>.+)\\.(cbz|cbr)$"}"#;
        let strategy = create_strategy(SeriesStrategy::Custom, Some(config)).unwrap();
        assert_eq!(strategy.strategy_type(), SeriesStrategy::Custom);
    }
}
