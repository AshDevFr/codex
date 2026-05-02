//! MetadataFirst book naming strategy
//!
//! Uses metadata title if present, falls back to filename

use crate::models::BookStrategy;

use super::{BookMetadata, BookNamingContext, BookNamingStrategy, filename_without_extension};

/// Use metadata title if present, fallback to filename
pub struct MetadataFirstStrategy;

impl MetadataFirstStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MetadataFirstStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl BookNamingStrategy for MetadataFirstStrategy {
    fn strategy_type(&self) -> BookStrategy {
        BookStrategy::MetadataFirst
    }

    fn resolve_title(
        &self,
        file_name: &str,
        metadata: Option<&BookMetadata>,
        _context: &BookNamingContext,
    ) -> String {
        metadata
            .and_then(|m| m.title.as_ref())
            .filter(|t| !t.is_empty())
            .cloned()
            .unwrap_or_else(|| filename_without_extension(file_name))
    }

    /// Volume from metadata only. Honors the user's "I picked Metadata, don't
    /// touch the filename" choice; returns `None` if metadata has no volume.
    fn resolve_volume(
        &self,
        _file_name: &str,
        metadata: Option<&BookMetadata>,
        _context: &BookNamingContext,
    ) -> Option<i32> {
        metadata.and_then(|m| m.volume)
    }

    /// Chapter from metadata only.
    fn resolve_chapter(
        &self,
        _file_name: &str,
        metadata: Option<&BookMetadata>,
        _context: &BookNamingContext,
    ) -> Option<f32> {
        metadata.and_then(|m| m.chapter)
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
    fn test_uses_metadata_title() {
        let strategy = MetadataFirstStrategy::new();
        let ctx = default_context();
        let metadata = BookMetadata {
            title: Some("The Dark Knight Returns".to_string()),
            number: Some(1.0),
            volume: None,
            chapter: None,
        };

        let title = strategy.resolve_title("batman-001.cbz", Some(&metadata), &ctx);
        assert_eq!(title, "The Dark Knight Returns");
    }

    #[test]
    fn test_fallback_to_filename() {
        let strategy = MetadataFirstStrategy::new();
        let ctx = default_context();

        let title = strategy.resolve_title("batman-001.cbz", None, &ctx);
        assert_eq!(title, "batman-001");
    }

    #[test]
    fn test_empty_metadata_fallback() {
        let strategy = MetadataFirstStrategy::new();
        let ctx = default_context();
        let metadata = BookMetadata {
            title: Some("".to_string()),
            number: None,
            volume: None,
            chapter: None,
        };

        let title = strategy.resolve_title("batman-001.cbz", Some(&metadata), &ctx);
        assert_eq!(title, "batman-001");
    }

    #[test]
    fn test_none_title_fallback() {
        let strategy = MetadataFirstStrategy::new();
        let ctx = default_context();
        let metadata = BookMetadata {
            title: None,
            number: Some(1.0),
            volume: None,
            chapter: None,
        };

        let title = strategy.resolve_title("batman-001.cbz", Some(&metadata), &ctx);
        assert_eq!(title, "batman-001");
    }

    #[test]
    fn test_strategy_type() {
        assert_eq!(
            MetadataFirstStrategy::new().strategy_type(),
            BookStrategy::MetadataFirst
        );
    }
}
