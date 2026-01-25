#[path = "../common/mod.rs"]
mod common;

use codex::db::repositories::{BookRepository, LibraryRepository, SeriesRepository};
use codex::models::ScanningStrategy;
use codex::scanner::{scan_library, ScanMode};
use common::*;
use std::fs;
use tempfile::TempDir;

/// Helper to create a library with a specific series folder and files
async fn setup_library_with_series(
    db: &sea_orm::DatabaseConnection,
    temp_dir: &TempDir,
    series_name: &str,
    filenames: &[&str],
) -> (
    codex::db::entities::libraries::Model,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let library_path = temp_dir.path().join("test_library");
    fs::create_dir_all(&library_path).unwrap();

    // Create series folder
    let series_path = library_path.join(series_name);
    fs::create_dir_all(&series_path).unwrap();

    // Create test CBZ files with specified names
    for (i, filename) in filenames.iter().enumerate() {
        let file_path = series_path.join(filename);
        let cbz_path = create_test_cbz(temp_dir, i + 1, false);
        fs::copy(&cbz_path, &file_path).unwrap();
    }

    let library = LibraryRepository::create(
        db,
        "Test Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    (library, library_path, series_path)
}

// ============================================================================
// Series Matching Tests - Rename vs Copy Detection
// ============================================================================

/// Test that renaming a series folder preserves the series identity via fingerprint matching
#[tokio::test]
async fn test_series_rename_preserves_identity() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with initial series
    let (library, library_path, series_path) = setup_library_with_series(
        db,
        &temp_dir,
        "Original Series",
        &["book1.cbz", "book2.cbz"],
    )
    .await;

    // Run initial scan
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();
    assert_eq!(result.books_created, 2);
    assert_eq!(result.series_created, 1);

    // Get the original series ID
    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    assert_eq!(series_list.len(), 1);
    let original_series_id = series_list[0].id;
    assert_eq!(series_list[0].name, "Original Series");

    // RENAME the series folder (not copy)
    let new_series_path = library_path.join("Renamed Series");
    fs::rename(&series_path, &new_series_path).unwrap();

    // Verify old path doesn't exist, new path does
    assert!(!series_path.exists());
    assert!(new_series_path.exists());

    // Run scan again - should match by fingerprint and update the series
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();

    // No new series should be created (fingerprint match)
    assert_eq!(result.series_created, 0);

    // Verify same series ID is used, but name and path are updated
    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    assert_eq!(series_list.len(), 1);
    assert_eq!(series_list[0].id, original_series_id);
    assert_eq!(series_list[0].name, "Renamed Series");
    assert_eq!(series_list[0].path, "Renamed Series");

    // Verify books are still associated with the same series
    let books = BookRepository::list_by_series(db, original_series_id, false)
        .await
        .unwrap();
    assert_eq!(books.len(), 2);
}

/// Test that copying a series folder creates a new series (not matched by fingerprint)
#[tokio::test]
async fn test_series_copy_creates_new_series() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with initial series
    let (library, library_path, series_path) = setup_library_with_series(
        db,
        &temp_dir,
        "Original Series",
        &["book1.cbz", "book2.cbz"],
    )
    .await;

    // Run initial scan
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();
    assert_eq!(result.books_created, 2);
    assert_eq!(result.series_created, 1);

    // Get the original series ID
    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    assert_eq!(series_list.len(), 1);
    let original_series_id = series_list[0].id;

    // COPY the series folder (keep original)
    let copied_series_path = library_path.join("Copied Series");
    copy_dir_recursive(&series_path, &copied_series_path).unwrap();

    // Verify both paths exist
    assert!(series_path.exists());
    assert!(copied_series_path.exists());

    // Run scan again - should create a NEW series for the copy
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();

    // A new series should be created
    assert_eq!(result.series_created, 1);
    // New books created for the copied series
    assert_eq!(result.books_created, 2);

    // Verify we now have 2 series
    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    assert_eq!(series_list.len(), 2);

    // Find original and copied series
    let original_series = series_list
        .iter()
        .find(|s| s.name == "Original Series")
        .unwrap();
    let copied_series = series_list
        .iter()
        .find(|s| s.name == "Copied Series")
        .unwrap();

    // Verify they have different IDs
    assert_ne!(original_series.id, copied_series.id);
    assert_eq!(original_series.id, original_series_id);

    // Verify each series has its own books
    let original_books = BookRepository::list_by_series(db, original_series.id, false)
        .await
        .unwrap();
    let copied_books = BookRepository::list_by_series(db, copied_series.id, false)
        .await
        .unwrap();

    assert_eq!(original_books.len(), 2);
    assert_eq!(copied_books.len(), 2);
}

