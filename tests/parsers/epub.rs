#[path = "../common/mod.rs"]
mod common;

use codex::parsers::epub::{extract_page_from_epub, EpubParser};
use codex::parsers::traits::FormatParser;
use codex::parsers::{FileFormat, ImageFormat};
use tempfile::TempDir;

#[test]
fn test_epub_parser_can_parse() {
    let parser = EpubParser::new();

    assert!(parser.can_parse("test.epub"));
    assert!(parser.can_parse("test.EPUB"));
    assert!(parser.can_parse("/path/to/file.epub"));

    assert!(!parser.can_parse("test.cbz"));
    assert!(!parser.can_parse("test.cbr"));
    assert!(!parser.can_parse("test.pdf"));
    assert!(!parser.can_parse("test.txt"));
}

#[test]
fn test_epub_parser_parse_basic() {
    let temp_dir = TempDir::new().unwrap();
    let epub_path = common::create_test_epub(&temp_dir, 3, 2);

    let parser = EpubParser::new();
    let metadata = parser.parse(&epub_path).unwrap();

    assert_eq!(metadata.format, FileFormat::EPUB);
    // Page count should be max of spine (chapters) or images
    assert!(metadata.page_count >= 2);
    assert_eq!(metadata.pages.len(), 2); // We have 2 images
    assert!(metadata.file_hash.len() == 64); // SHA-256 hash length
    assert!(metadata.comic_info.is_none()); // EPUB doesn't have ComicInfo.xml
}

#[test]
fn test_epub_parser_parse_chapters_and_images() {
    let temp_dir = TempDir::new().unwrap();
    let epub_path = common::create_test_epub(&temp_dir, 5, 3);

    let parser = EpubParser::new();
    let metadata = parser.parse(&epub_path).unwrap();

    // Should have 5 chapters in spine
    assert_eq!(metadata.page_count, 5);
    // Should have 3 images
    assert_eq!(metadata.pages.len(), 3);
}

#[test]
fn test_epub_parser_parse_images_only() {
    let temp_dir = TempDir::new().unwrap();
    let epub_path = common::create_test_epub(&temp_dir, 0, 4);

    let parser = EpubParser::new();
    let metadata = parser.parse(&epub_path).unwrap();

    // With no chapters but 4 images, page_count should be at least 4
    assert!(metadata.page_count >= 4);
    assert_eq!(metadata.pages.len(), 4);
}

#[test]
fn test_epub_parser_parse_page_info() {
    let temp_dir = TempDir::new().unwrap();
    let epub_path = common::create_test_epub(&temp_dir, 2, 3);

    let parser = EpubParser::new();
    let metadata = parser.parse(&epub_path).unwrap();

    assert_eq!(metadata.pages.len(), 3);

    // Check pages are numbered correctly
    for (idx, page) in metadata.pages.iter().enumerate() {
        assert_eq!(page.page_number, idx + 1);
        assert!(page.file_name.contains("image"));
        assert!(page.file_name.ends_with(".png"));
        assert_eq!(page.format, ImageFormat::PNG);
        assert_eq!(page.width, 10);
        assert_eq!(page.height, 10);
    }
}

#[test]
fn test_epub_parser_parse_page_dimensions() {
    let temp_dir = TempDir::new().unwrap();
    let epub_path = common::create_test_epub(&temp_dir, 1, 2);

    let parser = EpubParser::new();
    let metadata = parser.parse(&epub_path).unwrap();

    // Our test PNG is 10x10
    for page in &metadata.pages {
        assert_eq!(page.width, 10);
        assert_eq!(page.height, 10);
    }
}

#[test]
fn test_epub_parser_parse_nonexistent_file() {
    let parser = EpubParser::new();
    let result = parser.parse("/nonexistent/file.epub");

    assert!(result.is_err());
}

#[test]
fn test_epub_parser_parse_invalid_epub() {
    let temp_dir = TempDir::new().unwrap();

    // Create a file that's not a valid EPUB (just a simple text file)
    let invalid_path = temp_dir.path().join("invalid.epub");
    std::fs::write(&invalid_path, b"This is not an EPUB file").unwrap();

    let parser = EpubParser::new();
    let result = parser.parse(&invalid_path);

    // Should fail to parse
    assert!(result.is_err());
}

#[test]
fn test_epub_parser_default() {
    let parser1 = EpubParser::new();
    let parser2 = EpubParser;

    // Both should be able to parse EPUB files
    assert!(parser1.can_parse("test.epub"));
    assert!(parser2.can_parse("test.epub"));
}

#[test]
fn test_epub_parser_with_many_chapters() {
    let temp_dir = TempDir::new().unwrap();
    let epub_path = common::create_test_epub(&temp_dir, 20, 5);

    let parser = EpubParser::new();
    let metadata = parser.parse(&epub_path).unwrap();

    // Should have 20 chapters
    assert_eq!(metadata.page_count, 20);
    // Should have 5 images
    assert_eq!(metadata.pages.len(), 5);
}

#[test]
fn test_epub_parser_file_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let epub_path = common::create_test_epub(&temp_dir, 3, 2);

    let parser = EpubParser::new();
    let metadata = parser.parse(&epub_path).unwrap();

    // Check file metadata is populated
    assert!(metadata.file_size > 0);
    assert!(!metadata.file_hash.is_empty());
    assert!(metadata.file_hash.len() == 64); // SHA-256
    assert!(!metadata.file_path.is_empty());
}

#[test]
fn test_extract_page_from_epub_first_page() {
    let temp_dir = TempDir::new().unwrap();
    let epub_path = common::create_test_epub(&temp_dir, 2, 3);

    let image_data = extract_page_from_epub(&epub_path, 1).unwrap();

    // Should return valid image data
    assert!(!image_data.is_empty());

    // Check it's a valid PNG
    assert_eq!(&image_data[0..4], b"\x89PNG");
}

#[test]
fn test_extract_page_from_epub_last_page() {
    let temp_dir = TempDir::new().unwrap();
    let epub_path = common::create_test_epub(&temp_dir, 2, 3);

    let image_data = extract_page_from_epub(&epub_path, 3).unwrap();

    // Should return valid image data
    assert!(!image_data.is_empty());
    assert_eq!(&image_data[0..4], b"\x89PNG");
}

#[test]
fn test_extract_page_from_epub_invalid_page_number() {
    let temp_dir = TempDir::new().unwrap();
    let epub_path = common::create_test_epub(&temp_dir, 2, 3);

    // Page beyond count should fail
    let result = extract_page_from_epub(&epub_path, 4);
    assert!(result.is_err());
}

#[test]
fn test_extract_page_from_epub_nonexistent_file() {
    let result = extract_page_from_epub("/nonexistent/file.epub", 1);
    assert!(result.is_err());
}
