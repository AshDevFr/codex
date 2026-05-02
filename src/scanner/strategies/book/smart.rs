//! Smart book naming strategy
//!
//! Uses metadata only if meaningful, else falls back to filename.
//! Filters out generic titles like "Vol. 3", "Chapter 5", "#1", etc.

use lazy_static::lazy_static;
use regex::Regex;

use crate::models::{BookStrategy, SmartBookConfig};

use super::{
    BookMetadata, BookNamingContext, BookNamingStrategy, FilenameStrategy,
    filename_without_extension,
};

lazy_static! {
    /// Default patterns for generic titles that should be skipped
    static ref DEFAULT_GENERIC_PATTERNS: Vec<Regex> = {
        [
            r"^Vol\.?\s*\d+$",
            r"^Volume\s*\d+$",
            r"^Chapter\s*\d+$",
            r"^Issue\s*#?\d+$",
            r"^#\d+$",
            r"^\d+$",
            r"^Part\s*\d+$",
            r"^Ep\.?\s*\d+$",
            r"^Episode\s*\d+$",
        ]
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect()
    };
}

/// Use metadata only if meaningful, else filename
pub struct SmartStrategy {
    additional_patterns: Vec<Regex>,
}

impl SmartStrategy {
    pub fn new(config: SmartBookConfig) -> Self {
        let additional_patterns = config
            .additional_generic_patterns
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();
        Self {
            additional_patterns,
        }
    }

    fn is_generic_title(&self, title: &str) -> bool {
        let title = title.trim();

        // Check default patterns
        if DEFAULT_GENERIC_PATTERNS.iter().any(|p| p.is_match(title)) {
            return true;
        }

        // Check additional patterns from config
        if self.additional_patterns.iter().any(|p| p.is_match(title)) {
            return true;
        }

        false
    }
}

impl Default for SmartStrategy {
    fn default() -> Self {
        Self::new(SmartBookConfig::default())
    }
}

impl BookNamingStrategy for SmartStrategy {
    fn strategy_type(&self) -> BookStrategy {
        BookStrategy::Smart
    }

    fn resolve_title(
        &self,
        file_name: &str,
        metadata: Option<&BookMetadata>,
        _context: &BookNamingContext,
    ) -> String {
        metadata
            .and_then(|m| m.title.as_ref())
            .filter(|t| !t.is_empty() && !self.is_generic_title(t))
            .cloned()
            .unwrap_or_else(|| filename_without_extension(file_name))
    }

    /// Volume: ComicInfo first (more authoritative), filename fallback. Mirrors
    /// the Smart "metadata-when-meaningful, filename otherwise" idiom on the
    /// title axis.
    fn resolve_volume(
        &self,
        file_name: &str,
        metadata: Option<&BookMetadata>,
        context: &BookNamingContext,
    ) -> Option<i32> {
        metadata
            .and_then(|m| m.volume)
            .or_else(|| FilenameStrategy::new().resolve_volume(file_name, metadata, context))
    }

    /// Chapter: ComicInfo first, filename fallback.
    fn resolve_chapter(
        &self,
        file_name: &str,
        metadata: Option<&BookMetadata>,
        context: &BookNamingContext,
    ) -> Option<f32> {
        metadata
            .and_then(|m| m.chapter)
            .or_else(|| FilenameStrategy::new().resolve_chapter(file_name, metadata, context))
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
    fn test_uses_meaningful_title() {
        let strategy = SmartStrategy::default();
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
    fn test_skips_vol_pattern() {
        let strategy = SmartStrategy::default();
        let ctx = default_context();
        let metadata = BookMetadata {
            title: Some("Vol. 3".to_string()),
            number: Some(3.0),
            volume: None,
            chapter: None,
        };

        let title = strategy.resolve_title("batman-003.cbz", Some(&metadata), &ctx);
        assert_eq!(title, "batman-003");
    }

    #[test]
    fn test_skips_volume_pattern() {
        let strategy = SmartStrategy::default();
        let ctx = default_context();
        let metadata = BookMetadata {
            title: Some("Volume 1".to_string()),
            number: Some(1.0),
            volume: None,
            chapter: None,
        };

        let title = strategy.resolve_title("series-vol01.cbz", Some(&metadata), &ctx);
        assert_eq!(title, "series-vol01");
    }

    #[test]
    fn test_skips_chapter_pattern() {
        let strategy = SmartStrategy::default();
        let ctx = default_context();
        let metadata = BookMetadata {
            title: Some("Chapter 5".to_string()),
            number: Some(5.0),
            volume: None,
            chapter: None,
        };

        let title = strategy.resolve_title("manga-ch005.cbz", Some(&metadata), &ctx);
        assert_eq!(title, "manga-ch005");
    }

    #[test]
    fn test_skips_issue_pattern() {
        let strategy = SmartStrategy::default();
        let ctx = default_context();
        let metadata = BookMetadata {
            title: Some("Issue #42".to_string()),
            number: Some(42.0),
            volume: None,
            chapter: None,
        };

        let title = strategy.resolve_title("comic-042.cbz", Some(&metadata), &ctx);
        assert_eq!(title, "comic-042");
    }

    #[test]
    fn test_skips_number_only() {
        let strategy = SmartStrategy::default();
        let ctx = default_context();
        let metadata = BookMetadata {
            title: Some("42".to_string()),
            number: Some(42.0),
            volume: None,
            chapter: None,
        };

        let title = strategy.resolve_title("comic-042.cbz", Some(&metadata), &ctx);
        assert_eq!(title, "comic-042");
    }

    #[test]
    fn test_skips_hash_number() {
        let strategy = SmartStrategy::default();
        let ctx = default_context();
        let metadata = BookMetadata {
            title: Some("#1".to_string()),
            number: Some(1.0),
            volume: None,
            chapter: None,
        };

        let title = strategy.resolve_title("comic-001.cbz", Some(&metadata), &ctx);
        assert_eq!(title, "comic-001");
    }

    #[test]
    fn test_additional_patterns() {
        let config = SmartBookConfig {
            additional_generic_patterns: vec![r"^Book\s*\d+$".to_string()],
        };
        let strategy = SmartStrategy::new(config);
        let ctx = default_context();
        let metadata = BookMetadata {
            title: Some("Book 1".to_string()),
            number: Some(1.0),
            volume: None,
            chapter: None,
        };

        let title = strategy.resolve_title("novel-001.epub", Some(&metadata), &ctx);
        assert_eq!(title, "novel-001");
    }

    #[test]
    fn test_fallback_to_filename() {
        let strategy = SmartStrategy::default();
        let ctx = default_context();

        let title = strategy.resolve_title("batman-001.cbz", None, &ctx);
        assert_eq!(title, "batman-001");
    }

    #[test]
    fn test_strategy_type() {
        assert_eq!(
            SmartStrategy::default().strategy_type(),
            BookStrategy::Smart
        );
    }
}
