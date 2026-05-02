//! Book metadata strategy implementations
//!
//! Book metadata strategies determine how per-book facts (title, volume,
//! chapter) are resolved from filesystem paths and embedded metadata. The
//! framing is "which sources do we trust for facts about this book"; title is
//! just the first fact we extract. Volume and chapter classification (Phase 11
//! of metadata-count-split) live on the same trait, mirroring the title flow.
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

/// Context for resolving book metadata
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

/// Metadata that may contribute facts about a book (title, volume, chapter).
///
/// Mirrors the fields on `book_metadata` that the per-book classification cares
/// about. Strategies use this as their "ComicInfo says X" input shape.
#[derive(Debug, Clone, Default)]
pub struct BookMetadata {
    /// Title from ComicInfo.xml or embedded metadata
    pub title: Option<String>,
    /// Number from metadata (legacy single-axis identifier)
    pub number: Option<f32>,
    /// Volume number from metadata (e.g. ComicInfo `<Volume>`)
    pub volume: Option<i32>,
    /// Chapter number from metadata (fractional, e.g. side chapters at 47.5)
    pub chapter: Option<f32>,
}

/// Trait for per-book metadata strategy implementations.
///
/// Renamed from `BookNamingStrategy` (Phase 11 of metadata-count-split): the
/// trait is no longer purely about titles. It now resolves three independent
/// facts about a book — title, volume, chapter — from the same trio of inputs
/// (filename, metadata, context). Strategies remain free to implement only the
/// methods they have meaningful answers for; the rest return `None` defaults.
pub trait BookMetadataStrategy: Send + Sync {
    /// Get the strategy type
    fn strategy_type(&self) -> BookStrategy;

    /// Resolve the book title
    fn resolve_title(
        &self,
        file_name: &str,
        metadata: Option<&BookMetadata>,
        context: &BookNamingContext,
    ) -> String;

    /// Resolve the volume number for this book, if known.
    ///
    /// Default: `None`. Strategies that have a meaningful answer (filename
    /// regex, ComicInfo `<Volume>`, etc.) override this.
    fn resolve_volume(
        &self,
        _file_name: &str,
        _metadata: Option<&BookMetadata>,
        _context: &BookNamingContext,
    ) -> Option<i32> {
        None
    }

    /// Resolve the chapter number for this book, if known.
    ///
    /// Default: `None`. Fractional chapters (e.g. 47.5 side chapters) are
    /// preserved via `f32`; integer chapters parse cleanly into the same type.
    fn resolve_chapter(
        &self,
        _file_name: &str,
        _metadata: Option<&BookMetadata>,
        _context: &BookNamingContext,
    ) -> Option<f32> {
        None
    }
}

/// Backwards-compat alias. Prefer `BookMetadataStrategy` in new code; the
/// `BookNamingStrategy` name is kept as a re-export for now to keep the
/// downstream cascade narrow during Phase 11. Remove in a follow-up once all
/// call sites are updated.
pub use self::BookMetadataStrategy as BookNamingStrategy;

/// Remove file extension from filename
pub fn filename_without_extension(file_name: &str) -> String {
    if let Some(pos) = file_name.rfind('.') {
        file_name[..pos].to_string()
    } else {
        file_name.to_string()
    }
}

/// Create a book metadata strategy from configuration
pub fn create_book_strategy(
    strategy: BookStrategy,
    config: Option<&str>,
) -> Box<dyn BookMetadataStrategy> {
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
