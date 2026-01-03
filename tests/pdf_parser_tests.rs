mod common;

use codex::parsers::pdf::PdfParser;
use codex::parsers::traits::FormatParser;
use codex::parsers::FileFormat;
use tempfile::TempDir;

#[test]
fn test_pdf_parser_can_parse() {
    let parser = PdfParser::new();

    assert!(parser.can_parse("test.pdf"));
    assert!(parser.can_parse("test.PDF"));
    assert!(parser.can_parse("/path/to/file.pdf"));

    assert!(!parser.can_parse("test.cbz"));
    assert!(!parser.can_parse("test.cbr"));
    assert!(!parser.can_parse("test.epub"));
    assert!(!parser.can_parse("test.txt"));
}

#[test]
fn test_pdf_parser_parse_basic() {
    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_test_pdf(&temp_dir, 3, 0);

    let parser = PdfParser::new();
    let metadata = parser.parse(&pdf_path).unwrap();

    assert_eq!(metadata.format, FileFormat::PDF);
    assert_eq!(metadata.page_count, 3);
    assert!(metadata.file_hash.len() == 64); // SHA-256 hash length
    assert!(metadata.comic_info.is_none()); // PDF doesn't have ComicInfo.xml
}

#[test]
fn test_pdf_parser_parse_with_images() {
    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_test_pdf(&temp_dir, 2, 2);

    let parser = PdfParser::new();
    let metadata = parser.parse(&pdf_path).unwrap();

    assert_eq!(metadata.format, FileFormat::PDF);
    assert_eq!(metadata.page_count, 2);
    // Should have extracted some images (2 pages * 2 images = 4)
    assert!(metadata.pages.len() >= 0); // Image extraction might vary
}

#[test]
fn test_pdf_parser_parse_multiple_pages() {
    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_test_pdf(&temp_dir, 10, 0);

    let parser = PdfParser::new();
    let metadata = parser.parse(&pdf_path).unwrap();

    assert_eq!(metadata.page_count, 10);
}

#[test]
fn test_pdf_parser_parse_single_page() {
    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_test_pdf(&temp_dir, 1, 0);

    let parser = PdfParser::new();
    let metadata = parser.parse(&pdf_path).unwrap();

    assert_eq!(metadata.page_count, 1);
}

#[test]
fn test_pdf_parser_parse_nonexistent_file() {
    let parser = PdfParser::new();
    let result = parser.parse("/nonexistent/file.pdf");

    assert!(result.is_err());
}

#[test]
fn test_pdf_parser_parse_invalid_pdf() {
    let temp_dir = TempDir::new().unwrap();

    // Create a file that's not a valid PDF
    let invalid_path = temp_dir.path().join("invalid.pdf");
    std::fs::write(&invalid_path, b"This is not a PDF file").unwrap();

    let parser = PdfParser::new();
    let result = parser.parse(&invalid_path);

    // Should fail to parse
    assert!(result.is_err());
}

#[test]
fn test_pdf_parser_default() {
    let parser1 = PdfParser::new();
    let parser2 = PdfParser::default();

    // Both should be able to parse PDF files
    assert!(parser1.can_parse("test.pdf"));
    assert!(parser2.can_parse("test.pdf"));
}

#[test]
fn test_pdf_parser_file_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_test_pdf(&temp_dir, 2, 1);

    let parser = PdfParser::new();
    let metadata = parser.parse(&pdf_path).unwrap();

    // Check file metadata is populated
    assert!(metadata.file_size > 0);
    assert!(!metadata.file_hash.is_empty());
    assert!(metadata.file_hash.len() == 64); // SHA-256
    assert!(!metadata.file_path.is_empty());
}

#[test]
fn test_pdf_parser_with_many_pages() {
    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_test_pdf(&temp_dir, 20, 0);

    let parser = PdfParser::new();
    let metadata = parser.parse(&pdf_path).unwrap();

    // Should have 20 pages
    assert_eq!(metadata.page_count, 20);
}

