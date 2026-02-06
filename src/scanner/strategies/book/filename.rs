//! Filename book naming strategy
//!
//! Always uses the filename without extension (Komga-compatible)

use crate::models::BookStrategy;

use super::{BookMetadata, BookNamingContext, BookNamingStrategy, filename_without_extension};

/// Always use filename without extension (Komga-compatible)
pub struct FilenameStrategy;

impl FilenameStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FilenameStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl BookNamingStrategy for FilenameStrategy {
    fn strategy_type(&self) -> BookStrategy {
        BookStrategy::Filename
    }

    fn resolve_title(
        &self,
        file_name: &str,
        _metadata: Option<&BookMetadata>,
        _context: &BookNamingContext,
    ) -> String {
        filename_without_extension(file_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_context() -> BookNamingContext {
        BookNamingContext {
            series_name: "Test Series".to_string(),
            book_number: None,
            volume: None,
            chapter_number: None,
            total_books: 10,
        }
    }

    #[test]
    fn test_basic_filename() {
        let strategy = FilenameStrategy::new();
        let ctx = default_context();

        let title = strategy.resolve_title("Batman #001.cbz", None, &ctx);
        assert_eq!(title, "Batman #001");
    }

    #[test]
    fn test_filename_with_multiple_dots() {
        let strategy = FilenameStrategy::new();
        let ctx = default_context();

        let title = strategy.resolve_title("Batman Vol. 1 #001.cbz", None, &ctx);
        assert_eq!(title, "Batman Vol. 1 #001");
    }

    #[test]
    fn test_filename_ignores_metadata() {
        let strategy = FilenameStrategy::new();
        let ctx = default_context();
        let metadata = BookMetadata {
            title: Some("Different Title".to_string()),
            number: Some(1.0),
        };

        let title = strategy.resolve_title("Batman #001.cbz", Some(&metadata), &ctx);
        assert_eq!(title, "Batman #001");
    }

    #[test]
    fn test_filename_no_extension() {
        let strategy = FilenameStrategy::new();
        let ctx = default_context();

        let title = strategy.resolve_title("NoExtension", None, &ctx);
        assert_eq!(title, "NoExtension");
    }

    #[test]
    fn test_strategy_type() {
        assert_eq!(
            FilenameStrategy::new().strategy_type(),
            BookStrategy::Filename
        );
    }
}
