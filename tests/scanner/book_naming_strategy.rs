//! Integration tests for book naming strategies
//!
//! Tests that book naming strategies are correctly applied during
//! library scanning and book analysis.

#[path = "../common/mod.rs"]
mod common;

use codex::models::{BookStrategy, SeriesStrategy};
use codex::scanner::strategies::{BookMetadata, BookNamingContext, create_book_strategy};
use common::*;

// ============================================================================
// Unit-style integration tests for strategy resolution
// ============================================================================

/// Test that filename strategy always uses the filename regardless of metadata
#[test]
fn test_filename_strategy_ignores_metadata() {
    let strategy = create_book_strategy(BookStrategy::Filename, None);
    let context = BookNamingContext {
        series_name: "Batman".to_string(),
        book_number: Some(1.0),
        volume: None,
        chapter_number: None,
        total_books: 50,
    };
    let metadata = BookMetadata {
        title: Some("The Dark Knight Returns".to_string()),
        number: Some(1.0),
        volume: None,
        chapter: None,
    };

    let title = strategy.resolve_title("Batman Issue 001.cbz", Some(&metadata), &context);

    assert_eq!(title, "Batman Issue 001");
    assert_eq!(strategy.strategy_type(), BookStrategy::Filename);
}

/// Test that metadata_first strategy uses metadata when available
#[test]
fn test_metadata_first_strategy_uses_metadata() {
    let strategy = create_book_strategy(BookStrategy::MetadataFirst, None);
    let context = BookNamingContext {
        series_name: "Batman".to_string(),
        book_number: Some(1.0),
        volume: None,
        chapter_number: None,
        total_books: 50,
    };
    let metadata = BookMetadata {
        title: Some("The Dark Knight Returns".to_string()),
        number: Some(1.0),
        volume: None,
        chapter: None,
    };

    let title = strategy.resolve_title("batman_001.cbz", Some(&metadata), &context);

    assert_eq!(title, "The Dark Knight Returns");
    assert_eq!(strategy.strategy_type(), BookStrategy::MetadataFirst);
}

/// Test that metadata_first strategy falls back to filename when no metadata
#[test]
fn test_metadata_first_strategy_fallback() {
    let strategy = create_book_strategy(BookStrategy::MetadataFirst, None);
    let context = BookNamingContext {
        series_name: "Batman".to_string(),
        book_number: Some(1.0),
        volume: None,
        chapter_number: None,
        total_books: 50,
    };

    let title = strategy.resolve_title("batman_001.cbz", None, &context);

    assert_eq!(title, "batman_001");
}

/// Test that smart strategy rejects generic metadata titles
#[test]
fn test_smart_strategy_rejects_generic_titles() {
    let strategy = create_book_strategy(BookStrategy::Smart, None);
    let context = BookNamingContext {
        series_name: "One Piece".to_string(),
        book_number: Some(3.0),
        volume: None,
        chapter_number: None,
        total_books: 100,
    };

    // Test various generic title patterns
    let generic_titles = vec![
        ("Vol. 3", "one_piece_v03.cbz"),
        ("Volume 3", "one_piece_v03.cbz"),
        ("Chapter 5", "one_piece_ch005.cbz"),
        ("Issue #42", "comic_042.cbz"),
        ("#1", "comic_001.cbz"),
        ("3", "book_003.cbz"),
    ];

    for (generic_title, filename) in generic_titles {
        let metadata = BookMetadata {
            title: Some(generic_title.to_string()),
            number: Some(3.0),
            volume: None,
            chapter: None,
        };

        let title = strategy.resolve_title(filename, Some(&metadata), &context);

        // Should fall back to filename without extension
        let expected = filename.strip_suffix(".cbz").unwrap();
        assert_eq!(
            title, expected,
            "Generic title '{}' should be rejected, got '{}'",
            generic_title, title
        );
    }
}

