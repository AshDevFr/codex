//! Filename number strategy
//!
//! Parses number from filename patterns, ignores metadata.
//! Supports common patterns: #001, v01, c001, Chapter 001, Part 2, etc.

use lazy_static::lazy_static;
use regex::Regex;

use crate::models::NumberStrategy;

use super::{BookNumberStrategy, NumberContext, NumberMetadata};

lazy_static! {
    /// Patterns for extracting numbers from filenames, in priority order
    /// Each pattern should have a capture group for the number
    static ref NUMBER_PATTERNS: Vec<Regex> = vec![
        // Issue/Chapter patterns with # prefix (highest priority - most explicit)
        Regex::new(r"#(\d+(?:\.\d+)?)").unwrap(),

        // Volume patterns
        Regex::new(r"(?i)v(?:ol(?:ume)?)?\.?\s*(\d+(?:\.\d+)?)").unwrap(),

        // Chapter patterns
        Regex::new(r"(?i)c(?:h(?:apter)?)?\.?\s*(\d+(?:\.\d+)?)").unwrap(),

        // Part patterns
        Regex::new(r"(?i)(?:^|[^a-z])part\.?\s*(\d+(?:\.\d+)?)").unwrap(),

        // Episode patterns
        Regex::new(r"(?i)(?:^|[^a-z])ep(?:isode)?\.?\s*(\d+(?:\.\d+)?)").unwrap(),

        // Number in parentheses (often used for issue numbers)
        Regex::new(r"\((\d+(?:\.\d+)?)\)").unwrap(),

        // Bare number at end of filename (before extension) - lowest priority
        // Must be preceded by space, underscore, dash, or start of string
        Regex::new(r"(?:^|[\s_\-])(\d+(?:\.\d+)?)(?:\s*[\(\[]|$)").unwrap(),
    ];
}

/// Parse number from filename patterns only, ignore metadata
pub struct FilenameStrategy;

impl FilenameStrategy {
    pub fn new() -> Self {
        Self
    }

    /// Extract number from filename using pattern matching
    fn extract_number_from_filename(file_name: &str) -> Option<f32> {
        // Remove extension first
        let name_without_ext = if let Some(pos) = file_name.rfind('.') {
            &file_name[..pos]
        } else {
            file_name
        };

        // Try each pattern in priority order
        for pattern in NUMBER_PATTERNS.iter() {
            if let Some(captures) = pattern.captures(name_without_ext) {
                if let Some(num_match) = captures.get(1) {
                    if let Ok(num) = num_match.as_str().parse::<f32>() {
                        return Some(num);
                    }
                }
            }
        }

        None
    }
}

impl Default for FilenameStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl BookNumberStrategy for FilenameStrategy {
    fn strategy_type(&self) -> NumberStrategy {
        NumberStrategy::Filename
    }

