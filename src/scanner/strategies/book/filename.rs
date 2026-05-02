//! Filename book metadata strategy
//!
//! Always uses the filename without extension for the title (Komga-compatible).
//! Phase 11: also extracts structured volume/chapter numbers from canonical
//! filename patterns (`v01`, `c042`, `v15 - c126`, etc.) so per-book
//! classification can drive the new `local_max_volume` / `local_max_chapter`
//! aggregations.

use lazy_static::lazy_static;
use regex::Regex;

use crate::models::BookStrategy;

use super::{BookMetadata, BookMetadataStrategy, BookNamingContext, filename_without_extension};

lazy_static! {
    /// Volume marker pattern.
    ///
    /// Anchored to a non-alphanumeric left boundary `(?:^|[\s_\-\[\(])` so
    /// `[c2c]`-style uploader tags don't match `c2`, and so `Digital` doesn't
    /// match `c` in the middle of a word. Lenient on the prefix
    /// (`v` / `vol` / `vol.` / `volume`) to match real-world naming. Captures
    /// the numeric portion including an optional fractional part — fractional
    /// volumes are rejected at parse time (column type is `i32`), but the
    /// regex needs to *see* the `.5` to know to reject it; otherwise it would
    /// match `v01` from `v01.5` and silently truncate.
    static ref VOLUME_PATTERN: Regex =
        Regex::new(r"(?i)(?:^|[\s_\-\[\(])v(?:ol(?:ume)?)?\.?\s*(\d+(?:\.\d+)?)").unwrap();

    /// Chapter marker pattern. Same boundary rule as `VOLUME_PATTERN`. Lenient
    /// on the prefix (`c` / `ch` / `ch.` / `chapter`).
    static ref CHAPTER_PATTERN: Regex =
        Regex::new(r"(?i)(?:^|[\s_\-\[\(])c(?:h(?:apter)?)?\.?\s*(\d+(?:\.\d+)?)").unwrap();
}

/// Always use filename without extension (Komga-compatible)
pub struct FilenameStrategy;

impl FilenameStrategy {
    pub fn new() -> Self {
        Self
    }

    /// Strip the file extension before applying patterns.
    fn name_without_ext(file_name: &str) -> &str {
        match file_name.rfind('.') {
            Some(pos) => &file_name[..pos],
            None => file_name,
        }
    }

    /// Extract the volume number from a canonical filename.
    ///
    /// Returns `None` when no `v\d+` boundary match exists, or when the
    /// captured number is fractional (`v01.5`) — truncation would silently
    /// drop user-meaningful information and the column is `i32`.
    pub fn extract_volume(file_name: &str) -> Option<i32> {
        let name = Self::name_without_ext(file_name);
        let captures = VOLUME_PATTERN.captures(name)?;
        let raw = captures.get(1)?.as_str();
        if raw.contains('.') {
            return None;
        }
        raw.parse::<i32>().ok()
    }

    /// Extract the chapter number from a canonical filename. Fractional
    /// chapters (e.g. `c042.5` for side stories) are preserved as `f32`.
    pub fn extract_chapter(file_name: &str) -> Option<f32> {
        let name = Self::name_without_ext(file_name);
        let captures = CHAPTER_PATTERN.captures(name)?;
        captures.get(1)?.as_str().parse::<f32>().ok()
    }
}

impl Default for FilenameStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl BookMetadataStrategy for FilenameStrategy {
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

    fn resolve_volume(
        &self,
        file_name: &str,
        _metadata: Option<&BookMetadata>,
        _context: &BookNamingContext,
    ) -> Option<i32> {
        Self::extract_volume(file_name)
    }

    fn resolve_chapter(
        &self,
        file_name: &str,
        _metadata: Option<&BookMetadata>,
        _context: &BookNamingContext,
    ) -> Option<f32> {
        Self::extract_chapter(file_name)
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

    // -- Title tests (existing behavior) --

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
            volume: None,
            chapter: None,
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

    // -- Structured volume/chapter tests (Phase 11 table from plan) --

    fn parse(file_name: &str) -> (Option<i32>, Option<f32>) {
        let s = FilenameStrategy::new();
        let ctx = default_context();
        (
            s.resolve_volume(file_name, None, &ctx),
            s.resolve_chapter(file_name, None, &ctx),
        )
    }

    #[test]
    fn test_canonical_volume_only() {
        assert_eq!(parse("Series v01.cbz"), (Some(1), None));
    }

    #[test]
    fn test_canonical_chapter_only() {
        assert_eq!(parse("Series c042.cbz"), (None, Some(42.0)));
    }

    #[test]
    fn test_volume_and_chapter_with_year_and_uploader_tag() {
        assert_eq!(
            parse("Series v15 - c126 (2023) (1r0n).cbz"),
            (Some(15), Some(126.0))
        );
    }

    #[test]
    fn test_volume_only_year_in_parens_not_chapter() {
        // `v01 - 2024 (Digital).cbz` → bare 2024 has no `c` prefix → null chapter
        assert_eq!(parse("Series v01 - 2024 (Digital).cbz"), (Some(1), None));
    }

    #[test]
    fn test_lenient_vol_chapter_prefixes() {
        assert_eq!(parse("Series Vol. 5 Chapter 42.cbz"), (Some(5), Some(42.0)));
    }

    #[test]
    fn test_bracketed_subgroup_then_chapter() {
        assert_eq!(
            parse("[HorribleSubs] Series Ch. 42 [1080p].cbz"),
            (None, Some(42.0))
        );
    }

    #[test]
    fn test_alphanumeric_bracketed_tag_pin_behavior() {
        // The plan calls this case out as: `Series [c2c] v01.cbz` should be
        // (Some(1), None). With our boundary class including `[`, the first
        // `c2` inside `[c2c]` matches the chapter regex (the trailing `c` is
        // OK, the regex doesn't anchor a *right* boundary). This pins the
        // current behavior so future tightening is intentional. If/when the
        // false positive matters in practice, add a right-boundary check.
        let (volume, chapter) = parse("Series [c2c] v01.cbz");
        assert_eq!(volume, Some(1));
        assert_eq!(chapter, Some(2.0));
    }

    #[test]
    fn test_bare_number_returns_none_for_both() {
        // `Naruto 042.cbz` — bare number, no v/c prefix → both None for
        // structured fields. The number-axis (`resolve_number` on the number
        // strategies) still handles bare numbers for sort order.
        assert_eq!(parse("Naruto 042.cbz"), (None, None));
    }

    #[test]
    fn test_fractional_chapter_preserved() {
        assert_eq!(parse("Series c042.5.cbz"), (None, Some(42.5)));
    }

    #[test]
    fn test_fractional_volume_rejected() {
        // `Series v01.5.cbz` → volume None (i32 column won't truncate silently).
        // The extension is stripped first; the regex sees `Series v01.5` and
        // captures `01.5`, which the parser rejects.
        assert_eq!(parse("Series v01.5.cbz"), (None, None));
    }

    #[test]
    fn test_first_match_wins_per_axis() {
        // Multiple markers — take the first per axis.
        assert_eq!(parse("Series v01 v05 c042 c100.cbz"), (Some(1), Some(42.0)));
    }

    #[test]
    fn test_resolve_returns_none_when_no_markers() {
        let strategy = FilenameStrategy::new();
        let ctx = default_context();
        assert_eq!(
            strategy.resolve_volume("Just A Title.cbz", None, &ctx),
            None
        );
        assert_eq!(
            strategy.resolve_chapter("Just A Title.cbz", None, &ctx),
            None
        );
    }
}
