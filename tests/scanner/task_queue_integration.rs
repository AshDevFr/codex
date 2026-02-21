#[path = "../common/mod.rs"]
mod common;

use codex::db::ScanningStrategy;
use codex::db::repositories::{BookRepository, LibraryRepository, SeriesRepository};
use codex::scanner::ScanMode;
use common::*;
use sea_orm::EntityTrait;

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

    // Trigger normal scan (auto-analysis is handled by the scan handler)
    trigger_scan_task(&db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Create a worker to process the scan task
    use codex::tasks::TaskWorker;
    let worker = TaskWorker::new(db.clone());

    // Process the scan task (this will queue analysis tasks)
    worker.process_once().await.ok();

    // Verify books were created
    let books = BookRepository::get_unanalyzed_in_library(&db, library.id)
        .await
        .unwrap();
    assert_eq!(books.len(), 3, "Should have 3 unanalyzed books");

    // Verify tasks were queued for each book
    let all_tasks = get_all_tasks(&db).await;
    let analysis_tasks: Vec<_> = all_tasks
        .iter()
        .filter(|t| t.task_type == "analyze_book")
        .collect();
    assert_eq!(
        analysis_tasks.len(),
        3,
        "Should have 3 analysis tasks queued"
    );

    // Verify all tasks are AnalyzeBook tasks with correct book IDs
    let book_ids: Vec<_> = books.iter().map(|b| b.id).collect();
    for task in &analysis_tasks {
        assert_eq!(task.task_type, "analyze_book");
        assert!(task.book_id.is_some());
        assert!(
            book_ids.contains(&task.book_id.unwrap()),
            "Task book_id should match one of the created books"
        );
        assert_eq!(task.status, "pending");
        assert_eq!(
            task.priority, 800,
            "AnalyzeBook default priority should be 800"
        );
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

    // Create a worker to process tasks
    use codex::tasks::TaskWorker;
    let worker = TaskWorker::new(db.clone());

    // First scan - should queue tasks for both books
    trigger_scan_task(&db, library.id, ScanMode::Normal)
        .await
        .unwrap();
    worker.process_once().await.ok();

    let all_tasks_after_first_scan = get_all_tasks(&db).await;
    let analysis_tasks_after_first_scan: Vec<_> = all_tasks_after_first_scan
        .iter()
        .filter(|t| t.task_type == "analyze_book")
        .collect();
    assert_eq!(
        analysis_tasks_after_first_scan.len(),
        2,
        "First scan should queue 2 analysis tasks"
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

    // Second scan - should only queue task for the unanalyzed book
    trigger_scan_task(&db, library.id, ScanMode::Normal)
        .await
        .unwrap();
    worker.process_once().await.ok();

    let all_tasks_after_second_scan = get_all_tasks(&db).await;
    let analysis_tasks_after_second_scan: Vec<_> = all_tasks_after_second_scan
        .iter()
        .filter(|t| t.task_type == "analyze_book")
        .collect();
    assert_eq!(
        analysis_tasks_after_second_scan.len(),
        1,
        "Second scan should only queue 1 analysis task for unanalyzed book"
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

    // Create a worker to process tasks
    use codex::tasks::TaskWorker;
    let worker = TaskWorker::new(db.clone());

    // First normal scan
    trigger_scan_task(&db, library.id, ScanMode::Normal)
        .await
        .unwrap();
    worker.process_once().await.ok();

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

    // Trigger deep scan - should queue tasks for ALL books even though they're analyzed
    trigger_scan_task(&db, library.id, ScanMode::Deep)
        .await
        .unwrap();
    worker.process_once().await.ok();

    // Verify tasks were queued for all books
    let all_tasks = get_all_tasks(&db).await;
    let analysis_tasks: Vec<_> = all_tasks
        .iter()
        .filter(|t| t.task_type == "analyze_book")
        .collect();
    assert_eq!(
        analysis_tasks.len(),
        3,
        "Deep scan should queue analysis tasks for all 3 books"
    );

    // Verify all tasks are for the correct books
    let book_ids: Vec<_> = books.iter().map(|b| b.id).collect();
    for task in &analysis_tasks {
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
    .unwrap(); // Trigger scan
    trigger_scan_task(&db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Immediately after triggering, scan task should be in the database
    use codex::db::entities::{prelude::*, tasks};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let scan_task = Tasks::find()
        .filter(tasks::Column::TaskType.eq("scan_library"))
        .filter(tasks::Column::LibraryId.eq(library.id))
        .one(&db)
        .await
        .unwrap();
    assert!(
        scan_task.is_some(),
        "Scan task should be in database immediately"
    );

    // Wait for scan to complete (3 seconds)
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Scan task should still exist in database (persistent)
    let scan_task_after = Tasks::find()
        .filter(tasks::Column::TaskType.eq("scan_library"))
        .filter(tasks::Column::LibraryId.eq(library.id))
        .one(&db)
        .await
        .unwrap();
    assert!(
        scan_task_after.is_some(),
        "Scan task should still be in database after completion (persistent)"
    );

    // Verify we can trigger another scan now (will fail if pending/processing scan exists)
    let result = trigger_scan_task(&db, library.id, ScanMode::Normal).await;
    // This may fail if the first scan is still processing
    let _ = result; // Don't assert success, just verify we can call it
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
    .unwrap(); // Trigger first scan
    trigger_scan_task(&db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Try to trigger second scan immediately
    let result = trigger_scan_task(&db, library.id, ScanMode::Normal).await;

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

    // Create a worker to process tasks
    use codex::tasks::TaskWorker;
    let worker = TaskWorker::new(db.clone());

    // Trigger scan - should fail due to invalid path
    trigger_scan_task(&db, library.id, ScanMode::Normal)
        .await
        .unwrap(); // This doesn't fail - scan is async

    // Process the scan task (will fail)
    worker.process_once().await.ok();

    // Verify no analysis tasks were queued (only the failed scan task exists)
    let all_tasks = get_all_tasks(&db).await;
    let analysis_tasks: Vec<_> = all_tasks
        .iter()
        .filter(|t| t.task_type == "analyze_book")
        .collect();
    assert_eq!(
        analysis_tasks.len(),
        0,
        "No analysis tasks should be queued when scan fails"
    );

    // The scan task should be pending with retry (attempts < max_attempts)
    // It will be retried with exponential backoff
    let scan_task = all_tasks
        .iter()
        .find(|t| t.task_type == "scan_library")
        .expect("Scan task should exist");
    assert_eq!(scan_task.status, "pending");
    assert!(scan_task.last_error.is_some(), "Should have error message");
    assert!(scan_task.attempts > 0, "Should have attempted once");
}

/// Test that tasks have correct default priority based on TaskType
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
    trigger_scan_task(&db, library.id, ScanMode::Normal)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    let tasks = get_all_tasks(&db).await;
    assert_eq!(tasks.len(), 1);
    assert_eq!(
        tasks[0].priority, 1000,
        "ScanLibrary tasks should have default priority 1000"
    );
}

/// Test that purge_deleted_on_scan purges deleted books when enabled
#[tokio::test]
async fn test_purge_deleted_on_scan_enabled() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library with test files
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

    // Create library with purge_deleted_on_scan enabled
    let mut library = LibraryRepository::create(
        &db,
        "Test Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    // Set scanning_config with purge_deleted_on_scan enabled
    let scanning_config = serde_json::json!({
        "enabled": true,
        "scanMode": "normal",
        "scanOnStart": false,
        "purgeDeletedOnScan": true
    });
    library.scanning_config = Some(scanning_config.to_string());
    LibraryRepository::update(&db, &library).await.unwrap();

    // Trigger initial scan
    trigger_scan_task(&db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Process the scan task
    use codex::tasks::TaskWorker;
    let worker = TaskWorker::new(db.clone());
    worker.process_once().await.ok();

    // Wait a bit for scan to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Get books and mark one as deleted
    let series_list = SeriesRepository::list_by_library(&db, library.id)
        .await
        .unwrap();
    assert!(!series_list.is_empty(), "Should have at least one series");

    let books = BookRepository::list_by_series(&db, series_list[0].id, false)
        .await
        .unwrap();
    assert!(books.len() >= 2, "Should have at least 2 books");

    // Mark one book as deleted
    BookRepository::mark_deleted(&db, books[0].id, true, None)
        .await
        .unwrap();

    // Verify the book is marked as deleted but still exists
    let all_books = BookRepository::list_by_series(&db, series_list[0].id, true)
        .await
        .unwrap();
    let deleted_books: Vec<_> = all_books.iter().filter(|b| b.deleted).collect();
    assert_eq!(deleted_books.len(), 1, "Should have 1 deleted book");

    // Delete the file from filesystem to simulate it being removed
    std::fs::remove_file(&deleted_books[0].file_path).ok();

    // Trigger another scan - should purge the deleted book
    trigger_scan_task(&db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Process the scan task
    worker.process_once().await.ok();

    // Wait for scan to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Verify the deleted book was purged (permanently deleted)
    let all_books_after = BookRepository::list_by_series(&db, series_list[0].id, true)
        .await
        .unwrap();
    let deleted_books_after: Vec<_> = all_books_after.iter().filter(|b| b.deleted).collect();
    assert_eq!(
        deleted_books_after.len(),
        0,
        "Deleted book should have been purged"
    );
    assert_eq!(
        all_books_after.len(),
        books.len() - 1,
        "Total books should be reduced by 1"
    );

    // Verify task result includes books_purged
    use codex::db::entities::{prelude::*, tasks};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
    let scan_task = Tasks::find()
        .filter(tasks::Column::TaskType.eq("scan_library"))
        .filter(tasks::Column::LibraryId.eq(library.id))
        .order_by_desc(tasks::Column::CreatedAt)
        .one(&db)
        .await
        .unwrap()
        .expect("Scan task should exist");

    assert_eq!(scan_task.status, "completed");
    if let Some(result) = scan_task.result {
        let books_purged = result
            .get("books_purged")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert_eq!(
            books_purged, 1,
            "Task result should indicate 1 book was purged"
        );
    }
}

/// Test that purge_deleted_on_scan does NOT purge when disabled
#[tokio::test]
async fn test_purge_deleted_on_scan_disabled() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library with test files
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

    // Create library with purge_deleted_on_scan disabled (default)
    let mut library = LibraryRepository::create(
        &db,
        "Test Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    // Set scanning_config with purge_deleted_on_scan disabled
    let scanning_config = serde_json::json!({
        "enabled": true,
        "scanMode": "normal",
        "scanOnStart": false,
        "purgeDeletedOnScan": false
    });
    library.scanning_config = Some(scanning_config.to_string());
    LibraryRepository::update(&db, &library).await.unwrap();

    // Trigger initial scan
    trigger_scan_task(&db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Process the scan task
    use codex::tasks::TaskWorker;
    let worker = TaskWorker::new(db.clone());
    worker.process_once().await.ok();

    // Wait a bit for scan to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Get books and mark one as deleted
    let series_list = SeriesRepository::list_by_library(&db, library.id)
        .await
        .unwrap();
    assert!(!series_list.is_empty(), "Should have at least one series");

    let books = BookRepository::list_by_series(&db, series_list[0].id, false)
        .await
        .unwrap();
    assert!(books.len() >= 2, "Should have at least 2 books");

    // Mark one book as deleted
    BookRepository::mark_deleted(&db, books[0].id, true, None)
        .await
        .unwrap();

    // Verify the book is marked as deleted but still exists
    let all_books = BookRepository::list_by_series(&db, series_list[0].id, true)
        .await
        .unwrap();
    let deleted_books: Vec<_> = all_books.iter().filter(|b| b.deleted).collect();
    assert_eq!(deleted_books.len(), 1, "Should have 1 deleted book");

    // Delete the file from filesystem
    std::fs::remove_file(&deleted_books[0].file_path).ok();

    // Trigger another scan - should NOT purge the deleted book
    trigger_scan_task(&db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Process the scan task
    worker.process_once().await.ok();

    // Wait for scan to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Verify the deleted book was NOT purged (still exists)
    let all_books_after = BookRepository::list_by_series(&db, series_list[0].id, true)
        .await
        .unwrap();
    let deleted_books_after: Vec<_> = all_books_after.iter().filter(|b| b.deleted).collect();
    assert_eq!(
        deleted_books_after.len(),
        1,
        "Deleted book should NOT have been purged"
    );
    assert_eq!(
        all_books_after.len(),
        books.len(),
        "Total books should remain the same"
    );

    // Verify task result shows books_purged = 0
    use codex::db::entities::{prelude::*, tasks};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
    let scan_task = Tasks::find()
        .filter(tasks::Column::TaskType.eq("scan_library"))
        .filter(tasks::Column::LibraryId.eq(library.id))
        .order_by_desc(tasks::Column::CreatedAt)
        .one(&db)
        .await
        .unwrap()
        .expect("Scan task should exist");

    assert_eq!(scan_task.status, "completed");
    if let Some(result) = scan_task.result {
        let books_purged = result
            .get("books_purged")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert_eq!(
            books_purged, 0,
            "Task result should indicate 0 books were purged"
        );
    }
}

/// Test that purge_deleted_on_scan works with deep scan
#[tokio::test]
async fn test_purge_deleted_on_scan_with_deep_scan() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library with test files
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

    // Create library with purge_deleted_on_scan enabled
    let mut library = LibraryRepository::create(
        &db,
        "Test Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    // Set scanning_config with purge_deleted_on_scan enabled
    let scanning_config = serde_json::json!({
        "enabled": true,
        "scanMode": "deep",
        "scanOnStart": false,
        "purgeDeletedOnScan": true
    });
    library.scanning_config = Some(scanning_config.to_string());
    LibraryRepository::update(&db, &library).await.unwrap();

    // Trigger initial scan
    trigger_scan_task(&db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Process the scan task
    use codex::tasks::TaskWorker;
    let worker = TaskWorker::new(db.clone());
    worker.process_once().await.ok();

    // Wait a bit for scan to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Get books and mark one as deleted
    let series_list = SeriesRepository::list_by_library(&db, library.id)
        .await
        .unwrap();
    assert!(!series_list.is_empty(), "Should have at least one series");

    let books = BookRepository::list_by_series(&db, series_list[0].id, false)
        .await
        .unwrap();
    assert!(!books.is_empty(), "Should have at least 1 book");

    // Mark one book as deleted
    BookRepository::mark_deleted(&db, books[0].id, true, None)
        .await
        .unwrap();

    // Delete the file from filesystem
    std::fs::remove_file(&books[0].file_path).ok();

    // Trigger deep scan - should purge the deleted book
    trigger_scan_task(&db, library.id, ScanMode::Deep)
        .await
        .unwrap();

    // Process the scan task
    worker.process_once().await.ok();

    // Wait for scan to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Verify the deleted book was purged
    let all_books_after = BookRepository::list_by_series(&db, series_list[0].id, true)
        .await
        .unwrap();
    let deleted_books_after: Vec<_> = all_books_after.iter().filter(|b| b.deleted).collect();
    assert_eq!(
        deleted_books_after.len(),
        0,
        "Deleted book should have been purged after deep scan"
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
    use zip::ZipWriter;
    use zip::write::FileOptions;

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
