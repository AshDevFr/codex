//! Custom book naming strategy
//!
//! Uses user-defined regex patterns for extracting title, volume, and chapter
//! from filenames. Provides maximum flexibility for non-standard naming conventions.

use regex::Regex;

use crate::models::{BookStrategy, CustomBookConfig};

use super::{
    BookMetadata, BookNamingContext, BookNamingStrategy, create_book_strategy,
    filename_without_extension,
};

/// Custom book naming strategy using regex patterns
pub struct CustomStrategy {
    pattern: Option<Regex>,
    title_template: Option<String>,
    fallback: BookStrategy,
}

impl CustomStrategy {
    pub fn new(config: CustomBookConfig) -> Self {
        let pattern = Regex::new(&config.pattern).ok();

        // Parse fallback strategy, default to Filename if invalid
        let fallback = config
            .fallback
            .parse::<BookStrategy>()
            .unwrap_or(BookStrategy::Filename);

        // Prevent infinite recursion: Custom cannot fallback to Custom
        let fallback = if fallback == BookStrategy::Custom {
            BookStrategy::Filename
        } else {
            fallback
        };

        Self {
            pattern,
            title_template: config.title_template,
            fallback,
        }
    }

    /// Apply template substitution with captured groups
    fn apply_template(&self, template: &str, captures: &regex::Captures, filename: &str) -> String {
        let mut result = template.to_string();

        // Replace named groups
        for name in ["series", "volume", "chapter", "title"] {
            if let Some(m) = captures.name(name) {
                result = result.replace(&format!("{{{}}}", name), m.as_str());
            } else {
                // Remove placeholder if group not captured
                result = result.replace(&format!("{{{}}}", name), "");
            }
        }

        // Replace {filename} with the actual filename (without extension)
        result = result.replace("{filename}", &filename_without_extension(filename));

        // Clean up any double spaces from removed placeholders
        while result.contains("  ") {
            result = result.replace("  ", " ");
        }
        result.trim().to_string()
    }

    /// Extract volume number from captures
    pub fn extract_volume(&self, filename: &str) -> Option<f32> {
        self.pattern.as_ref().and_then(|p| {
            p.captures(filename)
                .and_then(|c| c.name("volume"))
                .and_then(|m| m.as_str().parse().ok())
        })
    }

    /// Extract chapter number from captures
    pub fn extract_chapter(&self, filename: &str) -> Option<f32> {
        self.pattern.as_ref().and_then(|p| {
            p.captures(filename)
                .and_then(|c| c.name("chapter"))
                .and_then(|m| m.as_str().parse().ok())
        })
    }
}

impl Default for CustomStrategy {
    fn default() -> Self {
        Self::new(CustomBookConfig::default())
    }
}

impl BookNamingStrategy for CustomStrategy {
    fn strategy_type(&self) -> BookStrategy {
        BookStrategy::Custom
    }

