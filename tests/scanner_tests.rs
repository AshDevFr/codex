mod common;

use codex::scanner::{analyze_file, detect_format};
use codex::parsers::FileFormat;
use tempfile::TempDir;

#[test]
fn test_detect_format_integration() {
    let temp_dir = TempDir::new().unwrap();
    let cbz_path = common::create_test_cbz(&temp_dir, 1, false);

    let format = detect_format(&cbz_path);
    assert_eq!(format, Some(FileFormat::CBZ));
}

#[test]
fn test_analyze_file_cbz() {
    let temp_dir = TempDir::new().unwrap();
    let cbz_path = common::create_test_cbz(&temp_dir, 1, false);

    let result = analyze_file(&cbz_path);
    assert!(result.is_ok());

    let metadata = result.unwrap();
    assert_eq!(metadata.format, FileFormat::CBZ);
    assert_eq!(metadata.page_count, 1);
    assert!(metadata.file_size > 0);
    assert_eq!(metadata.file_hash.len(), 64);
}

#[test]
fn test_analyze_file_unsupported_format() {
    let temp_dir = TempDir::new().unwrap();
    let txt_path = temp_dir.path().join("test.txt");

    std::fs::write(&txt_path, "This is a test file").unwrap();

    let result = analyze_file(&txt_path);
    assert!(result.is_err());
}

#[test]
fn test_analyze_file_nonexistent() {
    let result = analyze_file("/nonexistent/path/to/file.cbz");
    assert!(result.is_err());
}

#[test]
fn test_analyze_file_epub() {
    let temp_dir = TempDir::new().unwrap();
    let epub_path = common::create_test_epub(&temp_dir, 3, 2);

    let result = analyze_file(&epub_path);
    assert!(result.is_ok());

    let metadata = result.unwrap();
    assert_eq!(metadata.format, FileFormat::EPUB);
    assert_eq!(metadata.page_count, 3); // 3 chapters
    assert_eq!(metadata.pages.len(), 2); // 2 images
    assert!(metadata.file_size > 0);
    assert_eq!(metadata.file_hash.len(), 64);
}

#[test]
fn test_detect_format_epub() {
    let temp_dir = TempDir::new().unwrap();
    let epub_path = common::create_test_epub(&temp_dir, 1, 1);

    let format = detect_format(&epub_path);
    assert_eq!(format, Some(FileFormat::EPUB));
}

#[test]
fn test_analyze_file_pdf() {
    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_test_pdf(&temp_dir, 3, 0);

    let result = analyze_file(&pdf_path);
    assert!(result.is_ok());

    let metadata = result.unwrap();
    assert_eq!(metadata.format, FileFormat::PDF);
    assert_eq!(metadata.page_count, 3);
    assert!(metadata.file_size > 0);
    assert_eq!(metadata.file_hash.len(), 64);
}

#[test]
fn test_detect_format_pdf() {
    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_test_pdf(&temp_dir, 1, 0);

    let format = detect_format(&pdf_path);
    assert_eq!(format, Some(FileFormat::PDF));
}
