#[path = "../common/mod.rs"]
mod common;

use codex::db::repositories::{
    BookRepository, LibraryRepository, SeriesRepository, TaskRepository,
};
use codex::db::ScanningStrategy;
use codex::scanner::{ScanManager, ScanMode};
use codex::tasks::types::TaskType;
use common::*;
use sea_orm::EntityTrait;
use std::sync::Arc;

/// Test that normal scan queues tasks only for unanalyzed books
#[tokio::test]
async fn test_normal_scan_queues_unanalyzed_books() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a test library with some comic files
    let library_path = temp_dir.path().join("library");
    std::fs::create_dir_all(&library_path).unwrap();

    // Create a series directory with test files
    let series_dir = library_path.join("Test Series");
    std::fs::create_dir_all(&series_dir).unwrap();

    // Create 3 test CBZ files
    for i in 1..=3 {
        create_cbz_at_path(
            &series_dir.join(format!("Test Comic {:02}.cbz", i)),
            5,
            false,
        );
    }

    // Create the library in the database
    let library = LibraryRepository::create(
        &db,
        "Test Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    // Create scan manager
    let scan_manager = Arc::new(ScanManager::new_with_config(db.clone(), 2, 0)); // 0 = disable in-memory analysis

    // Trigger normal scan
    scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Wait for scan to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Verify books were created
    let books = BookRepository::get_unanalyzed_in_library(&db, library.id)
        .await
        .unwrap();
    assert_eq!(books.len(), 3, "Should have 3 unanalyzed books");

    // Verify tasks were queued for each book
    let tasks = get_all_tasks(&db).await;
    assert_eq!(tasks.len(), 3, "Should have 3 analysis tasks queued");

    // Verify all tasks are AnalyzeBook tasks with correct book IDs
    let book_ids: Vec<_> = books.iter().map(|b| b.id).collect();
    for task in &tasks {
        assert_eq!(task.task_type, "analyze_book");
        assert!(task.book_id.is_some());
        assert!(
            book_ids.contains(&task.book_id.unwrap()),
            "Task book_id should match one of the created books"
        );
        assert_eq!(task.status, "pending");
        assert_eq!(task.priority, 0);
    }
}

/// Test that normal scan does NOT queue tasks for already analyzed books
#[tokio::test]
async fn test_normal_scan_skips_analyzed_books() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a test library with some comic files
    let library_path = temp_dir.path().join("library");
    std::fs::create_dir_all(&library_path).unwrap();

    let series_dir = library_path.join("Test Series");
    std::fs::create_dir_all(&series_dir).unwrap();

    // Create 2 test CBZ files
    for i in 1..=2 {
        create_cbz_at_path(
            &series_dir.join(format!("Test Comic {:02}.cbz", i)),
            5,
            false,
        );
    }

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let scan_manager = Arc::new(ScanManager::new_with_config(db.clone(), 2, 0));

    // First scan - should queue tasks for both books
    scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    let tasks_after_first_scan = get_all_tasks(&db).await;
    assert_eq!(
        tasks_after_first_scan.len(),
        2,
        "First scan should queue 2 tasks"
    );

    // Mark one book as analyzed
    let books = BookRepository::get_unanalyzed_in_library(&db, library.id)
        .await
        .unwrap();
    BookRepository::mark_analyzed(&db, books[0].id, true)
        .await
        .unwrap();

    // Delete all tasks to start fresh
    delete_all_tasks(&db).await;

    // Wait for scan cleanup (10 seconds + buffer)
    tokio::time::sleep(tokio::time::Duration::from_secs(11)).await;

    // Second scan - should only queue task for the unanalyzed book
    scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    let tasks_after_second_scan = get_all_tasks(&db).await;
    assert_eq!(
        tasks_after_second_scan.len(),
        1,
        "Second scan should only queue 1 task for unanalyzed book"
    );
}

