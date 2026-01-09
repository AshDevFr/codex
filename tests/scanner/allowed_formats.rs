#[path = "../common/mod.rs"]
mod common;

use codex::db::repositories::{BookRepository, LibraryRepository, SeriesRepository};
use codex::db::ScanningStrategy;
use codex::scanner::{scan_library, ScanMode};
use common::*;
use std::fs;
use tempfile::TempDir;

/// Helper to create a library with multiple file formats
async fn setup_library_with_mixed_formats(
    db: &sea_orm::DatabaseConnection,
    temp_dir: &TempDir,
) -> codex::db::entities::libraries::Model {
    let library_path = temp_dir.path().join("test_library");
    fs::create_dir_all(&library_path).unwrap();

    // Create series folder
    let series_path = library_path.join("Test Series");
    fs::create_dir_all(&series_path).unwrap();

    // Create test files of different formats
    // CBZ files
    for i in 1..=2 {
        let file_path = series_path.join(format!("book{}.cbz", i));
        let cbz_path = create_test_cbz(temp_dir, i, false);
        fs::copy(&cbz_path, &file_path).unwrap();
    }

    // EPUB files
    for i in 1..=2 {
        let file_path = series_path.join(format!("book{}.epub", i));
        let epub_path = create_test_epub(temp_dir, 1, 1);
        fs::copy(&epub_path, &file_path).unwrap();
    }

    // PDF files
    for i in 1..=2 {
        let file_path = series_path.join(format!("book{}.pdf", i));
        let pdf_path = create_test_pdf(temp_dir, 1, 0);
        fs::copy(&pdf_path, &file_path).unwrap();
    }

    LibraryRepository::create(
        db,
        "Test Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap()
}

// ============================================================================
// Allowed Formats Integration Tests
// ============================================================================

#[tokio::test]
async fn test_scan_respects_allowed_formats_cbr_only() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with mixed formats
    let mut library = setup_library_with_mixed_formats(db, &temp_dir).await;

    // Set allowed_formats to only CBR (but we only have CBZ, EPUB, PDF)
    library.allowed_formats = Some(r#"["CBR"]"#.to_string());
    codex::db::repositories::LibraryRepository::update(db, &library)
        .await
        .unwrap();

    // Run scan - should find 0 files (no CBR files exist)
    let result = scan_library(db, library.id, ScanMode::Normal, None)
        .await
        .unwrap();

    assert_eq!(result.books_created, 0);
    assert_eq!(result.files_processed, 0);

    // Verify no books were created
    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    assert_eq!(series_list.len(), 0);

    db_wrapper.close().await;
}

#[tokio::test]
async fn test_scan_respects_allowed_formats_cbz_only() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with mixed formats
    let mut library = setup_library_with_mixed_formats(db, &temp_dir).await;

    // Set allowed_formats to only CBZ
    library.allowed_formats = Some(r#"["CBZ"]"#.to_string());
    codex::db::repositories::LibraryRepository::update(db, &library)
        .await
        .unwrap();

    // Run scan - should only find CBZ files (2 files)
    let result = scan_library(db, library.id, ScanMode::Normal, None)
        .await
        .unwrap();

    assert_eq!(result.books_created, 2);
    assert_eq!(result.files_processed, 2);

    // Verify only CBZ books were created
    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    assert_eq!(series_list.len(), 1);

    let books = BookRepository::list_by_series(db, series_list[0].id, false)
        .await
        .unwrap();
    assert_eq!(books.len(), 2);
    assert!(books.iter().all(|b| b.format == "cbz"));

    db_wrapper.close().await;
}

#[tokio::test]
async fn test_scan_respects_allowed_formats_multiple_formats() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with mixed formats
    let mut library = setup_library_with_mixed_formats(db, &temp_dir).await;

    // Set allowed_formats to CBZ and EPUB
    library.allowed_formats = Some(r#"["CBZ","EPUB"]"#.to_string());
    codex::db::repositories::LibraryRepository::update(db, &library)
        .await
        .unwrap();

    // Run scan - should find CBZ and EPUB files (4 files total)
    let result = scan_library(db, library.id, ScanMode::Normal, None)
        .await
        .unwrap();

    assert_eq!(result.books_created, 4);
    assert_eq!(result.files_processed, 4);

    // Verify only CBZ and EPUB books were created
    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    assert_eq!(series_list.len(), 1);

    let books = BookRepository::list_by_series(db, series_list[0].id, false)
        .await
        .unwrap();
    assert_eq!(books.len(), 4);
    let formats: Vec<&str> = books.iter().map(|b| b.format.as_str()).collect();
    assert!(formats.contains(&"cbz"));
    assert!(formats.contains(&"epub"));
    assert!(!formats.contains(&"pdf"));

    db_wrapper.close().await;
}

#[tokio::test]
async fn test_scan_respects_allowed_formats_none_restriction() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with mixed formats
    let library = setup_library_with_mixed_formats(db, &temp_dir).await;

    // Don't set allowed_formats (None) - should allow all formats
    // Run scan - should find all files (6 files: 2 CBZ, 2 EPUB, 2 PDF)
    let result = scan_library(db, library.id, ScanMode::Normal, None)
        .await
        .unwrap();

    assert_eq!(result.books_created, 6);
    assert_eq!(result.files_processed, 6);

    // Verify all books were created
    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    assert_eq!(series_list.len(), 1);

    let books = BookRepository::list_by_series(db, series_list[0].id, false)
        .await
        .unwrap();
    assert_eq!(books.len(), 6);
    let formats: Vec<&str> = books.iter().map(|b| b.format.as_str()).collect();
    assert!(formats.contains(&"cbz"));
    assert!(formats.contains(&"epub"));
    assert!(formats.contains(&"pdf"));

    db_wrapper.close().await;
}

#[tokio::test]
async fn test_scan_respects_allowed_formats_deep_scan() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with mixed formats
    let mut library = setup_library_with_mixed_formats(db, &temp_dir).await;

    // Set allowed_formats to only EPUB
    library.allowed_formats = Some(r#"["EPUB"]"#.to_string());
    codex::db::repositories::LibraryRepository::update(db, &library)
        .await
        .unwrap();

    // Run deep scan - should only find EPUB files (2 files)
    let result = scan_library(db, library.id, ScanMode::Deep, None)
        .await
        .unwrap();

    assert_eq!(result.files_processed, 2);

    // Verify only EPUB books were created
    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    assert_eq!(series_list.len(), 1);

    let books = BookRepository::list_by_series(db, series_list[0].id, false)
        .await
        .unwrap();
    assert_eq!(books.len(), 2);
    assert!(books.iter().all(|b| b.format == "epub"));

    db_wrapper.close().await;
}