/// Test that smart strategy accepts meaningful metadata titles
#[test]
fn test_smart_strategy_accepts_meaningful_titles() {
    let strategy = create_book_strategy(BookStrategy::Smart, None);
    let context = BookNamingContext {
        series_name: "Batman".to_string(),
        book_number: Some(1.0),
        volume: None,
        chapter_number: None,
        total_books: 50,
    };
    let metadata = BookMetadata {
        title: Some("The Killing Joke".to_string()),
        number: Some(1.0),
        volume: None,
        chapter: None,
    };

    let title = strategy.resolve_title("batman_special_001.cbz", Some(&metadata), &context);

    assert_eq!(title, "The Killing Joke");
}

/// Test smart strategy with custom generic patterns from config
#[test]
fn test_smart_strategy_custom_patterns() {
    let config = r#"{"additionalGenericPatterns":["^Book\\s*\\d+$"]}"#;
    let strategy = create_book_strategy(BookStrategy::Smart, Some(config));
    let context = BookNamingContext {
        series_name: "Test".to_string(),
        book_number: Some(1.0),
        volume: None,
        chapter_number: None,
        total_books: 10,
    };
    let metadata = BookMetadata {
        title: Some("Book 1".to_string()),
        number: Some(1.0),
        volume: None,
        chapter: None,
    };

    let title = strategy.resolve_title("novel_001.epub", Some(&metadata), &context);

    assert_eq!(title, "novel_001");
}

/// Test series_name strategy generates uniform titles
#[test]
fn test_series_name_strategy_volume_format() {
    let strategy = create_book_strategy(BookStrategy::SeriesName, None);
    let context = BookNamingContext {
        series_name: "One Piece".to_string(),
        book_number: Some(45.0),
        volume: None,
        chapter_number: None,
        total_books: 100,
    };

    let title = strategy.resolve_title("random_filename.cbz", None, &context);

    assert_eq!(title, "One Piece v.045");
    assert_eq!(strategy.strategy_type(), BookStrategy::SeriesName);
}

/// Test series_name strategy with volume and chapter info
#[test]
fn test_series_name_strategy_volume_chapter_format() {
    let strategy = create_book_strategy(BookStrategy::SeriesName, None);
    let context = BookNamingContext {
        series_name: "One Piece".to_string(),
        book_number: None,
        volume: Some("Volume 10".to_string()),
        chapter_number: Some(95.0),
        total_books: 150,
    };

    let title = strategy.resolve_title("chapter_95.cbz", None, &context);

    assert_eq!(title, "One Piece v.10 c.095");
}

/// Test series_name strategy fallback when no number info
#[test]
fn test_series_name_strategy_fallback() {
    let strategy = create_book_strategy(BookStrategy::SeriesName, None);
    let context = BookNamingContext {
        series_name: "Unknown".to_string(),
        book_number: None,
        volume: None,
        chapter_number: None,
        total_books: 5,
    };

    let title = strategy.resolve_title("actual_title.cbz", None, &context);

    // Falls back to filename when no number info available
    assert_eq!(title, "actual_title");
}

/// Test padding scales with book count
#[test]
fn test_series_name_strategy_padding_scales() {
    let strategy = create_book_strategy(BookStrategy::SeriesName, None);

    // Small series (< 100 books) uses 2 digits
    let small_context = BookNamingContext {
        series_name: "Test".to_string(),
        book_number: Some(5.0),
        volume: None,
        chapter_number: None,
        total_books: 50,
    };
    let title = strategy.resolve_title("file.cbz", None, &small_context);
    assert_eq!(title, "Test v.05");

    // Large series (100+ books) uses 3 digits
    let large_context = BookNamingContext {
        series_name: "Test".to_string(),
        book_number: Some(5.0),
        volume: None,
        chapter_number: None,
        total_books: 150,
    };
    let title = strategy.resolve_title("file.cbz", None, &large_context);
    assert_eq!(title, "Test v.005");
}

/// Test decimal book numbers (for specials, etc.)
#[test]
fn test_series_name_strategy_decimal_numbers() {
    let strategy = create_book_strategy(BookStrategy::SeriesName, None);
    let context = BookNamingContext {
        series_name: "Special".to_string(),
        book_number: Some(1.5),
        volume: None,
        chapter_number: None,
        total_books: 10,
    };

    let title = strategy.resolve_title("file.cbz", None, &context);

    assert_eq!(title, "Special v.01.5");
}

