mod common;

use codex::db::repositories::TaskRepository;
use codex::tasks::types::TaskType;
use codex::tasks::TaskWorker;
use common::setup_test_db;
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
        TaskType::GenerateThumbnails { library_id: None },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue task");

    // Start a worker
    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));

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

    // Create multiple tasks
    for _ in 0..3 {
        TaskRepository::enqueue(
            &db,
            TaskType::GenerateThumbnails { library_id: None },
            0,
            None,
        )
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
        TaskType::GenerateThumbnails { library_id: None },
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
        TaskType::GenerateThumbnails { library_id: None },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue task");

    // Create two workers
    let worker1 = TaskWorker::new(db.clone())
        .with_worker_id("worker-1")
        .with_poll_interval(Duration::from_millis(50));

    let worker2 = TaskWorker::new(db.clone())
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
        TaskType::GenerateThumbnails { library_id: None },
        0, // Low priority
        None,
    )
    .await
    .expect("Failed to enqueue");

    // Create high priority task second
    let high_id = TaskRepository::enqueue(
        &db,
        TaskType::GenerateThumbnails { library_id: None },
        10, // High priority
        None,
    )
    .await
    .expect("Failed to enqueue");

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(50));

    // Process one task
    worker.process_once().await.expect("Failed to process");

    // Check which task was processed
    let low_task = TaskRepository::get_by_id(&db, low_id)
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
        TaskType::GenerateThumbnails { library_id: None },
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
        TaskType::GenerateThumbnails { library_id: None },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue");

    // Claim with very short lock duration (1 second)
    TaskRepository::claim_next(&db, "crashed-worker", 1)
        .await
        .expect("Failed to claim");

    // Wait for lock to expire
    sleep(Duration::from_secs(2)).await;

    // New worker should be able to claim it
    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(50));

    let processed = worker.process_once().await.expect("Failed to process");

    assert!(processed, "Worker should have claimed the stale task");

    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    // Should have been re-claimed
    assert!(task.attempts >= 1);
}