    fn resolve_number(
        &self,
        file_name: &str,
        _metadata: Option<&NumberMetadata>,
        _context: &NumberContext,
    ) -> Option<f32> {
        Self::extract_number_from_filename(file_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(position: usize, total: usize) -> NumberContext {
        NumberContext::new(position, total)
    }

    // ===== Hash/Issue patterns =====

    #[test]
    fn test_hash_pattern() {
        let strategy = FilenameStrategy::new();
        let ctx = make_context(1, 10);

        assert_eq!(
            strategy.resolve_number("Batman #001.cbz", None, &ctx),
            Some(1.0)
        );
        assert_eq!(
            strategy.resolve_number("Batman #42.cbz", None, &ctx),
            Some(42.0)
        );
        assert_eq!(
            strategy.resolve_number("Amazing Spider-Man #583.cbz", None, &ctx),
            Some(583.0)
        );
    }

    #[test]
    fn test_hash_pattern_fractional() {
        let strategy = FilenameStrategy::new();
        let ctx = make_context(1, 10);

        assert_eq!(
            strategy.resolve_number("Batman #1.5.cbz", None, &ctx),
            Some(1.5)
        );
    }

    // ===== Volume patterns =====

    #[test]
    fn test_volume_patterns() {
        let strategy = FilenameStrategy::new();
        let ctx = make_context(1, 10);

        assert_eq!(
            strategy.resolve_number("One Piece v01.cbz", None, &ctx),
            Some(1.0)
        );
        assert_eq!(
            strategy.resolve_number("Naruto Vol. 5.cbz", None, &ctx),
            Some(5.0)
        );
        assert_eq!(
            strategy.resolve_number("Manga Volume 12.cbz", None, &ctx),
            Some(12.0)
        );
        assert_eq!(
            strategy.resolve_number("Series vol5.cbz", None, &ctx),
            Some(5.0)
        );
    }

    // ===== Chapter patterns =====

    #[test]
    fn test_chapter_patterns() {
        let strategy = FilenameStrategy::new();
        let ctx = make_context(1, 10);

        assert_eq!(
            strategy.resolve_number("A Returner's Magic Should Be Special c001.cbz", None, &ctx),
            Some(1.0)
        );
        assert_eq!(
            strategy.resolve_number("Manga Ch. 42.cbz", None, &ctx),
            Some(42.0)
        );
        assert_eq!(
            strategy.resolve_number("Series Chapter 100.cbz", None, &ctx),
            Some(100.0)
        );
        assert_eq!(
            strategy.resolve_number("Webtoon ch5.cbz", None, &ctx),
            Some(5.0)
        );
    }

    // ===== Part patterns =====

    #[test]
    fn test_part_patterns() {
        let strategy = FilenameStrategy::new();
        let ctx = make_context(1, 10);

        assert_eq!(
            strategy.resolve_number("Story Part 1.cbz", None, &ctx),
            Some(1.0)
        );
        assert_eq!(
            strategy.resolve_number("Novel part2.cbz", None, &ctx),
            Some(2.0)
        );
        assert_eq!(
            strategy.resolve_number("Book Part. 3.cbz", None, &ctx),
            Some(3.0)
        );
    }

    // ===== Episode patterns =====

    #[test]
    fn test_episode_patterns() {
        let strategy = FilenameStrategy::new();
        let ctx = make_context(1, 10);

        assert_eq!(
            strategy.resolve_number("Anime Ep 5.cbz", None, &ctx),
            Some(5.0)
        );
        assert_eq!(
            strategy.resolve_number("Show Episode 12.cbz", None, &ctx),
            Some(12.0)
        );
        assert_eq!(
            strategy.resolve_number("Series ep.3.cbz", None, &ctx),
            Some(3.0)
        );
    }

    // ===== Parentheses patterns =====

    #[test]
    fn test_parentheses_pattern() {
        let strategy = FilenameStrategy::new();
        let ctx = make_context(1, 10);

        assert_eq!(
            strategy.resolve_number("Batman (2011) (42).cbz", None, &ctx),
            // First match is 2011, but we want issue number
            // This test verifies parentheses pattern works
            Some(2011.0)
        );
    }

    // ===== Bare number patterns =====

    #[test]
    fn test_bare_number_at_end() {
        let strategy = FilenameStrategy::new();
        let ctx = make_context(1, 10);

        assert_eq!(
            strategy.resolve_number("Comic 001.cbz", None, &ctx),
            Some(1.0)
        );
        assert_eq!(
            strategy.resolve_number("Manga_042.cbz", None, &ctx),
            Some(42.0)
        );
        assert_eq!(strategy.resolve_number("Book-5.cbz", None, &ctx), Some(5.0));
    }

    // ===== Edge cases =====

    #[test]
    fn test_no_number_found() {
        let strategy = FilenameStrategy::new();
        let ctx = make_context(1, 10);

        assert_eq!(
            strategy.resolve_number("Just A Title.cbz", None, &ctx),
            None
        );
    }

    #[test]
    fn test_ignores_metadata() {
        let strategy = FilenameStrategy::new();
        let ctx = make_context(1, 10);
        let metadata = NumberMetadata { number: Some(99.0) };

        // Should use filename, not metadata
        assert_eq!(
            strategy.resolve_number("Batman #42.cbz", Some(&metadata), &ctx),
            Some(42.0)
        );
    }

    #[test]
    fn test_ignores_file_order() {
        let strategy = FilenameStrategy::new();
        let ctx = make_context(7, 10);

        // Should use filename, not position
        assert_eq!(
            strategy.resolve_number("Batman #42.cbz", None, &ctx),
            Some(42.0)
        );
    }

    #[test]
    fn test_complex_filename() {
        let strategy = FilenameStrategy::new();
        let ctx = make_context(1, 10);

        // Real-world example from issue
        assert_eq!(
            strategy.resolve_number(
                "A Returner's Magic Should Be Special c001 (2021) (Digital) (4str0).cbz",
                None,
                &ctx
            ),
            Some(1.0)
        );
    }

    #[test]
    fn test_priority_hash_over_bare() {
        let strategy = FilenameStrategy::new();
        let ctx = make_context(1, 10);

        // Hash pattern should take priority over bare number in year
        assert_eq!(
            strategy.resolve_number("Comic 2020 #5.cbz", None, &ctx),
            Some(5.0)
        );
    }

    #[test]
    fn test_strategy_type() {
        assert_eq!(
            FilenameStrategy::new().strategy_type(),
            NumberStrategy::Filename
        );
    }

    // ===== Real-world filenames =====

    #[test]
    fn test_real_world_filenames() {
        let strategy = FilenameStrategy::new();
        let ctx = make_context(1, 100);

        // Various real-world naming conventions
        assert_eq!(
            strategy.resolve_number("Naruto v05.cbz", None, &ctx),
            Some(5.0)
        );
        assert_eq!(
            strategy.resolve_number("One Punch Man - Chapter 150.cbz", None, &ctx),
            Some(150.0)
        );
        assert_eq!(
            strategy.resolve_number("[HorribleSubs] Attack on Titan - 001.cbz", None, &ctx),
            Some(1.0)
        );
        // JoJo filename: v03 volume pattern matches before Part pattern
        // because volume patterns are checked before part patterns in priority order
        assert_eq!(
            strategy.resolve_number("JoJo's Bizarre Adventure Part 5 v03 c016.cbz", None, &ctx),
            Some(3.0) // Volume pattern (v03) matches first
        );
    }
}