    fn resolve_title(
        &self,
        file_name: &str,
        metadata: Option<&BookMetadata>,
        context: &BookNamingContext,
    ) -> String {
        // Try to match pattern
        if let Some(ref pattern) = self.pattern {
            // Match against filename without extension for cleaner patterns
            let name_without_ext = filename_without_extension(file_name);

            if let Some(captures) = pattern.captures(&name_without_ext) {
                // If we have a title template, use it
                if let Some(ref template) = self.title_template {
                    return self.apply_template(template, &captures, file_name);
                }

                // If no template but title group captured, use it
                if let Some(title_match) = captures.name("title") {
                    let title = title_match.as_str().trim();
                    if !title.is_empty() {
                        return title.to_string();
                    }
                }
            }
        }

        // Fallback to another strategy
        let fallback_strategy = create_book_strategy(self.fallback, None);
        fallback_strategy.resolve_title(file_name, metadata, context)
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
    fn test_strategy_type() {
        assert_eq!(
            CustomStrategy::default().strategy_type(),
            BookStrategy::Custom
        );
    }

    #[test]
    fn test_simple_title_extraction() {
        let config = CustomBookConfig {
            pattern: r"(?P<title>.+?)_v\d+".to_string(),
            title_template: None,
            fallback: "filename".to_string(),
        };
        let strategy = CustomStrategy::new(config);
        let ctx = default_context();

        let title = strategy.resolve_title("One_Piece_v01.cbz", None, &ctx);
        assert_eq!(title, "One_Piece");
    }

    #[test]
    fn test_template_with_volume_chapter() {
        let config = CustomBookConfig {
            pattern: r"(?P<series>.+?)_v(?P<volume>\d+)_c(?P<chapter>\d+)".to_string(),
            title_template: Some("{series} v.{volume} c.{chapter}".to_string()),
            fallback: "filename".to_string(),
        };
        let strategy = CustomStrategy::new(config);
        let ctx = default_context();

        let title = strategy.resolve_title("One_Piece_v012_c145.cbz", None, &ctx);
        assert_eq!(title, "One_Piece v.012 c.145");
    }

    #[test]
    fn test_extract_volume() {
        let config = CustomBookConfig {
            pattern: r"(?P<series>.+?)_v(?P<volume>\d+)_c(?P<chapter>\d+)".to_string(),
            title_template: None,
            fallback: "filename".to_string(),
        };
        let strategy = CustomStrategy::new(config);

        assert_eq!(strategy.extract_volume("One_Piece_v012_c145"), Some(12.0));
    }

    #[test]
    fn test_extract_chapter() {
        let config = CustomBookConfig {
            pattern: r"(?P<series>.+?)_v(?P<volume>\d+)_c(?P<chapter>\d+)".to_string(),
            title_template: None,
            fallback: "filename".to_string(),
        };
        let strategy = CustomStrategy::new(config);

        assert_eq!(strategy.extract_chapter("One_Piece_v012_c145"), Some(145.0));
    }

    #[test]
    fn test_fallback_to_filename() {
        let config = CustomBookConfig {
            pattern: r"(?P<series>.+?)_v(?P<volume>\d+)_c(?P<chapter>\d+)".to_string(),
            title_template: None,
            fallback: "filename".to_string(),
        };
        let strategy = CustomStrategy::new(config);
        let ctx = default_context();

        // This doesn't match the pattern, so fallback to filename
        let title = strategy.resolve_title("random-file.cbz", None, &ctx);
        assert_eq!(title, "random-file");
    }

    #[test]
    fn test_fallback_to_metadata_first() {
        let config = CustomBookConfig {
            pattern: r"(?P<series>.+?)_v(?P<volume>\d+)".to_string(),
            title_template: None,
            fallback: "metadata_first".to_string(),
        };
        let strategy = CustomStrategy::new(config);
        let ctx = default_context();
        let metadata = BookMetadata {
            title: Some("The Dark Knight".to_string()),
            number: Some(1.0),
        };

        // This doesn't match the pattern, so fallback to metadata_first
        let title = strategy.resolve_title("random-file.cbz", Some(&metadata), &ctx);
        assert_eq!(title, "The Dark Knight");
    }

    #[test]
    fn test_invalid_regex_fallback() {
        let config = CustomBookConfig {
            pattern: r"[invalid(regex".to_string(), // Invalid regex
            title_template: None,
            fallback: "filename".to_string(),
        };
        let strategy = CustomStrategy::new(config);
        let ctx = default_context();

        // Invalid pattern should fallback to filename
        let title = strategy.resolve_title("test-file.cbz", None, &ctx);
        assert_eq!(title, "test-file");
    }

    #[test]
    fn test_prevent_custom_fallback_recursion() {
        let config = CustomBookConfig {
            pattern: r"(?P<title>.+)".to_string(),
            title_template: None,
            fallback: "custom".to_string(), // This should be replaced with filename
        };
        let strategy = CustomStrategy::new(config);

        // Should have been converted to Filename to prevent recursion
        assert_eq!(strategy.fallback, BookStrategy::Filename);
    }

    #[test]
    fn test_filename_placeholder_in_template() {
        let config = CustomBookConfig {
            pattern: r"(?P<series>.+?)_v(?P<volume>\d+)".to_string(),
            title_template: Some("{series} - {filename}".to_string()),
            fallback: "filename".to_string(),
        };
        let strategy = CustomStrategy::new(config);
        let ctx = default_context();

        let title = strategy.resolve_title("One_Piece_v01.cbz", None, &ctx);
        assert_eq!(title, "One_Piece - One_Piece_v01");
    }

    #[test]
    fn test_template_cleans_empty_placeholders() {
        let config = CustomBookConfig {
            pattern: r"(?P<series>.+?)_v(?P<volume>\d+)".to_string(),
            // chapter is not in pattern, should be cleaned up
            title_template: Some("{series} v.{volume} c.{chapter}".to_string()),
            fallback: "filename".to_string(),
        };
        let strategy = CustomStrategy::new(config);
        let ctx = default_context();

        let title = strategy.resolve_title("One_Piece_v01.cbz", None, &ctx);
        assert_eq!(title, "One_Piece v.01 c.");
    }

    #[test]
    fn test_episode_pattern() {
        let config = CustomBookConfig {
            pattern: r"^(?P<series>.+?) - (?P<volume>\d+)x(?P<chapter>\d+) - (?P<title>.+)$"
                .to_string(),
            title_template: None, // Use captured title
            fallback: "filename".to_string(),
        };
        let strategy = CustomStrategy::new(config);
        let ctx = default_context();

        let title = strategy.resolve_title("One Piece - 01x05 - Romance Dawn.cbz", None, &ctx);
        assert_eq!(title, "Romance Dawn");
    }

    #[test]
    fn test_scanlation_group_pattern() {
        let config = CustomBookConfig {
            pattern: r"^\[[^\]]+\]\s*(?P<series>.+?)\s+v(?P<volume>\d+)\s+c(?P<chapter>\d+)"
                .to_string(),
            title_template: Some("{series} v.{volume} c.{chapter}".to_string()),
            fallback: "filename".to_string(),
        };
        let strategy = CustomStrategy::new(config);
        let ctx = default_context();

        let title = strategy.resolve_title("[GroupName] One Piece v01 c001.cbz", None, &ctx);
        assert_eq!(title, "One Piece v.01 c.001");
    }

    #[test]
    fn test_volume_chapter_common_pattern() {
        let config = CustomBookConfig {
            pattern: r"(?P<series>.+?)\s+Vol\.(?P<volume>\d+)\s+Ch\.(?P<chapter>\d+)".to_string(),
            title_template: Some("{series} Volume {volume} Chapter {chapter}".to_string()),
            fallback: "filename".to_string(),
        };
        let strategy = CustomStrategy::new(config);
        let ctx = default_context();

        let title = strategy.resolve_title("One Piece Vol.1 Ch.5.cbz", None, &ctx);
        assert_eq!(title, "One Piece Volume 1 Chapter 5");
    }
}