// ============================================================================
// Database integration tests
// ============================================================================

/// Test that library with different book strategies stores correctly
#[tokio::test]
async fn test_library_book_strategy_persistence() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library with smart book strategy
    let library = create_test_library_with_strategies(
        &db,
        "Smart Library",
        "/test/path",
        SeriesStrategy::SeriesVolume,
        BookStrategy::Smart,
    )
    .await;

    assert_eq!(library.book_strategy, "smart");
    assert_eq!(library.series_strategy, "series_volume");

    // Create library with metadata_first strategy
    let library2 = create_test_library_with_strategies(
        &db,
        "Metadata Library",
        "/test/path2",
        SeriesStrategy::Flat,
        BookStrategy::MetadataFirst,
    )
    .await;

    assert_eq!(library2.book_strategy, "metadata_first");
    assert_eq!(library2.series_strategy, "flat");
}

/// Test that book strategy can be retrieved and parsed from library
#[tokio::test]
async fn test_library_book_strategy_retrieval() {
    use codex::db::repositories::LibraryRepository;

    let (db, _temp_dir) = setup_test_db().await;

    let library = create_test_library_with_strategies(
        &db,
        "Test Library",
        "/test/path",
        SeriesStrategy::SeriesVolume,
        BookStrategy::SeriesName,
    )
    .await;

    // Retrieve the library
    let retrieved = LibraryRepository::get_by_id(&db, library.id)
        .await
        .unwrap()
        .unwrap();

    // Parse the strategy
    let book_strategy: BookStrategy = retrieved.book_strategy.parse().unwrap();
    assert_eq!(book_strategy, BookStrategy::SeriesName);
}

/// Test default book strategy is filename
#[tokio::test]
async fn test_default_book_strategy() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library with defaults
    let library = create_test_library(&db, "Default Library", "/test/path").await;

    assert_eq!(library.book_strategy, "filename");
}

/// Test book strategy config persistence
#[tokio::test]
async fn test_book_strategy_config_persistence() {
    use codex::db::repositories::LibraryRepository;
    use codex::db::repositories::library::CreateLibraryParams;

    let (db, _temp_dir) = setup_test_db().await;

    let config = r#"{"additionalGenericPatterns":["^Test\\d+$"]}"#;
    let config_value: serde_json::Value = serde_json::from_str(config).unwrap();
    let params = CreateLibraryParams::new("Config Library", "/test/path")
        .with_book_strategy(BookStrategy::Smart)
        .with_book_config(Some(config_value.clone()));

    let library = LibraryRepository::create_with_params(&db, params)
        .await
        .unwrap();

    assert_eq!(library.book_strategy, "smart");
    assert_eq!(library.book_config, Some(config_value));

    // Verify the config can be used to create a strategy
    let config_str = library.book_config.as_ref().map(|v| v.to_string());
    let strategy = create_book_strategy(BookStrategy::Smart, config_str.as_deref());
    assert_eq!(strategy.strategy_type(), BookStrategy::Smart);
}

// ============================================================================
// Custom book strategy tests
// ============================================================================

/// Test custom strategy with volume and chapter extraction
#[test]
fn test_custom_strategy_volume_chapter_extraction() {
    let config = r#"{"pattern":"(?P<series>.+?)_v(?P<volume>\\d+)_c(?P<chapter>\\d+)","titleTemplate":"{series} v.{volume} c.{chapter}","fallback":"filename"}"#;
    let strategy = create_book_strategy(BookStrategy::Custom, Some(config));
    let context = BookNamingContext {
        series_name: "One Piece".to_string(),
        book_number: None,
        volume: None,
        chapter_number: None,
        total_books: 100,
    };

    let title = strategy.resolve_title("One_Piece_v012_c145.cbz", None, &context);

    assert_eq!(title, "One_Piece v.012 c.145");
    assert_eq!(strategy.strategy_type(), BookStrategy::Custom);
}

