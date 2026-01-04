#[path = "../common/mod.rs"]
mod common;

use codex::parsers::cbz::{CbzParser, extract_page_from_cbz};
use codex::parsers::traits::FormatParser;
use codex::parsers::{FileFormat, ImageFormat};
use tempfile::TempDir;

#[test]
fn test_cbz_parser_can_parse() {
    let parser = CbzParser::new();

    assert!(parser.can_parse("test.cbz"));
    assert!(parser.can_parse("test.CBZ"));
    assert!(parser.can_parse("/path/to/file.cbz"));

    assert!(!parser.can_parse("test.cbr"));
    assert!(!parser.can_parse("test.epub"));
    assert!(!parser.can_parse("test.pdf"));
    assert!(!parser.can_parse("test.txt"));
}

#[test]
fn test_cbz_parser_parse_basic() {
    let temp_dir = TempDir::new().unwrap();
    let cbz_path = common::create_test_cbz(&temp_dir, 3, false);

    let parser = CbzParser::new();
    let metadata = parser.parse(&cbz_path).unwrap();

    assert_eq!(metadata.format, FileFormat::CBZ);
    assert_eq!(metadata.page_count, 3);
    assert_eq!(metadata.pages.len(), 3);
    assert!(metadata.file_hash.len() == 64); // SHA-256 hash length
    assert!(metadata.comic_info.is_none());

    // Check pages are in order
    assert_eq!(metadata.pages[0].page_number, 1);
    assert_eq!(metadata.pages[1].page_number, 2);
    assert_eq!(metadata.pages[2].page_number, 3);

    // Check page filenames
    assert_eq!(metadata.pages[0].file_name, "page001.png");
    assert_eq!(metadata.pages[1].file_name, "page002.png");
    assert_eq!(metadata.pages[2].file_name, "page003.png");

    // Check image format
    assert_eq!(metadata.pages[0].format, ImageFormat::PNG);
}

#[test]
fn test_cbz_parser_parse_with_comic_info() {
    let temp_dir = TempDir::new().unwrap();
    let cbz_path = common::create_test_cbz(&temp_dir, 3, true);

    let parser = CbzParser::new();
    let metadata = parser.parse(&cbz_path).unwrap();

    assert_eq!(metadata.page_count, 3);
    assert!(metadata.comic_info.is_some());

    let comic_info = metadata.comic_info.unwrap();
    assert_eq!(comic_info.title, Some("Test Comic".to_string()));
    assert_eq!(comic_info.series, Some("Test Series".to_string()));
    assert_eq!(comic_info.number, Some("1".to_string()));
    assert_eq!(comic_info.volume, Some(1));
    assert_eq!(comic_info.writer, Some("Test Writer".to_string()));
    assert_eq!(comic_info.publisher, Some("Test Publisher".to_string()));
    assert_eq!(comic_info.year, Some(2024));
    assert_eq!(comic_info.page_count, Some(3));
}

#[test]
fn test_cbz_parser_parse_page_dimensions() {
    let temp_dir = TempDir::new().unwrap();
    let cbz_path = common::create_test_cbz(&temp_dir, 3, false);

    let parser = CbzParser::new();
    let metadata = parser.parse(&cbz_path).unwrap();

    // Our test PNG is 10x10
    for page in &metadata.pages {
        assert_eq!(page.width, 10);
        assert_eq!(page.height, 10);
    }
}

#[test]
fn test_cbz_parser_parse_nonexistent_file() {
    let parser = CbzParser::new();
    let result = parser.parse("/nonexistent/file.cbz");

    assert!(result.is_err());
}

#[test]
fn test_cbz_parser_default() {
    let parser1 = CbzParser::new();
    let parser2 = CbzParser::default();

    // Both should be able to parse CBZ files
    assert!(parser1.can_parse("test.cbz"));
    assert!(parser2.can_parse("test.cbz"));
}

#[test]
fn test_extract_page_from_cbz_first_page() {
    let temp_dir = TempDir::new().unwrap();
    let cbz_path = common::create_test_cbz(&temp_dir, 3, false);

    let image_data = extract_page_from_cbz(&cbz_path, 1).unwrap();

    // Should return valid image data
    assert!(!image_data.is_empty());

    // Check it's a valid PNG (starts with PNG magic bytes)
    assert_eq!(&image_data[0..4], b"\x89PNG");
}

#[test]
fn test_extract_page_from_cbz_last_page() {
    let temp_dir = TempDir::new().unwrap();
    let cbz_path = common::create_test_cbz(&temp_dir, 3, false);

    let image_data = extract_page_from_cbz(&cbz_path, 3).unwrap();

    // Should return valid image data
    assert!(!image_data.is_empty());
    assert_eq!(&image_data[0..4], b"\x89PNG");
}

#[test]
fn test_extract_page_from_cbz_middle_page() {
    let temp_dir = TempDir::new().unwrap();
    let cbz_path = common::create_test_cbz(&temp_dir, 5, false);

    let image_data = extract_page_from_cbz(&cbz_path, 3).unwrap();

    // Should return valid image data
    assert!(!image_data.is_empty());
    assert_eq!(&image_data[0..4], b"\x89PNG");
}

#[test]
fn test_extract_page_from_cbz_invalid_page_number() {
    let temp_dir = TempDir::new().unwrap();
    let cbz_path = common::create_test_cbz(&temp_dir, 3, false);

    // Page 0 should fail (1-indexed)
    let result = extract_page_from_cbz(&cbz_path, 0);
    assert!(result.is_err());

    // Page beyond count should fail
    let result = extract_page_from_cbz(&cbz_path, 4);
    assert!(result.is_err());

    // Negative page should fail
    let result = extract_page_from_cbz(&cbz_path, -1);
    assert!(result.is_err());
}

#[test]
fn test_extract_page_from_cbz_nonexistent_file() {
    let result = extract_page_from_cbz("/nonexistent/file.cbz", 1);
    assert!(result.is_err());
}
