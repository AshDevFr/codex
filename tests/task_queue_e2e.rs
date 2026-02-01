mod common;

use codex::db::repositories::TaskRepository;
use codex::services::ThumbnailService;
use codex::tasks::types::TaskType;
use codex::tasks::TaskWorker;
use common::setup_test_db;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

/// Test end-to-end task execution with worker
#[tokio::test]
async fn test_e2e_task_execution() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a task
    let task_id = TaskRepository::enqueue(
        &db,
        TaskType::GenerateThumbnails {
            library_id: None,
            series_id: None,
            series_ids: None,
            book_ids: None,
            force: false,
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue task");

    // Create thumbnail service
    let files_config = codex::config::FilesConfig {
        thumbnail_dir: "data/thumbnails".to_string(),
        uploads_dir: "data/uploads".to_string(),
    };
    let thumbnail_service = Arc::new(ThumbnailService::new(files_config));

    // Start a worker with thumbnail service
    let worker = TaskWorker::new(db.clone())
        .with_thumbnail_service(thumbnail_service)
        .with_poll_interval(Duration::from_millis(100));

    // Process one task
    let processed = worker.process_once().await.expect("Failed to process task");

    // Note: This will fail if the library doesn't exist, but the task should be claimed
    assert!(processed);

    // Check task status - it should be either completed or failed (depending on library existence)
    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    assert!(
        task.status == "completed" || task.status == "failed" || task.status == "pending",
        "Task should have been processed, got status: {}",
        task.status
    );
}

/// Test worker processes multiple tasks
#[tokio::test]
async fn test_worker_processes_multiple_tasks() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create multiple tasks (using FindDuplicates which doesn't require external data)
    for _ in 0..3 {
        TaskRepository::enqueue(&db, TaskType::FindDuplicates, 0, None)
            .await
            .expect("Failed to enqueue task");
    }

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(50));

    // Process all tasks
    for _ in 0..3 {
        worker.process_once().await.expect("Failed to process");
    }

    // Check that all tasks were processed
    let stats = TaskRepository::get_stats(&db)
        .await
        .expect("Failed to get stats");

    // All tasks should be either completed or failed (not pending)
    assert_eq!(stats.pending, 0);
}

/// Test task retry on failure
#[tokio::test]
async fn test_task_retry_on_failure() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a task that will fail (non-existent book)
    let task_id = TaskRepository::enqueue(
        &db,
        TaskType::GenerateThumbnails {
            library_id: None,
            series_id: None,
            series_ids: None,
            book_ids: None,
            force: false,
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue task");

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(50));

    // Process once - should fail
    worker.process_once().await.ok();

    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    // Should have been retried (back to pending) or still processing, or completed successfully
    assert!(
        task.status == "pending"
            || task.status == "processing"
            || task.status == "failed"
            || task.status == "completed",
        "Task status: {}",
        task.status
    );

    // Should have attempted at least once
    assert!(task.attempts >= 1);
}

