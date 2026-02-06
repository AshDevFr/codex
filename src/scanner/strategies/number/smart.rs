//! Smart number strategy
//!
//! Fallback chain: metadata → filename patterns → file order.
//! This provides the best coverage by using the best available information.

use crate::models::NumberStrategy;

use super::{BookNumberStrategy, NumberContext, NumberMetadata, filename::FilenameStrategy};

/// Smart fallback: metadata → filename → file order
pub struct SmartStrategy;

impl SmartStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SmartStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl BookNumberStrategy for SmartStrategy {
    fn strategy_type(&self) -> NumberStrategy {
        NumberStrategy::Smart
    }

    fn resolve_number(
        &self,
        file_name: &str,
        metadata: Option<&NumberMetadata>,
        context: &NumberContext,
    ) -> Option<f32> {
        // 1. Try metadata first
        if let Some(num) = metadata.and_then(|m| m.number) {
            return Some(num);
        }

        // 2. Try filename parsing
        if let Some(num) = FilenameStrategy::new().resolve_number(file_name, None, context) {
            return Some(num);
        }

        // 3. Fall back to file order
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
    fn test_uses_metadata_first() {
        let strategy = SmartStrategy::new();
        let ctx = make_context(5, 10);
        let metadata = NumberMetadata { number: Some(42.0) };

        // Should use metadata, not filename or position
        let number = strategy.resolve_number("Batman #001.cbz", Some(&metadata), &ctx);
        assert_eq!(number, Some(42.0));
    }

    #[test]
    fn test_falls_back_to_filename() {
        let strategy = SmartStrategy::new();
        let ctx = make_context(5, 10);

        // No metadata, should parse from filename
        let number = strategy.resolve_number("Batman #042.cbz", None, &ctx);
        assert_eq!(number, Some(42.0));
    }

    #[test]
    fn test_falls_back_to_file_order() {
        let strategy = SmartStrategy::new();
        let ctx = make_context(7, 10);

        // No metadata, no parseable number in filename
        let number = strategy.resolve_number("Just A Title.cbz", None, &ctx);
        assert_eq!(number, Some(7.0)); // Position
    }

    #[test]
    fn test_metadata_without_number_falls_back() {
        let strategy = SmartStrategy::new();
        let ctx = make_context(3, 10);
        let metadata = NumberMetadata { number: None };

        // Metadata exists but has no number, try filename
        let number = strategy.resolve_number("Comic #5.cbz", Some(&metadata), &ctx);
        assert_eq!(number, Some(5.0));
    }

    #[test]
    fn test_full_fallback_chain() {
        let strategy = SmartStrategy::new();
        let ctx = make_context(9, 10);
        let metadata = NumberMetadata { number: None };

        // Metadata without number, filename without pattern -> file order
        let number = strategy.resolve_number("Random Name.cbz", Some(&metadata), &ctx);
        assert_eq!(number, Some(9.0));
    }

    #[test]
    fn test_always_returns_some() {
        let strategy = SmartStrategy::new();
        let ctx = make_context(1, 10);

        // Smart strategy should always return Some due to file_order fallback
        assert!(
            strategy
                .resolve_number("anything.cbz", None, &ctx)
                .is_some()
        );
    }

    #[test]
    fn test_real_world_example() {
        let strategy = SmartStrategy::new();
        let ctx = make_context(1, 100);

        // Real-world filename from the original issue
        let number = strategy.resolve_number(
            "A Returner's Magic Should Be Special c001 (2021) (Digital) (4str0).cbz",
            None,
            &ctx,
        );
        assert_eq!(number, Some(1.0));
    }

    #[test]
    fn test_fractional_metadata() {
        let strategy = SmartStrategy::new();
        let ctx = make_context(1, 10);
        let metadata = NumberMetadata { number: Some(1.5) };

        let number = strategy.resolve_number("book.cbz", Some(&metadata), &ctx);
        assert_eq!(number, Some(1.5));
    }

    #[test]
    fn test_strategy_type() {
        assert_eq!(SmartStrategy::new().strategy_type(), NumberStrategy::Smart);
    }
}
