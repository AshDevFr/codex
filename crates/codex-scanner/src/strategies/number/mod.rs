//! Book number strategy implementations
//!
//! Book number strategies determine how individual book numbers are resolved
//! for sorting and display. Unlike book naming strategies (which determine titles),
//! number strategies determine the numeric ordering within a series.
//!
//! TODO: Remove allow(dead_code) once all number strategy features are fully integrated

#![allow(dead_code)]

mod file_order;
mod filename;
mod metadata;
mod smart;

pub use file_order::FileOrderStrategy;
pub use filename::FilenameStrategy;
pub use metadata::MetadataStrategy;
pub use smart::SmartStrategy;

use codex_models::NumberStrategy;

/// Context for resolving book numbers
#[derive(Debug, Clone)]
pub struct NumberContext {
    /// Position of this book in the sorted file list (1-indexed).
    ///
    /// `None` when the caller has no complete view of the series. A position
    /// is only meaningful against the full set of books, so callers that see
    /// a partial set (per-book analysis, which runs while a scan is still
    /// inserting siblings) must leave this unset rather than hand out a
    /// position two books could end up sharing.
    pub file_order_position: Option<usize>,
    /// Total books in series (for reference)
    pub total_books: usize,
}

impl NumberContext {
    pub fn new(file_order_position: usize, total_books: usize) -> Self {
        Self {
            file_order_position: Some(file_order_position),
            total_books,
        }
    }

    /// Context for a caller that cannot see the whole series, so positional
    /// strategies decline to produce a number instead of guessing one.
    pub fn without_position() -> Self {
        Self {
            file_order_position: None,
            total_books: 0,
        }
    }
}

/// Metadata that may contain a number
#[derive(Debug, Clone, Default)]
pub struct NumberMetadata {
    /// Number from ComicInfo.xml <Number> field
    pub number: Option<f32>,
}

/// Trait for book number strategy implementations
pub trait BookNumberStrategy: Send + Sync {
    /// Get the strategy type
    fn strategy_type(&self) -> NumberStrategy;

    /// Resolve the book number
    ///
    /// Returns `Some(number)` if a number can be determined, `None` otherwise.
    /// Positional strategies return `None` when the context carries no
    /// position, leaving the number for the renumber pass to assign.
    fn resolve_number(
        &self,
        file_name: &str,
        metadata: Option<&NumberMetadata>,
        context: &NumberContext,
    ) -> Option<f32>;
}

/// Create a book number strategy from configuration
pub fn create_number_strategy(
    strategy: NumberStrategy,
    _config: Option<&str>,
) -> Box<dyn BookNumberStrategy> {
    match strategy {
        NumberStrategy::FileOrder => Box::new(FileOrderStrategy::new()),
        NumberStrategy::Metadata => Box::new(MetadataStrategy::new()),
        NumberStrategy::Filename => Box::new(FilenameStrategy::new()),
        NumberStrategy::Smart => Box::new(SmartStrategy::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_file_order_strategy() {
        let strategy = create_number_strategy(NumberStrategy::FileOrder, None);
        assert_eq!(strategy.strategy_type(), NumberStrategy::FileOrder);
    }

    #[test]
    fn test_create_metadata_strategy() {
        let strategy = create_number_strategy(NumberStrategy::Metadata, None);
        assert_eq!(strategy.strategy_type(), NumberStrategy::Metadata);
    }

    #[test]
    fn test_create_filename_strategy() {
        let strategy = create_number_strategy(NumberStrategy::Filename, None);
        assert_eq!(strategy.strategy_type(), NumberStrategy::Filename);
    }

    #[test]
    fn test_create_smart_strategy() {
        let strategy = create_number_strategy(NumberStrategy::Smart, None);
        assert_eq!(strategy.strategy_type(), NumberStrategy::Smart);
    }

    #[test]
    fn test_number_context() {
        let ctx = NumberContext::new(5, 100);
        assert_eq!(ctx.file_order_position, Some(5));
        assert_eq!(ctx.total_books, 100);
    }
}
