#[path = "../common/mod.rs"]
mod common;

use codex::db::repositories::{BookRepository, LibraryRepository, SeriesRepository};
use codex::models::ScanningStrategy;
use codex::scanner::{scan_library, ScanMode};
use common::*;
use std::fs;
use tempfile::TempDir;

/// Helper to create a library with test files
async fn setup_library_with_files(
    db: &sea_orm::DatabaseConnection,
    temp_dir: &TempDir,
    file_count: usize,
) -> codex::db::entities::libraries::Model {
    let library_path = temp_dir.path().join("test_library");
    fs::create_dir_all(&library_path).unwrap();

    // Create series folder
    let series_path = library_path.join("Test Series");
    fs::create_dir_all(&series_path).unwrap();

    // Create test CBZ files
    for i in 1..=file_count {
        let file_path = series_path.join(format!("book{}.cbz", i));
        let cbz_path = create_test_cbz(temp_dir, i, false);
        fs::copy(&cbz_path, &file_path).unwrap();
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
// Soft Delete Integration Tests
// ============================================================================

#[tokio::test]
async fn test_scan_marks_missing_books_deleted() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with 3 files
    let library = setup_library_with_files(db, &temp_dir, 3).await;

    // Run initial scan to populate database
    let result = scan_library(db, library.id, ScanMode::Normal, None)
        .await
        .unwrap();

    assert_eq!(result.books_created, 3);
    assert_eq!(result.books_deleted, 0);

    // Get the created books
    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    assert_eq!(series_list.len(), 1);

    let books = BookRepository::list_by_series(db, series_list[0].id, false)
        .await
        .unwrap();
    assert_eq!(books.len(), 3);

    // Delete one file from filesystem
    let series_path = temp_dir.path().join("test_library/Test Series");
    let file_to_delete = series_path.join("book2.cbz");
    fs::remove_file(file_to_delete).unwrap();

    // Run scan again - should mark book as deleted
    let result = scan_library(db, library.id, ScanMode::Normal, None)
        .await
        .unwrap();

    assert_eq!(result.books_created, 0);
    assert_eq!(result.books_updated, 0);
    assert_eq!(result.books_deleted, 1);

    // Verify only 2 books are returned when not including deleted
    let active_books = BookRepository::list_by_series(db, series_list[0].id, false)
        .await
        .unwrap();
    assert_eq!(active_books.len(), 2);

    // Verify 3 books exist including deleted
    let all_books = BookRepository::list_by_series(db, series_list[0].id, true)
        .await
        .unwrap();
    assert_eq!(all_books.len(), 3);

    // Verify the deleted book is marked correctly
    let deleted_book = all_books.iter().find(|b| b.deleted).unwrap();
    assert!(deleted_book.file_path.ends_with("book2.cbz"));

    db_wrapper.close().await;
}

#[tokio::test]
async fn test_scan_restores_reappeared_books() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with 2 files
    let library = setup_library_with_files(db, &temp_dir, 2).await;

    // Run initial scan
    let result = scan_library(db, library.id, ScanMode::Normal, None)
        .await
        .unwrap();
    assert_eq!(result.books_created, 2);

    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    let series_path = temp_dir.path().join("test_library/Test Series");

    // Delete a file
    let file_path = series_path.join("book1.cbz");
    let backup_path = temp_dir.path().join("backup_book1.cbz");
    fs::rename(&file_path, &backup_path).unwrap();

    // Scan - should mark as deleted
    let result = scan_library(db, library.id, ScanMode::Normal, None)
        .await
        .unwrap();
    assert_eq!(result.books_deleted, 1);

    // Verify only 1 active book
    let active_books = BookRepository::list_by_series(db, series_list[0].id, false)
        .await
        .unwrap();
    assert_eq!(active_books.len(), 1);

    // Restore the file
    fs::rename(&backup_path, &file_path).unwrap();

    // Scan again - should restore the book
    let result = scan_library(db, library.id, ScanMode::Normal, None)
        .await
        .unwrap();
    assert_eq!(result.books_restored, 1);
    assert_eq!(result.books_deleted, 0);

    // Verify 2 active books again
    let active_books = BookRepository::list_by_series(db, series_list[0].id, false)
        .await
        .unwrap();
    assert_eq!(active_books.len(), 2);

    // Verify no books are marked deleted
    let all_books = BookRepository::list_by_series(db, series_list[0].id, true)
        .await
        .unwrap();
    assert!(all_books.iter().all(|b| !b.deleted));

    db_wrapper.close().await;
}