/// Test custom strategy with title group extraction
#[test]
fn test_custom_strategy_title_extraction() {
    let config = r#"{"pattern":"^(?P<series>.+?) - (?P<volume>\\d+)x(?P<chapter>\\d+) - (?P<title>.+)$","fallback":"filename"}"#;
    let strategy = create_book_strategy(BookStrategy::Custom, Some(config));
    let context = BookNamingContext {
        series_name: "Series".to_string(),
        book_number: None,
        volume: None,
        chapter_number: None,
        total_books: 50,
    };

    let title = strategy.resolve_title("One Piece - 01x05 - Romance Dawn.cbz", None, &context);

    assert_eq!(title, "Romance Dawn");
}

/// Test custom strategy fallback when pattern doesn't match
#[test]
fn test_custom_strategy_fallback_on_no_match() {
    let config = r#"{"pattern":"(?P<series>.+?)_v(?P<volume>\\d+)_c(?P<chapter>\\d+)","titleTemplate":"{series} v.{volume} c.{chapter}","fallback":"filename"}"#;
    let strategy = create_book_strategy(BookStrategy::Custom, Some(config));
    let context = BookNamingContext {
        series_name: "Test".to_string(),
        book_number: None,
        volume: None,
        chapter_number: None,
        total_books: 10,
    };

    // This filename doesn't match the pattern
    let title = strategy.resolve_title("random-file.cbz", None, &context);

    assert_eq!(title, "random-file");
}

/// Test custom strategy with metadata_first fallback
#[test]
fn test_custom_strategy_metadata_first_fallback() {
    let config = r#"{"pattern":"(?P<series>.+?)_v(?P<volume>\\d+)","fallback":"metadata_first"}"#;
    let strategy = create_book_strategy(BookStrategy::Custom, Some(config));
    let context = BookNamingContext {
        series_name: "Test".to_string(),
        book_number: None,
        volume: None,
        chapter_number: None,
        total_books: 10,
    };
    let metadata = BookMetadata {
        title: Some("The Dark Knight".to_string()),
        number: Some(1.0),
        volume: None,
        chapter: None,
    };

    // Filename doesn't match pattern, should use metadata
    let title = strategy.resolve_title("random-file.cbz", Some(&metadata), &context);

    assert_eq!(title, "The Dark Knight");
}

/// Test custom strategy with scanlation group pattern
#[test]
fn test_custom_strategy_scanlation_pattern() {
    let config = r#"{"pattern":"^\\[[^\\]]+\\]\\s*(?P<series>.+?)\\s+v(?P<volume>\\d+)\\s+c(?P<chapter>\\d+)","titleTemplate":"{series} v.{volume} c.{chapter}","fallback":"filename"}"#;
    let strategy = create_book_strategy(BookStrategy::Custom, Some(config));
    let context = BookNamingContext {
        series_name: "Test".to_string(),
        book_number: None,
        volume: None,
        chapter_number: None,
        total_books: 100,
    };

    let title = strategy.resolve_title("[GroupName] One Piece v01 c001.cbz", None, &context);

    assert_eq!(title, "One Piece v.01 c.001");
}

/// Test custom strategy with default config
#[test]
fn test_custom_strategy_default_config() {
    // When no config is provided, should use sensible defaults
    let strategy = create_book_strategy(BookStrategy::Custom, None);
    let context = BookNamingContext {
        series_name: "Test".to_string(),
        book_number: None,
        volume: None,
        chapter_number: None,
        total_books: 10,
    };

    let title = strategy.resolve_title("test-file.cbz", None, &context);

    // Default pattern matches everything as title, so should use filename
    assert_eq!(title, "test-file");
}

