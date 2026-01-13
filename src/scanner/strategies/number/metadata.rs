//! Metadata number strategy
//!
//! Uses ComicInfo <Number> field only, no fallback.
//! Returns None if no metadata number is available.

use crate::models::NumberStrategy;

use super::{BookNumberStrategy, NumberContext, NumberMetadata};

/// Use ComicInfo/metadata number field only, no fallback
pub struct MetadataStrategy;

impl MetadataStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MetadataStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl BookNumberStrategy for MetadataStrategy {
    fn strategy_type(&self) -> NumberStrategy {
        NumberStrategy::Metadata
    }

    fn resolve_number(
        &self,
        _file_name: &str,
        metadata: Option<&NumberMetadata>,
        _context: &NumberContext,
    ) -> Option<f32> {
        // Only use metadata number, no fallback
        metadata.and_then(|m| m.number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(position: usize, total: usize) -> NumberContext {
        NumberContext::new(position, total)
    }

    #[test]
    fn test_uses_metadata_number() {
        let strategy = MetadataStrategy::new();
        let ctx = make_context(1, 10);
        let metadata = NumberMetadata { number: Some(42.0) };

        let number = strategy.resolve_number("book.cbz", Some(&metadata), &ctx);
        assert_eq!(number, Some(42.0));
    }

    #[test]
    fn test_fractional_number() {
        let strategy = MetadataStrategy::new();
        let ctx = make_context(1, 10);
        let metadata = NumberMetadata { number: Some(1.5) };

        let number = strategy.resolve_number("book.cbz", Some(&metadata), &ctx);
        assert_eq!(number, Some(1.5));
    }

    #[test]
    fn test_no_metadata_returns_none() {
        let strategy = MetadataStrategy::new();
        let ctx = make_context(1, 10);

        let number = strategy.resolve_number("book.cbz", None, &ctx);
        assert_eq!(number, None);
    }

    #[test]
    fn test_metadata_without_number_returns_none() {
        let strategy = MetadataStrategy::new();
        let ctx = make_context(1, 10);
        let metadata = NumberMetadata { number: None };

        let number = strategy.resolve_number("book.cbz", Some(&metadata), &ctx);
        assert_eq!(number, None);
    }

    #[test]
    fn test_ignores_filename_patterns() {
        let strategy = MetadataStrategy::new();
        let ctx = make_context(1, 10);

        // Should not parse number from filename when no metadata
        let number = strategy.resolve_number("Batman #042.cbz", None, &ctx);
        assert_eq!(number, None);
    }

    #[test]
    fn test_ignores_file_order() {
        let strategy = MetadataStrategy::new();
        let ctx = make_context(5, 10);

        // Should not fall back to file order
        let number = strategy.resolve_number("book.cbz", None, &ctx);
        assert_eq!(number, None);
    }

    #[test]
    fn test_strategy_type() {
        assert_eq!(
            MetadataStrategy::new().strategy_type(),
            NumberStrategy::Metadata
        );
    }
}