#[tokio::test]
async fn test_scan_leaves_deleted_books_unchanged() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with 2 files
    let library = setup_library_with_files(db, &temp_dir, 2).await;

    // Run initial scan
    scan_library(db, library.id, ScanMode::Normal, None)
        .await
        .unwrap();

    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    let series_path = temp_dir.path().join("test_library/Test Series");

    // Delete a file
    fs::remove_file(series_path.join("book1.cbz")).unwrap();

    // First scan - marks as deleted
    let result = scan_library(db, library.id, ScanMode::Normal, None)
        .await
        .unwrap();
    assert_eq!(result.books_deleted, 1);

    // Second scan - file still missing, should not change anything
    let result = scan_library(db, library.id, ScanMode::Normal, None)
        .await
        .unwrap();
    assert_eq!(result.books_deleted, 0); // Already deleted
    assert_eq!(result.books_restored, 0);

    // Verify still only 1 active book
    let active_books = BookRepository::list_by_series(db, series_list[0].id, false)
        .await
        .unwrap();
    assert_eq!(active_books.len(), 1);

    db_wrapper.close().await;
}

#[tokio::test]
async fn test_scan_multiple_files_deleted_and_restored() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with 4 files
    let library = setup_library_with_files(db, &temp_dir, 4).await;

    // Run initial scan
    scan_library(db, library.id, ScanMode::Normal, None)
        .await
        .unwrap();

    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    let series_path = temp_dir.path().join("test_library/Test Series");

    // Delete 2 files
    fs::remove_file(series_path.join("book1.cbz")).unwrap();
    fs::remove_file(series_path.join("book3.cbz")).unwrap();

    // Scan - should mark 2 as deleted
    let result = scan_library(db, library.id, ScanMode::Normal, None)
        .await
        .unwrap();
    assert_eq!(result.books_deleted, 2);

    // Verify 2 active books
    let active_books = BookRepository::list_by_series(db, series_list[0].id, false)
        .await
        .unwrap();
    assert_eq!(active_books.len(), 2);

    // Restore one file
    let cbz_path = create_test_cbz(&temp_dir, 1, false);
    fs::copy(&cbz_path, series_path.join("book1.cbz")).unwrap();

    // Scan - should restore 1 book
    let result = scan_library(db, library.id, ScanMode::Normal, None)
        .await
        .unwrap();
    assert_eq!(result.books_restored, 1);
    assert_eq!(result.books_deleted, 0);

    // Verify 3 active books
    let active_books = BookRepository::list_by_series(db, series_list[0].id, false)
        .await
        .unwrap();
    assert_eq!(active_books.len(), 3);

    // Verify total of 4 books (3 active + 1 deleted)
    let all_books = BookRepository::list_by_series(db, series_list[0].id, true)
        .await
        .unwrap();
    assert_eq!(all_books.len(), 4);

    let deleted_count = all_books.iter().filter(|b| b.deleted).count();
    assert_eq!(deleted_count, 1);

    db_wrapper.close().await;
}

#[tokio::test]
async fn test_deep_scan_reprocesses_all_files() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with 2 files
    let library = setup_library_with_files(db, &temp_dir, 2).await;

    // Run initial scan
    scan_library(db, library.id, ScanMode::Normal, None)
        .await
        .unwrap();

    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();

    // Run deep scan - should reprocess all files but not report updates (since nothing changed)
    let result = scan_library(db, library.id, ScanMode::Deep, None)
        .await
        .unwrap();

    // Deep scan reprocesses everything but doesn't load existing books cache
    // So it may create or update based on what it finds
    assert_eq!(result.files_processed, 2);

    // Verify 2 active books still exist
    let active_books = BookRepository::list_by_series(db, series_list[0].id, false)
        .await
        .unwrap();
    assert_eq!(active_books.len(), 2);

    db_wrapper.close().await;
}