/// Test custom strategy with {filename} placeholder
#[test]
fn test_custom_strategy_filename_placeholder() {
    let config = r#"{"pattern":"(?P<series>.+?)_v(?P<volume>\\d+)","titleTemplate":"{series} - {filename}","fallback":"filename"}"#;
    let strategy = create_book_strategy(BookStrategy::Custom, Some(config));
    let context = BookNamingContext {
        series_name: "Test".to_string(),
        book_number: None,
        volume: None,
        chapter_number: None,
        total_books: 10,
    };

    let title = strategy.resolve_title("One_Piece_v01.cbz", None, &context);

    assert_eq!(title, "One_Piece - One_Piece_v01");
}

/// Test custom strategy config persistence
#[tokio::test]
async fn test_custom_book_strategy_persistence() {
    use codex::db::repositories::LibraryRepository;
    use codex::db::repositories::library::CreateLibraryParams;

    let (db, _temp_dir) = setup_test_db().await;

    let config = r#"{"pattern":"(?P<series>.+?)_v(?P<volume>\\d+)_c(?P<chapter>\\d+)","titleTemplate":"{series} v.{volume} c.{chapter}","fallback":"filename"}"#;
    let config_value: serde_json::Value = serde_json::from_str(config).unwrap();
    let params = CreateLibraryParams::new("Custom Library", "/test/path")
        .with_book_strategy(BookStrategy::Custom)
        .with_book_config(Some(config_value.clone()));

    let library = LibraryRepository::create_with_params(&db, params)
        .await
        .unwrap();

    assert_eq!(library.book_strategy, "custom");
    assert_eq!(library.book_config, Some(config_value));

    // Verify the config can be used to create a strategy
    let config_str = library.book_config.as_ref().map(|v| v.to_string());
    let strategy = create_book_strategy(BookStrategy::Custom, config_str.as_deref());
    assert_eq!(strategy.strategy_type(), BookStrategy::Custom);

    // Test that the strategy works correctly
    let context = BookNamingContext {
        series_name: "Test".to_string(),
        book_number: None,
        volume: None,
        chapter_number: None,
        total_books: 100,
    };
    let title = strategy.resolve_title("One_Piece_v012_c145.cbz", None, &context);
    assert_eq!(title, "One_Piece v.012 c.145");
}

// ============================================================================
// Per-book volume/chapter classification (Phase 12 of metadata-count-split)
// ============================================================================
//
// These tests cover the strategy x parse-case matrix the scanner now relies on
// to populate `book_metadata.volume` / `book_metadata.chapter`. They verify the
// strategy contract directly (the scanner glue is a thin call into
// `resolve_volume` / `resolve_chapter`); a full end-to-end scan test would only
// re-cover library wiring already tested elsewhere.

fn classification_context() -> BookNamingContext {
    BookNamingContext {
        series_name: "Test Series".to_string(),
        book_number: None,
        volume: None,
        chapter_number: None,
        total_books: 50,
    }
}

/// Filename strategy: structured parse from filename, ComicInfo ignored.
#[test]
fn test_filename_strategy_resolves_volume_chapter() {
    let strategy = create_book_strategy(BookStrategy::Filename, None);
    let ctx = classification_context();
    // ComicInfo says volume=99 chapter=999, but Filename strategy ignores it.
    let metadata = BookMetadata {
        title: None,
        number: None,
        volume: Some(99),
        chapter: Some(999.0),
    };

    assert_eq!(
        strategy.resolve_volume("Series v15 - c126.cbz", Some(&metadata), &ctx),
        Some(15)
    );
    assert_eq!(
        strategy.resolve_chapter("Series v15 - c126.cbz", Some(&metadata), &ctx),
        Some(126.0)
    );
}

/// MetadataFirst strategy: ComicInfo only, filename never parsed.
#[test]
fn test_metadata_first_strategy_uses_only_comic_info() {
    let strategy = create_book_strategy(BookStrategy::MetadataFirst, None);
    let ctx = classification_context();
    // Filename has v15 - c126, but MetadataFirst defers to ComicInfo.
    let metadata = BookMetadata {
        title: None,
        number: None,
        volume: Some(7),
        chapter: Some(42.0),
    };

    assert_eq!(
        strategy.resolve_volume("Series v15 - c126.cbz", Some(&metadata), &ctx),
        Some(7)
    );
    assert_eq!(
        strategy.resolve_chapter("Series v15 - c126.cbz", Some(&metadata), &ctx),
        Some(42.0)
    );

    // No ComicInfo: returns None for both.
    assert_eq!(
        strategy.resolve_volume("Series v15 - c126.cbz", None, &ctx),
        None
    );
    assert_eq!(
        strategy.resolve_chapter("Series v15 - c126.cbz", None, &ctx),
        None
    );
}

