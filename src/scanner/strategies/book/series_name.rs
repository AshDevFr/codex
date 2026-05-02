//! SeriesName book naming strategy
//!
//! Generates uniform titles from series name + position (e.g., "One Piece v.045")

use lazy_static::lazy_static;
use regex::Regex;

use crate::models::BookStrategy;

use super::{BookMetadata, BookNamingContext, BookNamingStrategy, filename_without_extension};

lazy_static! {
    /// Pattern for extracting numbers from strings
    static ref NUMBER_PATTERN: Regex = Regex::new(r"(\d+(?:\.\d+)?)").unwrap();
}

/// Generate title from series name + position
pub struct SeriesNameStrategy;

impl SeriesNameStrategy {
    pub fn new() -> Self {
        Self
    }

    /// Calculate padding for volume numbers based on total count
    fn get_volume_padding(count: usize) -> usize {
        match count {
            0..=99 => 2,
            100..=999 => 3,
            _ => 4,
        }
    }

    /// Calculate padding for chapter numbers based on total count
    fn get_chapter_padding(count: usize) -> usize {
        match count {
            0..=999 => 3,
            _ => 4,
        }
    }

    /// Format a number with appropriate padding
    fn format_number(num: f32, padding: usize) -> String {
        if num.fract() == 0.0 {
            format!("{:0>width$}", num as i32, width = padding)
        } else {
            // Handle decimal numbers (e.g., 1.5 for specials)
            let int_part = num as i32;
            let frac_part = ((num.fract() * 10.0).round() as i32).abs();
            format!("{:0>width$}.{}", int_part, frac_part, width = padding)
        }
    }
}

impl Default for SeriesNameStrategy {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract a number from a string (e.g., "Volume 01" -> 1.0)
fn extract_number_from_string(s: &str) -> Option<f32> {
    NUMBER_PATTERN
        .captures(s)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<f32>().ok())
}

impl BookNamingStrategy for SeriesNameStrategy {
    fn strategy_type(&self) -> BookStrategy {
        BookStrategy::SeriesName
    }

    /// SeriesName operates on the upstream series-detection output via
    /// `BookNamingContext`. For the per-book classification axis it just
    /// passes through whatever the series detection populated. When the
    /// detection isn't `series_volume_chapter`, both context fields are `None`
    /// and so are the answers (correct — SeriesName isn't a parser).
    fn resolve_volume(
        &self,
        _file_name: &str,
        _metadata: Option<&BookMetadata>,
        context: &BookNamingContext,
    ) -> Option<i32> {
        context
            .volume
            .as_deref()
            .and_then(extract_number_from_string)
            .and_then(|n| {
                if n.fract() == 0.0 && n >= 0.0 {
                    Some(n as i32)
                } else {
                    None
                }
            })
    }

    fn resolve_chapter(
        &self,
        _file_name: &str,
        _metadata: Option<&BookMetadata>,
        context: &BookNamingContext,
    ) -> Option<f32> {
        context.chapter_number
    }

    fn resolve_title(
        &self,
        file_name: &str,
        _metadata: Option<&BookMetadata>,
        context: &BookNamingContext,
    ) -> String {
        let series = &context.series_name;

        // If we have volume and chapter info (series_volume_chapter strategy)
        if let (Some(volume), Some(chapter)) = (&context.volume, context.chapter_number) {
            // Extract volume number from string like "Volume 01" or just "01"
            let vol_num = extract_number_from_string(volume).unwrap_or(1.0);
            // Use 2 digits for volumes by default (most series have <100 volumes)
            let vol_padding = Self::get_volume_padding(vol_num as usize);
            let chap_padding = Self::get_chapter_padding(context.total_books);

            return format!(
                "{} v.{} c.{}",
                series,
                Self::format_number(vol_num, vol_padding),
                Self::format_number(chapter, chap_padding)
            );
        }

        // If we have a book number
        if let Some(number) = context.book_number {
            let padding = Self::get_volume_padding(context.total_books);
            return format!("{} v.{}", series, Self::format_number(number, padding));
        }

        // Fallback to filename if no number info available
        filename_without_extension(file_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_volume_format() {
        let strategy = SeriesNameStrategy::new();
        let ctx = BookNamingContext {
            series_name: "One Piece".to_string(),
            book_number: Some(1.0),
            volume: None,
            chapter_number: None,
            total_books: 50,
        };

        let title = strategy.resolve_title("random_file.cbz", None, &ctx);
        assert_eq!(title, "One Piece v.01");
    }

    #[test]
    fn test_volume_chapter_format() {
        let strategy = SeriesNameStrategy::new();
        let ctx = BookNamingContext {
            series_name: "One Piece".to_string(),
            book_number: None,
            volume: Some("Volume 01".to_string()),
            chapter_number: Some(5.0),
            total_books: 150,
        };

        let title = strategy.resolve_title("random_file.cbz", None, &ctx);
        assert_eq!(title, "One Piece v.01 c.005");
    }

    #[test]
    fn test_large_series_padding() {
        let strategy = SeriesNameStrategy::new();
        let ctx = BookNamingContext {
            series_name: "Detective Conan".to_string(),
            book_number: Some(42.0),
            volume: None,
            chapter_number: None,
            total_books: 120,
        };

        let title = strategy.resolve_title("file.cbz", None, &ctx);
        assert_eq!(title, "Detective Conan v.042");
    }

    #[test]
    fn test_decimal_number() {
        let strategy = SeriesNameStrategy::new();
        let ctx = BookNamingContext {
            series_name: "Special".to_string(),
            book_number: Some(1.5),
            volume: None,
            chapter_number: None,
            total_books: 10,
        };

        let title = strategy.resolve_title("file.cbz", None, &ctx);
        assert_eq!(title, "Special v.01.5");
    }

    #[test]
    fn test_fallback_when_no_number() {
        let strategy = SeriesNameStrategy::new();
        let ctx = BookNamingContext {
            series_name: "Unknown".to_string(),
            book_number: None,
            volume: None,
            chapter_number: None,
            total_books: 5,
        };

        let title = strategy.resolve_title("actual_title.cbz", None, &ctx);
        assert_eq!(title, "actual_title");
    }

    #[test]
    fn test_chapter_padding_large() {
        let strategy = SeriesNameStrategy::new();
        let ctx = BookNamingContext {
            series_name: "Long Series".to_string(),
            book_number: None,
            volume: Some("Vol 10".to_string()),
            chapter_number: Some(999.0),
            total_books: 1500,
        };

        let title = strategy.resolve_title("file.cbz", None, &ctx);
        assert_eq!(title, "Long Series v.10 c.0999");
    }

    #[test]
    fn test_strategy_type() {
        assert_eq!(
            SeriesNameStrategy::new().strategy_type(),
            BookStrategy::SeriesName
        );
    }

    #[test]
    fn test_extract_number_basic() {
        assert_eq!(extract_number_from_string("Volume 5"), Some(5.0));
    }

    #[test]
    fn test_extract_number_with_leading_zeros() {
        assert_eq!(extract_number_from_string("Vol 01"), Some(1.0));
    }

    #[test]
    fn test_extract_number_decimal() {
        assert_eq!(extract_number_from_string("Chapter 1.5"), Some(1.5));
    }

    #[test]
    fn test_extract_number_none() {
        assert_eq!(extract_number_from_string("No numbers here"), None);
    }
}
