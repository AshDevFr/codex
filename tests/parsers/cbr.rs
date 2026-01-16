#![cfg(feature = "rar")]

use codex::parsers::cbr::CbrParser;
use codex::parsers::traits::FormatParser;
use codex::parsers::FileFormat;
use std::path::Path;

#[test]
fn test_cbr_parser_can_parse() {
    let parser = CbrParser::new();

    assert!(parser.can_parse("test.cbr"));
    assert!(parser.can_parse("test.CBR"));
    assert!(parser.can_parse("/path/to/file.cbr"));

    assert!(!parser.can_parse("test.cbz"));
    assert!(!parser.can_parse("test.epub"));
    assert!(!parser.can_parse("test.pdf"));
    assert!(!parser.can_parse("test.txt"));
}

#[test]
fn test_cbr_parser_parse_nonexistent_file() {
    let parser = CbrParser::new();
    let result = parser.parse("/nonexistent/file.cbr");

    assert!(result.is_err());
}

#[test]
fn test_cbr_parser_default() {
    let parser1 = CbrParser::new();
    let parser2 = CbrParser;

    // Both should be able to parse CBR files
    assert!(parser1.can_parse("test.cbr"));
    assert!(parser2.can_parse("test.cbr"));
}

// Note: Full integration tests with actual CBR files require manually created test files
// due to UnRAR license restrictions (extraction only, no creation).
//
// To test with a real CBR file:
// 1. Manually create a CBR file using WinRAR or another RAR tool
// 2. Place it in tests/fixtures/test_comic.cbr
// 3. Add test images (page001.png, page002.png, etc.) to the archive
// 4. Optionally add ComicInfo.xml
// 5. Run: cargo test --features rar test_cbr_parser_parse_real_file -- --ignored
#[test]
#[ignore]
fn test_cbr_parser_parse_real_file() {
    let test_file = Path::new("tests/fixtures/test_comic.cbr");

    assert!(
        test_file.exists(),
        "Test fixture not found: tests/fixtures/test_comic.cbr\n\
         Create a test CBR file manually to run this test.\n\
         See tests/fixtures/README.md for instructions."
    );

    let parser = CbrParser::new();
    let metadata = parser.parse(test_file).expect("Failed to parse CBR file");

    assert_eq!(metadata.format, FileFormat::CBR);
    assert!(metadata.page_count > 0);
    assert_eq!(metadata.pages.len(), metadata.page_count);
    assert!(metadata.file_hash.len() == 64); // SHA-256 hash length

    // Check pages are numbered correctly
    for (idx, page) in metadata.pages.iter().enumerate() {
        assert_eq!(page.page_number, idx + 1);
    }
}

#[test]
#[ignore]
fn test_cbr_parser_parse_with_comic_info() {
    let test_file = Path::new("tests/fixtures/test_comic_with_info.cbr");

    assert!(
        test_file.exists(),
        "Test fixture not found: tests/fixtures/test_comic_with_info.cbr\n\
         Create a test CBR file with ComicInfo.xml manually to run this test.\n\
         See tests/fixtures/README.md for instructions."
    );

    let parser = CbrParser::new();
    let metadata = parser.parse(test_file).expect("Failed to parse CBR file");

    assert_eq!(metadata.format, FileFormat::CBR);
    assert!(
        metadata.comic_info.is_some(),
        "ComicInfo.xml should be present"
    );

    let comic_info = metadata.comic_info.unwrap();
    // These assertions will depend on what's in your test file's ComicInfo.xml
    assert!(comic_info.title.is_some() || comic_info.series.is_some());
}