/// Test that deep scan queues tasks for ALL books (including already analyzed)
#[tokio::test]
async fn test_deep_scan_queues_all_books() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a test library with some comic files
    let library_path = temp_dir.path().join("library");
    std::fs::create_dir_all(&library_path).unwrap();

    let series_dir = library_path.join("Test Series");
    std::fs::create_dir_all(&series_dir).unwrap();

    // Create 3 test CBZ files
    for i in 1..=3 {
        create_cbz_at_path(
            &series_dir.join(format!("Test Comic {:02}.cbz", i)),
            5,
            false,
        );
    }

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let scan_manager = Arc::new(ScanManager::new_with_config(db.clone(), 2, 0));

    // First normal scan
    scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Mark all books as analyzed
    let books = BookRepository::get_unanalyzed_in_library(&db, library.id)
        .await
        .unwrap();
    for book in &books {
        BookRepository::mark_analyzed(&db, book.id, true)
            .await
            .unwrap();
    }

    // Delete all tasks
    delete_all_tasks(&db).await;

    // Verify no unanalyzed books remain
    let unanalyzed = BookRepository::get_unanalyzed_in_library(&db, library.id)
        .await
        .unwrap();
    assert_eq!(unanalyzed.len(), 0, "All books should be analyzed");

    // Wait for scan cleanup (10 seconds + buffer)
    tokio::time::sleep(tokio::time::Duration::from_secs(11)).await;

    // Trigger deep scan - should queue tasks for ALL books even though they're analyzed
    scan_manager
        .trigger_scan(library.id, ScanMode::Deep)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Verify tasks were queued for all books
    let tasks = get_all_tasks(&db).await;
    assert_eq!(
        tasks.len(),
        3,
        "Deep scan should queue tasks for all 3 books"
    );

    // Verify all tasks are for the correct books
    let book_ids: Vec<_> = books.iter().map(|b| b.id).collect();
    for task in &tasks {
        assert_eq!(task.task_type, "analyze_book");
        assert!(task.book_id.is_some());
        assert!(
            book_ids.contains(&task.book_id.unwrap()),
            "Task should be for one of the books"
        );
    }
}

/// Test that scan cleanup happens after 10 seconds
#[tokio::test]
async fn test_scan_cleanup_after_delay() {
    let (db, temp_dir) = setup_test_db().await;

    let library_path = temp_dir.path().join("library");
    std::fs::create_dir_all(&library_path).unwrap();

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let scan_manager = Arc::new(ScanManager::new_with_config(db.clone(), 2, 0));

    // Trigger scan
    scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Immediately after triggering, scan should be in active scans
    let status = scan_manager.get_status(library.id).await;
    assert!(
        status.is_some(),
        "Scan should be tracked in active scans immediately"
    );

    // Wait for scan to complete (3 seconds)
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Scan should still be in active scans (10 second cleanup delay)
    let status_after_completion = scan_manager.get_status(library.id).await;
    assert!(
        status_after_completion.is_some(),
        "Scan should still be tracked after completion (before cleanup)"
    );

    // Wait for cleanup (10 seconds + buffer)
    tokio::time::sleep(tokio::time::Duration::from_secs(11)).await;

    // Scan should now be cleaned up
    let status_after_cleanup = scan_manager.get_status(library.id).await;
    assert!(
        status_after_cleanup.is_none(),
        "Scan should be cleaned up after 10 seconds"
    );

    // Verify we can trigger another scan now
    let result = scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
        .await;
    assert!(
        result.is_ok(),
        "Should be able to trigger scan again after cleanup"
    );
}

/// Test that we cannot trigger a scan while another is in progress
#[tokio::test]
async fn test_cannot_trigger_concurrent_scan() {
    let (db, temp_dir) = setup_test_db().await;

    let library_path = temp_dir.path().join("library");
    std::fs::create_dir_all(&library_path).unwrap();

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let scan_manager = Arc::new(ScanManager::new_with_config(db.clone(), 2, 0));

    // Trigger first scan
    scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Try to trigger second scan immediately
    let result = scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
        .await;

    assert!(result.is_err(), "Should not allow concurrent scans");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("already being scanned"),
        "Error should indicate scan is in progress"
    );
}

