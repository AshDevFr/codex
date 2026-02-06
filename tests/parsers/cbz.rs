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
    let parser2 = CbzParser;

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

#[test]
fn test_cbz_parser_skips_macos_resource_forks() {
    use std::fs::File;
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::FileOptions;

    let temp_dir = TempDir::new().unwrap();
    let cbz_path = temp_dir.path().join("macos_test.cbz");

    // Create a CBZ with macOS resource fork files
    let file = File::create(&cbz_path).unwrap();
    let mut zip = ZipWriter::new(file);

    let options: FileOptions<'_, ()> =
        FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // Add actual image pages
    for i in 1..=3 {
        let page_data = common::create_test_png(10, 10);
        let filename = format!("page{:03}.jpg", i);
        zip.start_file(&filename, options).unwrap();
        zip.write_all(&page_data).unwrap();
    }

    // Add macOS resource fork files (these look like images by extension but aren't)
    // __MACOSX directory files
    let macos_metadata = b"\x00\x05\x16\x07Mac OS X    ATTR";
    zip.start_file("__MACOSX/._page001.jpg", options).unwrap();
    zip.write_all(macos_metadata).unwrap();

    zip.start_file("__MACOSX/._page002.jpg", options).unwrap();
    zip.write_all(macos_metadata).unwrap();

    // AppleDouble file at root level
    zip.start_file("._page003.jpg", options).unwrap();
    zip.write_all(macos_metadata).unwrap();

    zip.finish().unwrap();

    // Parse the CBZ - should only find 3 pages, not 6
    let parser = CbzParser::new();
    let metadata = parser.parse(&cbz_path).unwrap();

    assert_eq!(
        metadata.page_count, 3,
        "Should skip macOS resource fork files"
    );
    assert_eq!(metadata.pages.len(), 3);

    // Verify all pages are actual images
    for page in &metadata.pages {
        assert!(!page.file_name.starts_with("._"));
        assert!(!page.file_name.contains("__MACOSX"));
    }
}

#[test]
fn test_cbz_parser_skips_non_image_files_with_image_extensions() {
    use std::fs::File;
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::FileOptions;

    let temp_dir = TempDir::new().unwrap();
    let cbz_path = temp_dir.path().join("fake_images.cbz");

    let file = File::create(&cbz_path).unwrap();
    let mut zip = ZipWriter::new(file);

    let options: FileOptions<'_, ()> =
        FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // Add actual image pages
    for i in 1..=2 {
        let page_data = common::create_test_png(10, 10);
        let filename = format!("page{:03}.png", i);
        zip.start_file(&filename, options).unwrap();
        zip.write_all(&page_data).unwrap();
    }

    // Add a file with .jpg extension but non-image content (e.g., text)
    zip.start_file("fake_image.jpg", options).unwrap();
    zip.write_all(b"This is not a real image file").unwrap();

    // Add a file with .png extension but random binary content
    zip.start_file("corrupted.png", options).unwrap();
    zip.write_all(&[0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC])
        .unwrap();

    zip.finish().unwrap();

    // Parse the CBZ - should only find 2 valid pages
    let parser = CbzParser::new();
    let metadata = parser.parse(&cbz_path).unwrap();

    assert_eq!(
        metadata.page_count, 2,
        "Should skip files that don't pass image format verification"
    );
    assert_eq!(metadata.pages.len(), 2);

    // Verify page numbers are sequential starting from 1
    assert_eq!(metadata.pages[0].page_number, 1);
    assert_eq!(metadata.pages[1].page_number, 2);
}

#[test]
fn test_extract_page_with_fallback_skips_corrupted_first_image() {
    use codex::parsers::cbz::extract_page_from_cbz_with_fallback;
    use std::fs::File;
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::FileOptions;

    let temp_dir = TempDir::new().unwrap();
    let cbz_path = temp_dir.path().join("corrupted_first.cbz");

    let file = File::create(&cbz_path).unwrap();
    let mut zip = ZipWriter::new(file);

    let options: FileOptions<'_, ()> =
        FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // Add a corrupted image first (alphabetically first due to 'a' prefix)
    // This simulates a corrupted cover image with null bytes
    zip.start_file("a_cover.jpg", options).unwrap();
    zip.write_all(&[0u8; 100]).unwrap(); // 100 null bytes

    // Add valid images after
    let valid_png = common::create_test_png(10, 10);
    zip.start_file("b_page001.png", options).unwrap();
    zip.write_all(&valid_png).unwrap();

    zip.start_file("c_page002.png", options).unwrap();
    zip.write_all(&valid_png).unwrap();

    zip.finish().unwrap();

    // Extract page 1 without fallback should fail (first image is corrupted)
    let result = extract_page_from_cbz_with_fallback(&cbz_path, 1, false);
    assert!(
        result.is_err(),
        "Should fail without fallback when first image is corrupted"
    );

    // Extract page 1 with fallback should succeed (skips corrupted, uses next valid)
    let result = extract_page_from_cbz_with_fallback(&cbz_path, 1, true);
    assert!(result.is_ok(), "Should succeed with fallback enabled");

    // The returned data should be the valid PNG (starts with PNG magic bytes)
    let data = result.unwrap();
    assert_eq!(&data[0..4], b"\x89PNG", "Should return valid PNG data");
}

#[test]
fn test_extract_page_with_fallback_fails_when_all_images_corrupted() {
    use codex::parsers::cbz::extract_page_from_cbz_with_fallback;
    use std::fs::File;
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::FileOptions;

    let temp_dir = TempDir::new().unwrap();
    let cbz_path = temp_dir.path().join("all_corrupted.cbz");

    let file = File::create(&cbz_path).unwrap();
    let mut zip = ZipWriter::new(file);

    let options: FileOptions<'_, ()> =
        FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // Add only corrupted images
    zip.start_file("page001.jpg", options).unwrap();
    zip.write_all(&[0u8; 100]).unwrap(); // null bytes

    zip.start_file("page002.jpg", options).unwrap();
    zip.write_all(b"not an image").unwrap(); // text content

    zip.start_file("page003.png", options).unwrap();
    zip.write_all(&[0x12, 0x34, 0x56, 0x78]).unwrap(); // random bytes

    zip.finish().unwrap();

    // Even with fallback, should fail when all images are corrupted
    let result = extract_page_from_cbz_with_fallback(&cbz_path, 1, true);
    assert!(result.is_err(), "Should fail when all images are corrupted");
    assert!(result.unwrap_err().to_string().contains("No valid images"));
}