/// Test nested folder copy scenario (the original bug case)
/// When files are copied to a nested location with same filenames, should create new series
#[tokio::test]
async fn test_nested_folder_copy_creates_new_series() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with initial series
    let (library, library_path, _series_path) =
        setup_library_with_series(db, &temp_dir, "My Series", &["book.cbz"]).await;

    // Run initial scan
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();
    assert_eq!(result.books_created, 1);
    assert_eq!(result.series_created, 1);

    // Get the original series ID
    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    let original_series_id = series_list[0].id;
    assert_eq!(series_list[0].path, "My Series");

    // Create a nested copy: _to_filter/My Series Filtered/book.cbz
    // This mimics the bug scenario where same filename causes same fingerprint
    let nested_parent = library_path.join("_to_filter");
    fs::create_dir_all(&nested_parent).unwrap();
    let nested_series_path = nested_parent.join("My Series Filtered");
    fs::create_dir_all(&nested_series_path).unwrap();

    // Copy the same file with same name (causes fingerprint collision)
    let original_file = library_path.join("My Series/book.cbz");
    let nested_file = nested_series_path.join("book.cbz");
    fs::copy(&original_file, &nested_file).unwrap();

    // Verify both paths exist
    assert!(original_file.exists());
    assert!(nested_file.exists());

    // Run scan again - should create a NEW series for the nested copy
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();

    // A new series should be created (not matched by fingerprint)
    assert_eq!(result.series_created, 1);
    assert_eq!(result.books_created, 1);

    // Verify we now have 2 series
    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    assert_eq!(series_list.len(), 2);

    // Find both series
    let original_series = series_list.iter().find(|s| s.name == "My Series").unwrap();
    let nested_series = series_list
        .iter()
        .find(|s| s.name == "My Series Filtered")
        .unwrap();

    // Verify they have different IDs
    assert_ne!(original_series.id, nested_series.id);
    assert_eq!(original_series.id, original_series_id);

    // Verify paths are correct
    assert_eq!(original_series.path, "My Series");
    assert_eq!(nested_series.path, "_to_filter/My Series Filtered");
}

/// Test that deep scan also respects rename vs copy distinction
#[tokio::test]
async fn test_deep_scan_respects_rename_vs_copy() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with initial series
    let (library, library_path, series_path) = setup_library_with_series(
        db,
        &temp_dir,
        "Original Series",
        &["book1.cbz", "book2.cbz"],
    )
    .await;

    // Run initial scan
    scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();

    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    let original_series_id = series_list[0].id;

    // COPY the series folder
    let copied_series_path = library_path.join("Copied Series");
    copy_dir_recursive(&series_path, &copied_series_path).unwrap();

    // Run DEEP scan - should also create new series for copy
    let result = scan_library(db, library.id, ScanMode::Deep, None, None)
        .await
        .unwrap();

    // A new series should be created
    assert_eq!(result.series_created, 1);

    // Verify we have 2 series
    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    assert_eq!(series_list.len(), 2);

    // Original series should be preserved
    let original_series = series_list
        .iter()
        .find(|s| s.name == "Original Series")
        .unwrap();
    assert_eq!(original_series.id, original_series_id);
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Recursively copy a directory
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}