/// Test that no tasks are queued when scan fails
#[tokio::test]
async fn test_no_tasks_queued_on_scan_failure() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library with invalid path (will cause scan to fail)
    let library = LibraryRepository::create(
        &db,
        "Test Library",
        "/nonexistent/path/that/does/not/exist",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let scan_manager = Arc::new(ScanManager::new_with_config(db.clone(), 2, 0));

    // Trigger scan - should fail due to invalid path
    scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
        .await
        .unwrap(); // This doesn't fail - scan is async

    // Wait for scan to fail
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Verify no tasks were queued
    let tasks = get_all_tasks(&db).await;
    assert_eq!(tasks.len(), 0, "No tasks should be queued when scan fails");
}

/// Test that tasks have correct priority (0 for auto-queued tasks)
#[tokio::test]
async fn test_queued_tasks_have_zero_priority() {
    let (db, temp_dir) = setup_test_db().await;

    let library_path = temp_dir.path().join("library");
    std::fs::create_dir_all(&library_path).unwrap();

    let series_dir = library_path.join("Test Series");
    std::fs::create_dir_all(&series_dir).unwrap();
    create_cbz_at_path(&series_dir.join("Test.cbz"), 5, false);

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let scan_manager = Arc::new(ScanManager::new_with_config(db.clone(), 2, 0));

    scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    let tasks = get_all_tasks(&db).await;
    assert_eq!(tasks.len(), 1);
    assert_eq!(
        tasks[0].priority, 0,
        "Auto-queued tasks should have priority 0"
    );
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get all tasks from the database
async fn get_all_tasks(db: &sea_orm::DatabaseConnection) -> Vec<codex::db::entities::tasks::Model> {
    use codex::db::entities::prelude::*;
    Tasks::find().all(db).await.unwrap()
}

/// Delete all tasks from the database
async fn delete_all_tasks(db: &sea_orm::DatabaseConnection) {
    use codex::db::entities::prelude::*;
    Tasks::delete_many().exec(db).await.unwrap();
}

/// Create a CBZ file at a specific path
fn create_cbz_at_path(path: &std::path::Path, num_pages: usize, with_comic_info: bool) {
    use std::fs::File;
    use std::io::Write;
    use zip::write::FileOptions;
    use zip::ZipWriter;

    let file = File::create(path).unwrap();
    let mut zip = ZipWriter::new(file);

    let options: FileOptions<'_, ()> =
        FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // Add pages
    for i in 1..=num_pages {
        // Create simple PNG data
        let page_data = create_simple_png();
        let filename = format!("page{:03}.png", i);
        zip.start_file(&filename, options).unwrap();
        zip.write_all(&page_data).unwrap();
    }

    // Add ComicInfo.xml if requested
    if with_comic_info {
        let comic_info_xml = r#"<?xml version="1.0"?>
<ComicInfo>
    <Title>Test Comic</Title>
    <Series>Test Series</Series>
    <Number>1</Number>
</ComicInfo>"#;

        zip.start_file("ComicInfo.xml", options).unwrap();
        zip.write_all(comic_info_xml.as_bytes()).unwrap();
    }

    zip.finish().unwrap();
}

/// Create a minimal valid PNG (1x1 pixel)
fn create_simple_png() -> Vec<u8> {
    // Minimal 1x1 red PNG
    vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, // IHDR chunk length
        0x49, 0x48, 0x44, 0x52, // "IHDR"
        0x00, 0x00, 0x00, 0x01, // width: 1
        0x00, 0x00, 0x00, 0x01, // height: 1
        0x08, 0x02, 0x00, 0x00, 0x00, // bit depth, color type, compression, filter, interlace
        0x90, 0x77, 0x53, 0xDE, // CRC
        0x00, 0x00, 0x00, 0x0C, // IDAT chunk length
        0x49, 0x44, 0x41, 0x54, // "IDAT"
        0x08, 0x99, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x03, 0x01, 0x01,
        0x00, // compressed data
        0x18, 0xDD, 0x8D, 0xB4, // CRC
        0x00, 0x00, 0x00, 0x00, // IEND chunk length
        0x49, 0x45, 0x4E, 0x44, // "IEND"
        0xAE, 0x42, 0x60, 0x82, // CRC
    ]
}