/// Smart strategy: ComicInfo first, filename fallback when ComicInfo silent.
#[test]
fn test_smart_strategy_falls_back_to_filename() {
    let strategy = create_book_strategy(BookStrategy::Smart, None);
    let ctx = classification_context();

    // ComicInfo populated -> takes precedence.
    let with_meta = BookMetadata {
        title: None,
        number: None,
        volume: Some(7),
        chapter: Some(42.0),
    };
    assert_eq!(
        strategy.resolve_volume("Series v15 - c126.cbz", Some(&with_meta), &ctx),
        Some(7)
    );
    assert_eq!(
        strategy.resolve_chapter("Series v15 - c126.cbz", Some(&with_meta), &ctx),
        Some(42.0)
    );

    // ComicInfo silent on volume -> filename fallback fills in.
    let chapter_only = BookMetadata {
        title: None,
        number: None,
        volume: None,
        chapter: Some(42.0),
    };
    assert_eq!(
        strategy.resolve_volume("Series v15 - c126.cbz", Some(&chapter_only), &ctx),
        Some(15)
    );
    assert_eq!(
        strategy.resolve_chapter("Series v15 - c126.cbz", Some(&chapter_only), &ctx),
        Some(42.0)
    );

    // No ComicInfo at all -> filename is the only source.
    assert_eq!(
        strategy.resolve_volume("Series v15 - c126.cbz", None, &ctx),
        Some(15)
    );
    assert_eq!(
        strategy.resolve_chapter("Series v15 - c126.cbz", None, &ctx),
        Some(126.0)
    );
}

/// SeriesName strategy: passes through whatever series detection populated on
/// the context. No detection -> None.
#[test]
fn test_series_name_strategy_passes_through_context() {
    let strategy = create_book_strategy(BookStrategy::SeriesName, None);

    // Detection wrote volume + chapter into the context.
    let with_detection = BookNamingContext {
        series_name: "Series".to_string(),
        book_number: None,
        volume: Some("Volume 12".to_string()),
        chapter_number: Some(126.0),
        total_books: 200,
    };
    assert_eq!(
        strategy.resolve_volume("Series v99 - c999.cbz", None, &with_detection),
        Some(12)
    );
    assert_eq!(
        strategy.resolve_chapter("Series v99 - c999.cbz", None, &with_detection),
        Some(126.0)
    );

    // No detection on context -> None on both axes (filename ignored).
    let without = classification_context();
    assert_eq!(
        strategy.resolve_volume("Series v15 - c126.cbz", None, &without),
        None
    );
    assert_eq!(
        strategy.resolve_chapter("Series v15 - c126.cbz", None, &without),
        None
    );
}

/// Custom strategy: extracts named groups from the user's regex.
#[test]
fn test_custom_strategy_resolves_volume_chapter_from_named_groups() {
    let config = r#"{"pattern":"(?P<series>.+?)_v(?P<volume>\\d+)_c(?P<chapter>\\d+)","fallback":"filename"}"#;
    let strategy = create_book_strategy(BookStrategy::Custom, Some(config));
    let ctx = classification_context();

    assert_eq!(
        strategy.resolve_volume("One_Piece_v012_c145.cbz", None, &ctx),
        Some(12)
    );
    assert_eq!(
        strategy.resolve_chapter("One_Piece_v012_c145.cbz", None, &ctx),
        Some(145.0)
    );

    // Pattern doesn't match -> None on both.
    assert_eq!(strategy.resolve_volume("random.cbz", None, &ctx), None);
    assert_eq!(strategy.resolve_chapter("random.cbz", None, &ctx), None);
}
