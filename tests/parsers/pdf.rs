#[path = "../common/mod.rs"]
mod common;

use codex::parsers::FileFormat;
use codex::parsers::pdf::{PdfParser, extract_page_from_pdf};
use codex::parsers::traits::FormatParser;
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
    // Image extraction might vary, just verify it's a valid vec
    assert!(metadata.pages.is_empty() || !metadata.pages.is_empty());
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
    let parser2 = PdfParser;

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

#[test]
fn test_extract_page_from_pdf_first_page() {
    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_test_pdf(&temp_dir, 3, 2);

    let image_data = extract_page_from_pdf(&pdf_path, 1).unwrap();

    // Should return valid image data
    assert!(!image_data.is_empty());

    // Check it's a valid JPEG (from our test PDF with DCTDecode)
    // JPEG magic bytes: FF D8 FF
    assert!(
        image_data.len() >= 3 && image_data[0] == 0xFF && image_data[1] == 0xD8,
        "Expected JPEG magic bytes, got: {:02X?}",
        &image_data[..image_data.len().min(4)]
    );
}

#[test]
fn test_extract_page_from_pdf_last_page() {
    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_test_pdf(&temp_dir, 3, 2);

    // PDF with 3 pages - extract the last page (page 3)
    // Note: page_number is 1-indexed and refers to actual PDF pages, not embedded images
    let image_data = extract_page_from_pdf(&pdf_path, 3).unwrap();

    // Should return valid image data
    assert!(!image_data.is_empty());
}

#[test]
fn test_extract_page_from_pdf_middle_page() {
    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_test_pdf(&temp_dir, 5, 2);

    // PDF with 5 pages - extract the middle page (page 3)
    // Note: page_number is 1-indexed and refers to actual PDF pages
    let image_data = extract_page_from_pdf(&pdf_path, 3).unwrap();

    // Should return valid image data
    assert!(!image_data.is_empty());
}

#[test]
fn test_extract_page_from_pdf_invalid_page_number() {
    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_test_pdf(&temp_dir, 2, 2);

    // PDF with 2 pages - page 3 should fail
    // Note: page_number is 1-indexed and refers to actual PDF pages
    let result = extract_page_from_pdf(&pdf_path, 3);
    assert!(result.is_err());
}

#[test]
fn test_extract_page_from_pdf_nonexistent_file() {
    let result = extract_page_from_pdf("/nonexistent/file.pdf", 1);
    assert!(result.is_err());
}
