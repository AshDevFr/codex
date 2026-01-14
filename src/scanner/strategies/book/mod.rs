//! Book naming strategy implementations
//!
//! Book naming strategies determine how individual book titles are resolved
//! from filesystem paths and metadata.
//!
//! TODO: Remove allow(dead_code) once all book strategy features are fully integrated

#![allow(dead_code)]

mod custom;
mod filename;
mod metadata_first;
mod series_name;
mod smart;

pub use custom::CustomStrategy;
pub use filename::FilenameStrategy;
pub use metadata_first::MetadataFirstStrategy;
pub use series_name::SeriesNameStrategy;
pub use smart::SmartStrategy;

use crate::models::{BookStrategy, CustomBookConfig, SmartBookConfig};

/// Context for resolving book titles
#[derive(Debug, Clone)]
pub struct BookNamingContext {
    /// Series name (for SeriesName strategy)
    pub series_name: String,
    /// Book number within series (if detected)
    pub book_number: Option<f32>,
    /// Volume name/number (for series_volume_chapter)
    pub volume: Option<String>,
    /// Chapter number (for series_volume_chapter)
    pub chapter_number: Option<f32>,
    /// Total book count in series (for padding calculation)
    pub total_books: usize,
}

/// Metadata that may contain a title
#[derive(Debug, Clone, Default)]
pub struct BookMetadata {
    /// Title from ComicInfo.xml or embedded metadata
    pub title: Option<String>,
    /// Number from metadata
    pub number: Option<f32>,
}

/// Trait for book naming strategy implementations
pub trait BookNamingStrategy: Send + Sync {
    /// Get the strategy type
    fn strategy_type(&self) -> BookStrategy;

    /// Resolve the book title
    fn resolve_title(
        &self,
        file_name: &str,
        metadata: Option<&BookMetadata>,
        context: &BookNamingContext,
    ) -> String;
}

/// Remove file extension from filename
pub fn filename_without_extension(file_name: &str) -> String {
    if let Some(pos) = file_name.rfind('.') {
        file_name[..pos].to_string()
    } else {
        file_name.to_string()
    }
}

/// Create a book naming strategy from configuration
pub fn create_book_strategy(
    strategy: BookStrategy,
    config: Option<&str>,
) -> Box<dyn BookNamingStrategy> {
    match strategy {
        BookStrategy::Filename => Box::new(FilenameStrategy::new()),
        BookStrategy::MetadataFirst => Box::new(MetadataFirstStrategy::new()),
        BookStrategy::Smart => {
            let smart_config: SmartBookConfig = config
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or_default();
            Box::new(SmartStrategy::new(smart_config))
        }
        BookStrategy::SeriesName => Box::new(SeriesNameStrategy::new()),
        BookStrategy::Custom => {
            let custom_config: CustomBookConfig = config
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or_default();
            Box::new(CustomStrategy::new(custom_config))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filename_without_extension_basic() {
        assert_eq!(filename_without_extension("test.cbz"), "test");
    }

    #[test]
    fn test_filename_without_extension_multiple_dots() {
        assert_eq!(filename_without_extension("test.vol.1.cbz"), "test.vol.1");
    }

    #[test]
    fn test_filename_without_extension_no_ext() {
        assert_eq!(filename_without_extension("noext"), "noext");
    }

    #[test]
    fn test_create_filename_strategy() {
        let strategy = create_book_strategy(BookStrategy::Filename, None);
        assert_eq!(strategy.strategy_type(), BookStrategy::Filename);
    }

    #[test]
    fn test_create_metadata_first_strategy() {
        let strategy = create_book_strategy(BookStrategy::MetadataFirst, None);
        assert_eq!(strategy.strategy_type(), BookStrategy::MetadataFirst);
    }

    #[test]
    fn test_create_smart_strategy() {
        let strategy = create_book_strategy(BookStrategy::Smart, None);
        assert_eq!(strategy.strategy_type(), BookStrategy::Smart);
    }

    #[test]
    fn test_create_smart_strategy_with_config() {
        let config = r#"{"additionalGenericPatterns":["^Test\\d+$"]}"#;
        let strategy = create_book_strategy(BookStrategy::Smart, Some(config));
        assert_eq!(strategy.strategy_type(), BookStrategy::Smart);
    }

    #[test]
    fn test_create_series_name_strategy() {
        let strategy = create_book_strategy(BookStrategy::SeriesName, None);
        assert_eq!(strategy.strategy_type(), BookStrategy::SeriesName);
    }

    #[test]
    fn test_create_custom_strategy() {
        let strategy = create_book_strategy(BookStrategy::Custom, None);
        assert_eq!(strategy.strategy_type(), BookStrategy::Custom);
    }

    #[test]
    fn test_create_custom_strategy_with_config() {
        let config = r#"{"pattern":"(?P<series>.+?)_v(?P<volume>\\d+)","titleTemplate":"{series} v.{volume}","fallback":"filename"}"#;
        let strategy = create_book_strategy(BookStrategy::Custom, Some(config));
        assert_eq!(strategy.strategy_type(), BookStrategy::Custom);
    }
}
