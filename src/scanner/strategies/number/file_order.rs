//! File order number strategy
//!
//! Book number = position in alphabetically sorted file list within series.
//! This is the default strategy and matches Komga behavior.

use crate::models::NumberStrategy;

use super::{BookNumberStrategy, NumberContext, NumberMetadata};

/// Use file position in sorted directory listing (default, Komga-compatible)
pub struct FileOrderStrategy;

impl FileOrderStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileOrderStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl BookNumberStrategy for FileOrderStrategy {
    fn strategy_type(&self) -> NumberStrategy {
        NumberStrategy::FileOrder
    }

    fn resolve_number(
        &self,
        _file_name: &str,
        _metadata: Option<&NumberMetadata>,
        context: &NumberContext,
    ) -> Option<f32> {
        // Always use the file order position (1-indexed)
        Some(context.file_order_position as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(position: usize, total: usize) -> NumberContext {
        NumberContext::new(position, total)
    }

    #[test]
    fn test_basic_file_order() {
        let strategy = FileOrderStrategy::new();
        let ctx = make_context(1, 10);

        let number = strategy.resolve_number("anything.cbz", None, &ctx);
        assert_eq!(number, Some(1.0));
    }

    #[test]
    fn test_file_order_middle() {
        let strategy = FileOrderStrategy::new();
        let ctx = make_context(5, 10);

        let number = strategy.resolve_number("book.cbz", None, &ctx);
        assert_eq!(number, Some(5.0));
    }

    #[test]
    fn test_file_order_last() {
        let strategy = FileOrderStrategy::new();
        let ctx = make_context(100, 100);

        let number = strategy.resolve_number("zzz.cbz", None, &ctx);
        assert_eq!(number, Some(100.0));
    }

    #[test]
    fn test_file_order_ignores_metadata() {
        let strategy = FileOrderStrategy::new();
        let ctx = make_context(3, 10);
        let metadata = NumberMetadata { number: Some(42.0) };

        // Should ignore metadata and use position
        let number = strategy.resolve_number("book.cbz", Some(&metadata), &ctx);
        assert_eq!(number, Some(3.0));
    }

    #[test]
    fn test_file_order_ignores_filename() {
        let strategy = FileOrderStrategy::new();
        let ctx = make_context(7, 10);

        // Should ignore any number patterns in filename
        let number = strategy.resolve_number("Batman #042.cbz", None, &ctx);
        assert_eq!(number, Some(7.0));
    }

    #[test]
    fn test_strategy_type() {
        assert_eq!(
            FileOrderStrategy::new().strategy_type(),
            NumberStrategy::FileOrder
        );
    }
}
