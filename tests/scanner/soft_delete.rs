#[path = "../common/mod.rs"]
mod common;

use codex::db::repositories::{
    BookMetadataRepository, BookRepository, LibraryRepository, SeriesRepository,
};
use codex::models::ScanningStrategy;
use codex::scanner::{ScanMode, scan_library};
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
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
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
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
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
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
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
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
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
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
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
    scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();

    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    let series_path = temp_dir.path().join("test_library/Test Series");

    // Delete a file
    fs::remove_file(series_path.join("book1.cbz")).unwrap();

    // First scan - marks as deleted
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();
    assert_eq!(result.books_deleted, 1);

    // Second scan - file still missing, should not change anything
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
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
    scan_library(db, library.id, ScanMode::Normal, None, None)
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
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
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
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
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
    scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();

    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();

    // Run deep scan - should reprocess all files but not report updates (since nothing changed)
    let result = scan_library(db, library.id, ScanMode::Deep, None, None)
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

/// Test that purge_deleted_on_scan configuration works correctly via task handler
#[tokio::test]
async fn test_purge_deleted_on_scan_config() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with 3 files
    let mut library = setup_library_with_files(db, &temp_dir, 3).await;

    // Set scanning_config with purge_deleted_on_scan enabled
    let scanning_config = serde_json::json!({
        "enabled": true,
        "scanMode": "normal",
        "scanOnStart": false,
        "purgeDeletedOnScan": true
    });
    library.scanning_config = Some(scanning_config.to_string());
    LibraryRepository::update(db, &library).await.unwrap();

    // Run initial scan
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();
    assert_eq!(result.books_created, 3);

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
    fs::remove_file(&file_to_delete).unwrap();

    // Run scan again - should mark book as deleted
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();
    assert_eq!(result.books_deleted, 1);

    // Verify book is marked as deleted
    let all_books = BookRepository::list_by_series(db, series_list[0].id, true)
        .await
        .unwrap();
    let deleted_books: Vec<_> = all_books.iter().filter(|b| b.deleted).collect();
    assert_eq!(deleted_books.len(), 1);

    // Now test that the task handler purges deleted books when purge_deleted_on_scan is enabled
    // We need to trigger a scan via the task queue to test the handler
    use codex::db::repositories::TaskRepository;
    use codex::tasks::TaskWorker;
    use codex::tasks::types::TaskType;

    // Trigger scan via task queue
    let task_type = TaskType::ScanLibrary {
        library_id: library.id,
        mode: "normal".to_string(),
    };
    TaskRepository::enqueue(db, task_type, None).await.unwrap();

    // Process the scan task (this should purge deleted books)
    let worker = TaskWorker::new(db.clone());
    worker.process_once().await.ok();

    // Wait a bit for task to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Verify the deleted book was purged (permanently deleted)
    let all_books_after = BookRepository::list_by_series(db, series_list[0].id, true)
        .await
        .unwrap();
    let deleted_books_after: Vec<_> = all_books_after.iter().filter(|b| b.deleted).collect();
    assert_eq!(
        deleted_books_after.len(),
        0,
        "Deleted book should have been purged by scan handler"
    );
    assert_eq!(
        all_books_after.len(),
        2,
        "Should only have 2 active books remaining"
    );

    db_wrapper.close().await;
}

// ============================================================================
// Renumbering on Delete/Restore Tests
// ============================================================================

use sea_orm::prelude::Decimal;