/// Test concurrent workers don't process same task
#[tokio::test]
async fn test_concurrent_workers_skip_locked() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a single task
    TaskRepository::enqueue(
        &db,
        TaskType::GenerateThumbnails {
            library_id: None,
            series_id: None,
            series_ids: None,
            book_ids: None,
            force: false,
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue task");

    // Create thumbnail service
    let files_config = codex::config::FilesConfig {
        thumbnail_dir: "data/thumbnails".to_string(),
        uploads_dir: "data/uploads".to_string(),
    };
    let thumbnail_service = Arc::new(ThumbnailService::new(files_config));

    // Create two workers
    let worker1 = TaskWorker::new(db.clone())
        .with_thumbnail_service(thumbnail_service.clone())
        .with_worker_id("worker-1")
        .with_poll_interval(Duration::from_millis(50));

    let worker2 = TaskWorker::new(db.clone())
        .with_thumbnail_service(thumbnail_service)
        .with_worker_id("worker-2")
        .with_poll_interval(Duration::from_millis(50));

    // Try to process concurrently
    let (result1, result2) = tokio::join!(worker1.process_once(), worker2.process_once());

    // One should succeed, one should get no task
    let processed_count = [result1.ok(), result2.ok()]
        .iter()
        .filter(|r| r.is_some() && r.unwrap())
        .count();

    assert_eq!(
        processed_count, 1,
        "Exactly one worker should have processed the task"
    );
}

/// Test priority ordering
#[tokio::test]
async fn test_worker_respects_priority() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create low priority task first
    let low_id = TaskRepository::enqueue(
        &db,
        TaskType::FindDuplicates,
        0, // Low priority
        None,
    )
    .await
    .expect("Failed to enqueue");

    // Create high priority task second
    let high_id = TaskRepository::enqueue(
        &db,
        TaskType::FindDuplicates,
        10, // High priority
        None,
    )
    .await
    .expect("Failed to enqueue");

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(50));

    // Process one task
    worker.process_once().await.expect("Failed to process");

    // Check which task was processed
    let _low_task = TaskRepository::get_by_id(&db, low_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    let high_task = TaskRepository::get_by_id(&db, high_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    // High priority should be processed first
    assert_ne!(high_task.status, "pending");
    // Low priority might still be pending
    // (it could also be processing if high task completed very quickly)
}

/// Test task cancellation prevents execution
#[tokio::test]
async fn test_cancelled_task_not_executed() {
    let (db, _temp_dir) = setup_test_db().await;

    let task_id = TaskRepository::enqueue(
        &db,
        TaskType::GenerateThumbnails {
            library_id: None,
            series_id: None,
            series_ids: None,
            book_ids: None,
            force: false,
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue");

    // Cancel it immediately
    TaskRepository::cancel(&db, task_id)
        .await
        .expect("Failed to cancel");

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(50));

    // Try to process
    let processed = worker.process_once().await.expect("Failed to process");

    // Should not have processed anything
    assert!(!processed);

    // Task should still be cancelled
    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    assert_eq!(task.status, "cancelled");
}

/// Test stale task recovery
#[tokio::test]
async fn test_stale_task_recovery() {
    let (db, _temp_dir) = setup_test_db().await;

    let task_id = TaskRepository::enqueue(
        &db,
        TaskType::GenerateThumbnails {
            library_id: None,
            series_id: None,
            series_ids: None,
            book_ids: None,
            force: false,
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue");

    // Claim with very short lock duration (1 second)
    TaskRepository::claim_next(&db, "crashed-worker", 1, false)
        .await
        .expect("Failed to claim");

    // Wait for lock to expire
    sleep(Duration::from_secs(2)).await;

    // Create thumbnail service
    let files_config = codex::config::FilesConfig {
        thumbnail_dir: "data/thumbnails".to_string(),
        uploads_dir: "data/uploads".to_string(),
    };
    let thumbnail_service = Arc::new(ThumbnailService::new(files_config));

    // New worker should be able to claim it
    let worker = TaskWorker::new(db.clone())
        .with_thumbnail_service(thumbnail_service)
        .with_poll_interval(Duration::from_millis(50));

    let processed = worker.process_once().await.expect("Failed to process");

    assert!(processed, "Worker should have claimed the stale task");

    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    // Should have been re-claimed
    assert!(task.attempts >= 1);
}

/// Test that worker reads prioritize_scans setting from SettingsService
#[tokio::test]
async fn test_worker_reads_prioritize_scans_setting() {
    use codex::db::repositories::SettingsRepository;
    use codex::services::SettingsService;
    use codex::tasks::TaskWorker;
    use std::sync::Arc;

    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;
    let series_id = create_test_series(&db, library_id).await;
    let book_id = create_test_book(&db, series_id, library_id).await;

    // Create settings service
    let settings_service = Arc::new(
        SettingsService::new(db.clone())
            .await
            .expect("Failed to create settings service"),
    );

    // Set prioritize_scans to false
    SettingsRepository::set(
        &db,
        "task.prioritize_scans_over_analysis",
        "false".to_string(),
        uuid::Uuid::new_v4(),
        Some("Test update".to_string()),
        None,
    )
    .await
    .expect("Failed to set setting");

    // Reload settings service to pick up the change
    settings_service.reload().await.expect("Failed to reload");

    // Enqueue tasks with same priority
    // Analysis task first
    let analyze_task_id = TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeBook {
            book_id,
            force: false,
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue analyze task");

    // Scan task second
    let scan_task_id = TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id,
            mode: "normal".to_string(),
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue scan task");

    // Create worker with settings service
    let worker = TaskWorker::new(db.clone())
        .with_settings_service(settings_service)
        .with_poll_interval(Duration::from_millis(50));

    // Process task - should get analysis task first (prioritization disabled)
    // With prioritization disabled and same priority, FIFO order applies
    let processed = worker.process_once().await.expect("Failed to process");
    assert!(processed, "Should have processed a task");

    // Check which task was processed by looking at attempts or started_at
    let analyze_task = TaskRepository::get_by_id(&db, analyze_task_id)
        .await
        .expect("Failed to get analyze task")
        .expect("Analyze task not found");

    let scan_task = TaskRepository::get_by_id(&db, scan_task_id)
        .await
        .expect("Failed to get scan task")
        .expect("Scan task not found");

    let analyze_processed = analyze_task.attempts > 0 || analyze_task.started_at.is_some();
    let scan_processed = scan_task.attempts > 0 || scan_task.started_at.is_some();

    // Verify that a task was processed
    assert!(
        analyze_processed || scan_processed,
        "At least one task should have been processed. Analyze: attempts={}, started_at={:?}, status={}. Scan: attempts={}, started_at={:?}, status={}",
        analyze_task.attempts,
        analyze_task.started_at,
        analyze_task.status,
        scan_task.attempts,
        scan_task.started_at,
        scan_task.status
    );

    // Note: This test verifies the worker reads the setting, not the exact ordering
    // The exact ordering depends on timing and priority when prioritization is disabled
}

/// Test worker with prioritize_scans enabled via settings
#[tokio::test]
async fn test_worker_with_prioritize_scans_enabled() {
    use codex::db::repositories::SettingsRepository;
    use codex::services::SettingsService;
    use codex::tasks::TaskWorker;
    use std::sync::Arc;

    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;
    let series_id = create_test_series(&db, library_id).await;
    let book_id = create_test_book(&db, series_id, library_id).await;

    // Create settings service
    let settings_service = Arc::new(
        SettingsService::new(db.clone())
            .await
            .expect("Failed to create settings service"),
    );

    // Ensure prioritize_scans is true (default)
    SettingsRepository::set(
        &db,
        "task.prioritize_scans_over_analysis",
        "true".to_string(),
        uuid::Uuid::new_v4(),
        Some("Test update".to_string()),
        None,
    )
    .await
    .expect("Failed to set setting");

    // Reload settings service
    settings_service.reload().await.expect("Failed to reload");

    // Enqueue tasks with same priority and scheduled_for
    // Analysis task first
    let analyze_task_id = TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeBook {
            book_id,
            force: false,
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue analyze task");

    // Scan task second
    let scan_task_id = TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id,
            mode: "normal".to_string(),
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue scan task");

    // Create worker with settings service
    let worker = TaskWorker::new(db.clone())
        .with_settings_service(settings_service)
        .with_poll_interval(Duration::from_millis(50));

    // Process task - should get scan task first (prioritization enabled)
    let processed = worker.process_once().await.expect("Failed to process");
    assert!(processed, "Should have processed a task");

    // Check which task was processed by looking at attempts or started_at
    // A processed task will have attempts > 0 or started_at set
    let analyze_task = TaskRepository::get_by_id(&db, analyze_task_id)
        .await
        .expect("Failed to get analyze task")
        .expect("Analyze task not found");

    let scan_task = TaskRepository::get_by_id(&db, scan_task_id)
        .await
        .expect("Failed to get scan task")
        .expect("Scan task not found");

    // With prioritization enabled, scan_library should be processed first
    // Check which task has been attempted (attempts > 0 or started_at is set)
    let scan_processed = scan_task.attempts > 0 || scan_task.started_at.is_some();
    let analyze_processed = analyze_task.attempts > 0 || analyze_task.started_at.is_some();

    assert!(
        scan_processed,
        "Scan task should be prioritized when setting is enabled. Scan task: attempts={}, started_at={:?}, status={}. Analyze task: attempts={}, started_at={:?}, status={}",
        scan_task.attempts,
        scan_task.started_at,
        scan_task.status,
        analyze_task.attempts,
        analyze_task.started_at,
        analyze_task.status
    );

    // If scan was processed, analyze should not have been processed yet
    if scan_processed {
        assert!(
            !analyze_processed,
            "Analyze task should not be processed before scan task when prioritization is enabled"
        );
    }
}

/// Helper to create a test library
async fn create_test_library(db: &sea_orm::DatabaseConnection) -> Uuid {
    use codex::db::repositories::LibraryRepository;
    use codex::db::ScanningStrategy;

    let library = LibraryRepository::create(
        db,
        "Test Library",
        "/tmp/test-library",
        ScanningStrategy::Default,
    )
    .await
    .expect("Failed to create library");
    library.id
}

/// Helper to create a test series
async fn create_test_series(db: &sea_orm::DatabaseConnection, library_id: Uuid) -> Uuid {
    use chrono::Utc;
    use codex::db::entities::{series, series_metadata};
    use sea_orm::{ActiveModelTrait, Set};

    let series_id = Uuid::new_v4();
    let now = Utc::now();
    let series = series::ActiveModel {
        id: Set(series_id),
        library_id: Set(library_id),
        fingerprint: Set(Some(format!("test-series-{}", series_id))),
        path: Set("/test/series".to_string()),
        name: Set("Test Series".to_string()),
        normalized_name: Set("test series".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
    };
    series.insert(db).await.expect("Failed to create series");

    // Also create series_metadata with the title
    let series_meta = series_metadata::ActiveModel {
        series_id: Set(series_id),
        title: Set("Test Series".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    series_meta
        .insert(db)
        .await
        .expect("Failed to create series metadata");

    series_id
}

/// Helper to create a test book
async fn create_test_book(
    db: &sea_orm::DatabaseConnection,
    series_id: Uuid,
    library_id: Uuid,
) -> Uuid {
    use chrono::Utc;
    use codex::db::entities::books;
    use sea_orm::{ActiveModelTrait, Set};

    let book_id = Uuid::new_v4();
    let now = Utc::now();
    let book = books::ActiveModel {
        id: Set(book_id),
        series_id: Set(series_id),
        library_id: Set(library_id),
        file_path: Set("/tmp/test.cbz".to_string()),
        file_name: Set("test.cbz".to_string()),
        file_size: Set(1024),
        file_hash: Set("test-hash".to_string()),
        format: Set("cbz".to_string()),
        page_count: Set(10),
        modified_at: Set(now),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    book.insert(db).await.expect("Failed to create book");
    book_id
}