/// Helper to create metadata with a number for all books in a series
async fn create_metadata_for_books(db: &sea_orm::DatabaseConnection, series_id: uuid::Uuid) {
    let books = BookRepository::list_by_series(db, series_id, true)
        .await
        .unwrap();

    // Sort by filename using natural sort to assign numbers consistently
    let mut sorted_books: Vec<_> = books.iter().collect();
    sorted_books.sort_by(|a, b| a.file_name.cmp(&b.file_name));

    for (i, book) in sorted_books.iter().enumerate() {
        let number = if book.deleted {
            None
        } else {
            Some(Decimal::from((i + 1) as i64))
        };
        BookMetadataRepository::create_with_title_and_number(
            db,
            book.id,
            Some(book.file_name.clone()),
            number,
        )
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn test_deleted_books_have_number_cleared() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with 3 files
    let library = setup_library_with_files(db, &temp_dir, 3).await;

    // Run initial scan to create books
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();
    assert_eq!(result.books_created, 3);

    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    let series_id = series_list[0].id;

    // Manually create metadata with numbers (analysis tasks run asynchronously)
    create_metadata_for_books(db, series_id).await;

    // Verify all books have numbers assigned
    let books = BookRepository::list_by_series(db, series_id, false)
        .await
        .unwrap();
    assert_eq!(books.len(), 3);
    for book in &books {
        let metadata = BookMetadataRepository::get_by_book_id(db, book.id)
            .await
            .unwrap()
            .unwrap();
        assert!(
            metadata.number.is_some(),
            "Book '{}' should have a number before deletion",
            book.file_name
        );
    }

    // Delete one file from filesystem
    let series_path = temp_dir.path().join("test_library/Test Series");
    fs::remove_file(series_path.join("book2.cbz")).unwrap();

    // Run scan - should mark as deleted AND clear number on deleted book
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();
    assert_eq!(result.books_deleted, 1);

    // Verify deleted book has number cleared
    let all_books = BookRepository::list_by_series(db, series_id, true)
        .await
        .unwrap();
    let deleted_book = all_books.iter().find(|b| b.deleted).unwrap();
    let deleted_metadata = BookMetadataRepository::get_by_book_id(db, deleted_book.id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        deleted_metadata.number.is_none(),
        "Deleted book '{}' should have number cleared, got {:?}",
        deleted_book.file_name,
        deleted_metadata.number
    );

    // Verify active books still have valid numbers
    let active_books = BookRepository::list_by_series(db, series_id, false)
        .await
        .unwrap();
    assert_eq!(active_books.len(), 2);
    for book in &active_books {
        let metadata = BookMetadataRepository::get_by_book_id(db, book.id)
            .await
            .unwrap()
            .unwrap();
        assert!(
            metadata.number.is_some(),
            "Active book '{}' should still have a number",
            book.file_name
        );
    }

    db_wrapper.close().await;
}

#[tokio::test]
async fn test_restored_books_get_renumbered() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with 3 files
    let library = setup_library_with_files(db, &temp_dir, 3).await;

    // Run initial scan and create metadata
    scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();

    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    let series_id = series_list[0].id;
    let series_path = temp_dir.path().join("test_library/Test Series");

    create_metadata_for_books(db, series_id).await;

    // Delete a file and scan to mark it deleted
    let backup_path = temp_dir.path().join("backup_book2.cbz");
    fs::rename(series_path.join("book2.cbz"), &backup_path).unwrap();
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();
    assert_eq!(result.books_deleted, 1);

    // Verify deleted book has number cleared
    let all_books = BookRepository::list_by_series(db, series_id, true)
        .await
        .unwrap();
    let deleted_book = all_books.iter().find(|b| b.deleted).unwrap();
    let deleted_metadata = BookMetadataRepository::get_by_book_id(db, deleted_book.id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        deleted_metadata.number.is_none(),
        "Deleted book should have number cleared"
    );

    // Restore the file
    fs::rename(&backup_path, series_path.join("book2.cbz")).unwrap();

    // Scan again - should restore and renumber
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();
    assert_eq!(result.books_restored, 1);

    // Verify all active books have numbers (restored book gets renumbered)
    let active_books = BookRepository::list_by_series(db, series_id, false)
        .await
        .unwrap();
    assert_eq!(active_books.len(), 3);
    for book in &active_books {
        let metadata = BookMetadataRepository::get_by_book_id(db, book.id)
            .await
            .unwrap()
            .unwrap();
        assert!(
            metadata.number.is_some(),
            "Book '{}' should have a number after restore",
            book.file_name
        );
    }

    db_wrapper.close().await;
}

#[tokio::test]
async fn test_remaining_books_renumbered_contiguously_after_deletion() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    // Create library with 4 files
    let library = setup_library_with_files(db, &temp_dir, 4).await;

    // Run initial scan and create metadata
    scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();

    let series_list = SeriesRepository::list_by_library(db, library.id)
        .await
        .unwrap();
    let series_id = series_list[0].id;
    let series_path = temp_dir.path().join("test_library/Test Series");

    create_metadata_for_books(db, series_id).await;

    // Delete books 1 and 3 (leaving gaps)
    fs::remove_file(series_path.join("book1.cbz")).unwrap();
    fs::remove_file(series_path.join("book3.cbz")).unwrap();

    // Scan - should delete 2 books and renumber remaining
    let result = scan_library(db, library.id, ScanMode::Normal, None, None)
        .await
        .unwrap();
    assert_eq!(result.books_deleted, 2);

    // Verify active books are renumbered contiguously (1, 2) not (2, 4)
    let active_books = BookRepository::list_by_series(db, series_id, false)
        .await
        .unwrap();
    assert_eq!(active_books.len(), 2);

    let mut numbers: Vec<i64> = Vec::new();
    for book in &active_books {
        let metadata = BookMetadataRepository::get_by_book_id(db, book.id)
            .await
            .unwrap()
            .unwrap();
        if let Some(num) = metadata.number {
            numbers.push(num.to_string().parse::<i64>().unwrap());
        }
    }
    numbers.sort();
    assert_eq!(
        numbers,
        vec![1, 2],
        "Remaining books should be renumbered contiguously as 1, 2"
    );

    // Verify deleted books have cleared numbers
    let all_books = BookRepository::list_by_series(db, series_id, true)
        .await
        .unwrap();
    for book in all_books.iter().filter(|b| b.deleted) {
        let metadata = BookMetadataRepository::get_by_book_id(db, book.id)
            .await
            .unwrap()
            .unwrap();
        assert!(
            metadata.number.is_none(),
            "Deleted book '{}' should have number cleared",
            book.file_name
        );
    }

    db_wrapper.close().await;
}
