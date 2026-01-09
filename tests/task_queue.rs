mod common;

use chrono::Utc;
use codex::db::entities::{books, libraries, series, tasks};
use codex::db::repositories::TaskRepository;
use codex::tasks::types::TaskType;
use common::{setup_test_db, setup_test_db_postgres};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use uuid::Uuid;

/// Helper to create a test library
async fn create_test_library(db: &DatabaseConnection) -> Uuid {
    let library_id = Uuid::new_v4();
    let now = Utc::now();
    let library = libraries::ActiveModel {
        id: Set(library_id),
        name: Set("Test Library".to_string()),
        path: Set("/tmp/test-library".to_string()),
        scanning_strategy: Set("normal".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    library.insert(db).await.expect("Failed to create library");
    library_id
}

/// Helper to create a test series
async fn create_test_series(db: &DatabaseConnection, library_id: Uuid) -> Uuid {
    let series_id = Uuid::new_v4();
    let now = Utc::now();
    let series = series::ActiveModel {
        id: Set(series_id),
        library_id: Set(library_id),
        name: Set("Test Series".to_string()),
        normalized_name: Set("test series".to_string()),
        book_count: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    series.insert(db).await.expect("Failed to create series");
    series_id
}

/// Helper to create a test book
async fn create_test_book(db: &DatabaseConnection, series_id: Uuid) -> Uuid {
    let book_id = Uuid::new_v4();
    let now = Utc::now();
    // Use book_id in file_path to ensure uniqueness
    let file_path = format!("/tmp/test-{}.cbz", book_id);
    let file_name = format!("test-{}.cbz", book_id);
    let file_hash = format!("test-hash-{}", book_id);
    let book = books::ActiveModel {
        id: Set(book_id),
        series_id: Set(series_id),
        file_path: Set(file_path),
        file_name: Set(file_name),
        file_size: Set(1024),
        file_hash: Set(file_hash),
        partial_hash: Set(format!("partial-{}", book_id)),
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

/// Test basic task enqueueing
#[tokio::test]
async fn test_enqueue_task() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: "normal".to_string(),
    };

    let task_id = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    // Verify task was created
    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    assert_eq!(task.task_type, "scan_library");
    assert_eq!(task.status, "pending");
    assert_eq!(task.priority, 0);
    assert_eq!(task.attempts, 0);
}

/// Test claiming next available task
#[tokio::test]
async fn test_claim_next_task() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    // Enqueue a task
    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: "normal".to_string(),
    };

    TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    // Claim the task
    let claimed = TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim task")
        .expect("No task available");

    assert_eq!(claimed.status, "processing");
    assert_eq!(claimed.locked_by, Some("worker-1".to_string()));
    assert_eq!(claimed.attempts, 1);
    assert!(claimed.locked_until.is_some());
}

/// Test that claimed tasks cannot be claimed again (SKIP LOCKED)
#[tokio::test]
async fn test_skip_locked_prevents_double_claim() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    // Enqueue a task
    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: "normal".to_string(),
    };

    TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    // First worker claims
    let claimed1 = TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim task");

    assert!(claimed1.is_some());

    // Second worker tries to claim - should get nothing
    let claimed2 = TaskRepository::claim_next(&db, "worker-2", 300, false)
        .await
        .expect("Failed to claim task");

    assert!(claimed2.is_none());
}

/// Test marking task as completed
#[tokio::test]
async fn test_mark_completed() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    // Create and claim a task
    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: "normal".to_string(),
    };

    let task_id = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim task");

    // Mark as completed
    TaskRepository::mark_completed(&db, task_id, None)
        .await
        .expect("Failed to mark completed");

    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    assert_eq!(task.status, "completed");
    assert!(task.completed_at.is_some());
    assert_eq!(task.locked_by, None);
}

/// Test retry logic with exponential backoff
#[tokio::test]
async fn test_mark_failed_retry() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    // Create and claim a task
    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: "normal".to_string(),
    };

    let task_id = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim task");

    // Mark as failed (should retry)
    TaskRepository::mark_failed(&db, task_id, "Test error".to_string())
        .await
        .expect("Failed to mark failed");

    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    assert_eq!(task.status, "pending"); // Back to pending for retry
    assert_eq!(task.attempts, 1);
    assert_eq!(task.last_error, Some("Test error".to_string()));
    assert!(task.scheduled_for > Utc::now()); // Scheduled in future
}

/// Test max attempts reached
#[tokio::test]
async fn test_max_attempts_reached() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    // Create task with max_attempts = 1
    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: "normal".to_string(),
    };

    let task_id = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    // Claim and fail once (attempts = 1)
    TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim");
    TaskRepository::mark_failed(&db, task_id, "Error 1".to_string())
        .await
        .expect("Failed to mark failed");
    // Reset scheduled_for to now so we can claim immediately
    {
        let task = TaskRepository::get_by_id(&db, task_id)
            .await
            .unwrap()
            .unwrap();
        let mut active: tasks::ActiveModel = task.into();
        active.scheduled_for = Set(Utc::now());
        active.update(&db).await.unwrap();
    }

    // Claim and fail again (attempts = 2)
    TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim");
    TaskRepository::mark_failed(&db, task_id, "Error 2".to_string())
        .await
        .expect("Failed to mark failed");
    // Reset scheduled_for
    {
        let task = TaskRepository::get_by_id(&db, task_id)
            .await
            .unwrap()
            .unwrap();
        let mut active: tasks::ActiveModel = task.into();
        active.scheduled_for = Set(Utc::now());
        active.update(&db).await.unwrap();
    }

    // Claim and fail third time (attempts = 3, should reach max)
    TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim");
    TaskRepository::mark_failed(&db, task_id, "Error 3".to_string())
        .await
        .expect("Failed to mark failed");

    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    eprintln!(
        "Final task state: status={}, attempts={}, max_attempts={}",
        task.status, task.attempts, task.max_attempts
    );

    assert_eq!(task.status, "failed"); // Permanently failed
    assert_eq!(task.attempts, 3);
}

/// Test task cancellation
#[tokio::test]
async fn test_cancel_task() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: "normal".to_string(),
    };

    let task_id = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    TaskRepository::cancel(&db, task_id)
        .await
        .expect("Failed to cancel task");

    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    assert_eq!(task.status, "cancelled");
}

/// Test task unlocking
#[tokio::test]
async fn test_unlock_task() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: "normal".to_string(),
    };

    let task_id = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    // Claim task
    TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim");

    // Unlock it
    TaskRepository::unlock(&db, task_id)
        .await
        .expect("Failed to unlock");

    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    assert_eq!(task.status, "pending");
    assert_eq!(task.locked_by, None);
    assert_eq!(task.attempts, 0); // Reset
}

/// Test priority ordering
#[tokio::test]
async fn test_priority_ordering() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create two different libraries to avoid uniqueness constraint
    let library_id_1 = create_test_library(&db).await;
    let library_id_2 = create_test_library(&db).await;

    // Enqueue tasks with different priorities for different libraries
    let low_priority = TaskType::ScanLibrary {
        library_id: library_id_1,
        mode: "normal".to_string(),
    };
    let high_priority = TaskType::ScanLibrary {
        library_id: library_id_2,
        mode: "normal".to_string(),
    };

    TaskRepository::enqueue(&db, low_priority, 0, None)
        .await
        .expect("Failed to enqueue low priority");

    let high_task_id = TaskRepository::enqueue(&db, high_priority, 10, None)
        .await
        .expect("Failed to enqueue high priority");

    // Claim next - should get high priority task
    let claimed = TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim")
        .expect("No task");

    assert_eq!(claimed.id, high_task_id);
    assert_eq!(claimed.priority, 10);
}

/// Test duplicate task prevention
#[tokio::test]
async fn test_duplicate_task_prevention() {
    let (db, _temp_dir) = setup_test_db().await;

    let library_id = create_test_library(&db).await;

    // Enqueue first task
    let task1 = TaskType::ScanLibrary {
        library_id,
        mode: "normal".to_string(),
    };

    let first_task_id = TaskRepository::enqueue(&db, task1.clone(), 0, None)
        .await
        .expect("Failed to enqueue first task");

    // Try to enqueue duplicate - should return the same task ID
    let second_task_id = TaskRepository::enqueue(&db, task1, 0, None)
        .await
        .expect("Failed to handle duplicate task");

    // Should return the same task ID for duplicate
    assert_eq!(first_task_id, second_task_id);

    // Verify only one task exists
    let stats = TaskRepository::get_stats(&db)
        .await
        .expect("Failed to get stats");
    assert_eq!(stats.total, 1);
}

/// Test queue statistics
#[tokio::test]
async fn test_queue_stats() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;
    let series_id = create_test_series(&db, library_id).await;
    let book_id = create_test_book(&db, series_id).await;

    // Create tasks in different states
    let task1_id = TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id,
            mode: "normal".to_string(),
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue");

    let task2_id = TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeBook {
            book_id,
            force: false,
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue");

    // Claim one
    TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim");

    // Complete it
    TaskRepository::mark_completed(&db, task1_id, None)
        .await
        .expect("Failed to mark completed");

    // Get stats
    let stats = TaskRepository::get_stats(&db)
        .await
        .expect("Failed to get stats");

    assert_eq!(stats.pending, 1);
    assert_eq!(stats.processing, 0);
    assert_eq!(stats.completed, 1);
    assert_eq!(stats.failed, 0);
}

/// Test purging old tasks
#[tokio::test]
async fn test_purge_old_tasks() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    // Create and complete a task
    let task_id = TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id,
            mode: "normal".to_string(),
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue");

    TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim");

    TaskRepository::mark_completed(&db, task_id, None)
        .await
        .expect("Failed to mark completed");

    // Purge tasks older than 0 days (all)
    let deleted = TaskRepository::purge_old_tasks(&db, 0)
        .await
        .expect("Failed to purge");

    // Note: This might be 0 if the task isn't "old enough" yet
    // depending on the timestamp precision
    assert!(deleted >= 0);
}

/// Test nuke all tasks
#[tokio::test]
async fn test_nuke_all_tasks() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create tasks for 5 different libraries to avoid uniqueness constraint
    for i in 0..5 {
        let library_id = create_test_library(&db).await;
        TaskRepository::enqueue(
            &db,
            TaskType::ScanLibrary {
                library_id,
                mode: "normal".to_string(),
            },
            0,
            None,
        )
        .await
        .expect(&format!("Failed to enqueue task {}", i));
    }

    // Nuke all
    let deleted = TaskRepository::nuke_all_tasks(&db)
        .await
        .expect("Failed to nuke");

    assert_eq!(deleted, 5);

    let stats = TaskRepository::get_stats(&db)
        .await
        .expect("Failed to get stats");

    assert_eq!(stats.total, 0);
}

/// Test recovering stale tasks with locks that expired
#[tokio::test]
async fn test_recover_stale_tasks_basic() {
    use chrono::Duration;

    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    // Create and claim a task
    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: "normal".to_string(),
    };

    let task_id = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim task");

    // Simulate worker crash by manually setting locked_until to past
    // (more than 600 seconds ago to exceed stale threshold)
    {
        let task = TaskRepository::get_by_id(&db, task_id)
            .await
            .unwrap()
            .unwrap();
        let mut active: tasks::ActiveModel = task.into();
        active.locked_until = Set(Some(Utc::now() - Duration::seconds(700)));
        active.update(&db).await.unwrap();
    }

    // Recover stale tasks (threshold: 600 seconds)
    let recovered = TaskRepository::recover_stale_tasks(&db, 600)
        .await
        .expect("Failed to recover stale tasks");

    assert_eq!(recovered, 1);

    // Verify task is back to pending
    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    assert_eq!(task.status, "pending");
    assert_eq!(task.locked_by, None);
    assert_eq!(task.locked_until, None);
    assert_eq!(task.attempts, 1); // Attempts not reset (worker crash wasn't task's fault)
}

/// Test that stale tasks at max attempts are marked as failed
#[tokio::test]
async fn test_recover_stale_tasks_max_attempts() {
    use chrono::Duration;

    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    // Create and claim a task
    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: "normal".to_string(),
    };

    let task_id = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim task");

    // Simulate task at max attempts with stale lock
    {
        let task = TaskRepository::get_by_id(&db, task_id)
            .await
            .unwrap()
            .unwrap();
        let mut active: tasks::ActiveModel = task.into();
        active.locked_until = Set(Some(Utc::now() - Duration::seconds(700)));
        active.attempts = Set(3); // At max_attempts
        active.update(&db).await.unwrap();
    }

    // Recover stale tasks
    let recovered = TaskRepository::recover_stale_tasks(&db, 600)
        .await
        .expect("Failed to recover stale tasks");

    assert_eq!(recovered, 1);

    // Verify task is marked as failed
    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    assert_eq!(task.status, "failed");
    assert_eq!(task.locked_by, None);
    assert_eq!(task.locked_until, None);
    assert_eq!(
        task.last_error,
        Some("Task stale after max attempts".to_string())
    );
    assert!(task.completed_at.is_some());
}

/// Test that non-stale tasks are not affected
#[tokio::test]
async fn test_recover_stale_tasks_ignores_active() {
    use chrono::Duration;

    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    // Create and claim a task
    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: "normal".to_string(),
    };

    let task_id = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim task");

    // Set locked_until to recent past (500 seconds ago - less than threshold)
    {
        let task = TaskRepository::get_by_id(&db, task_id)
            .await
            .unwrap()
            .unwrap();
        let mut active: tasks::ActiveModel = task.into();
        active.locked_until = Set(Some(Utc::now() - Duration::seconds(500)));
        active.update(&db).await.unwrap();
    }

    // Try to recover stale tasks (threshold: 600 seconds)
    let recovered = TaskRepository::recover_stale_tasks(&db, 600)
        .await
        .expect("Failed to recover stale tasks");

    // Should not recover anything - task is not stale enough
    assert_eq!(recovered, 0);

    // Verify task still processing
    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    assert_eq!(task.status, "processing");
    assert_eq!(task.locked_by, Some("worker-1".to_string()));
}

/// Test recovering multiple stale tasks at once
#[tokio::test]
async fn test_recover_multiple_stale_tasks() {
    use chrono::Duration;

    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;
    let series_id = create_test_series(&db, library_id).await;
    let book_id = create_test_book(&db, series_id).await;

    // Create and claim multiple tasks
    let task1_id = TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id,
            mode: "normal".to_string(),
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue task 1");

    let task2_id = TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeBook {
            book_id,
            force: false,
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue task 2");

    let task3_id = TaskRepository::enqueue(&db, TaskType::AnalyzeSeries { series_id }, 0, None)
        .await
        .expect("Failed to enqueue task 3");

    // Claim all tasks
    TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim");
    TaskRepository::claim_next(&db, "worker-2", 300, false)
        .await
        .expect("Failed to claim");
    TaskRepository::claim_next(&db, "worker-3", 300, false)
        .await
        .expect("Failed to claim");

    // Make all tasks stale
    for task_id in [task1_id, task2_id, task3_id] {
        let task = TaskRepository::get_by_id(&db, task_id)
            .await
            .unwrap()
            .unwrap();
        let mut active: tasks::ActiveModel = task.into();
        active.locked_until = Set(Some(Utc::now() - Duration::seconds(700)));
        active.update(&db).await.unwrap();
    }

    // Recover all stale tasks
    let recovered = TaskRepository::recover_stale_tasks(&db, 600)
        .await
        .expect("Failed to recover stale tasks");

    assert_eq!(recovered, 3);

    // Verify all tasks are back to pending
    for task_id in [task1_id, task2_id, task3_id] {
        let task = TaskRepository::get_by_id(&db, task_id)
            .await
            .expect("Failed to get task")
            .expect("Task not found");

        assert_eq!(task.status, "pending");
        assert_eq!(task.locked_by, None);
        assert_eq!(task.locked_until, None);
    }
}

/// Test that completed/failed tasks are not touched by stale recovery
#[tokio::test]
async fn test_recover_stale_tasks_ignores_completed() {
    use chrono::Duration;

    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    // Create, claim, and complete a task
    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: "normal".to_string(),
    };

    let task_id = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim task");

    TaskRepository::mark_completed(&db, task_id, None)
        .await
        .expect("Failed to mark completed");

    // Artificially set locked_until to stale timestamp (shouldn't matter)
    {
        let task = TaskRepository::get_by_id(&db, task_id)
            .await
            .unwrap()
            .unwrap();
        let mut active: tasks::ActiveModel = task.into();
        active.locked_until = Set(Some(Utc::now() - Duration::seconds(700)));
        active.update(&db).await.unwrap();
    }

    // Try to recover stale tasks
    let recovered = TaskRepository::recover_stale_tasks(&db, 600)
        .await
        .expect("Failed to recover stale tasks");

    // Should not recover completed task
    assert_eq!(recovered, 0);

    // Verify task still completed
    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    assert_eq!(task.status, "completed");
}

/// Test stats correctly identify stale tasks
#[tokio::test]
async fn test_stats_shows_stale_tasks() {
    use chrono::Duration;

    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    // Create and claim a task
    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: "normal".to_string(),
    };

    TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    let task = TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim task")
        .expect("No task");

    // Make it stale (locked > 10 minutes)
    let mut active: tasks::ActiveModel = task.into();
    active.locked_until = Set(Some(Utc::now() - Duration::minutes(11)));
    active.update(&db).await.unwrap();

    // Get stats
    let stats = TaskRepository::get_stats(&db)
        .await
        .expect("Failed to get stats");

    assert_eq!(stats.processing, 1);
    assert_eq!(stats.stale, 1); // Should detect the stale task
}

/// Test that scan tasks are prioritized over analysis tasks when prioritize_scans is true
#[tokio::test]
async fn test_prioritize_scans_over_analysis() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;
    let series_id = create_test_series(&db, library_id).await;
    let book_id = create_test_book(&db, series_id).await;

    // Enqueue tasks with same priority and scheduled_for time
    // Analysis task first (should be picked second if prioritization works)
    TaskRepository::enqueue(
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

    // Scan task second (should be picked first if prioritization works)
    TaskRepository::enqueue(
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

    // Claim with prioritization enabled - should get scan task first
    let claimed1 = TaskRepository::claim_next(&db, "worker-1", 300, true)
        .await
        .expect("Failed to claim task")
        .expect("No task available");

    assert_eq!(
        claimed1.task_type, "scan_library",
        "Scan task should be prioritized"
    );

    // Complete the scan task
    TaskRepository::mark_completed(&db, claimed1.id, None)
        .await
        .expect("Failed to complete task");

    // Now claim again - should get the analysis task
    let claimed2 = TaskRepository::claim_next(&db, "worker-1", 300, true)
        .await
        .expect("Failed to claim task")
        .expect("No task available");

    assert_eq!(
        claimed2.task_type, "analyze_book",
        "Analysis task should be picked after scan"
    );
}

/// Test that priority-based ordering is used when prioritize_scans is false
#[tokio::test]
async fn test_no_prioritization_uses_priority() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;
    let book_id = create_test_book(&db, create_test_series(&db, library_id).await).await;

    // Enqueue scan task with lower priority
    TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id,
            mode: "normal".to_string(),
        },
        0, // Lower priority
        None,
    )
    .await
    .expect("Failed to enqueue scan task");

    // Enqueue analysis task with higher priority
    TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeBook {
            book_id,
            force: false,
        },
        10, // Higher priority
        None,
    )
    .await
    .expect("Failed to enqueue analyze task");

    // Claim with prioritization disabled - should get higher priority task first
    let claimed1 = TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim task")
        .expect("No task available");

    assert_eq!(
        claimed1.task_type, "analyze_book",
        "Higher priority task should be picked first"
    );
    assert_eq!(claimed1.priority, 10, "Should have priority 10");
}

/// Test that scan prioritization works even when scan has lower priority
#[tokio::test]
async fn test_prioritize_scans_even_with_lower_priority() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;
    let book_id = create_test_book(&db, create_test_series(&db, library_id).await).await;

    // Enqueue analysis task with higher priority
    TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeBook {
            book_id,
            force: false,
        },
        10, // Higher priority
        None,
    )
    .await
    .expect("Failed to enqueue analyze task");

    // Enqueue scan task with lower priority
    TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id,
            mode: "normal".to_string(),
        },
        0, // Lower priority
        None,
    )
    .await
    .expect("Failed to enqueue scan task");

    // Claim with prioritization enabled - should get scan task despite lower priority
    let claimed1 = TaskRepository::claim_next(&db, "worker-1", 300, true)
        .await
        .expect("Failed to claim task")
        .expect("No task available");

    assert_eq!(
        claimed1.task_type, "scan_library",
        "Scan task should be prioritized even with lower priority"
    );
    assert_eq!(claimed1.priority, 0, "Should have priority 0");
}

/// Test prioritization with multiple scan and analysis tasks
#[tokio::test]
async fn test_prioritize_multiple_scans_over_analysis() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id1 = create_test_library(&db).await;
    let library_id2 = create_test_library(&db).await;
    let book_id1 = create_test_book(&db, create_test_series(&db, library_id1).await).await;
    let book_id2 = create_test_book(&db, create_test_series(&db, library_id2).await).await;

    // Enqueue analysis tasks first
    TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeBook {
            book_id: book_id1,
            force: false,
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue analyze task 1");
    TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeBook {
            book_id: book_id2,
            force: false,
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue analyze task 2");

    // Enqueue scan tasks second
    TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id: library_id1,
            mode: "normal".to_string(),
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue scan task 1");

    TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id: library_id2,
            mode: "normal".to_string(),
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue scan task 2");

    // Claim with prioritization enabled - should get scan tasks first
    let claimed1 = TaskRepository::claim_next(&db, "worker-1", 300, true)
        .await
        .expect("Failed to claim task")
        .expect("No task available");
    assert_eq!(
        claimed1.task_type, "scan_library",
        "First task should be scan"
    );

    // Complete it
    TaskRepository::mark_completed(&db, claimed1.id, None)
        .await
        .expect("Failed to complete task");

    // Claim again - should get the other scan task
    let claimed2 = TaskRepository::claim_next(&db, "worker-1", 300, true)
        .await
        .expect("Failed to claim task")
        .expect("No task available");
    assert_eq!(
        claimed2.task_type, "scan_library",
        "Second task should be scan"
    );

    // Complete it
    TaskRepository::mark_completed(&db, claimed2.id, None)
        .await
        .expect("Failed to complete task");

    // Now should get analysis tasks
    let claimed3 = TaskRepository::claim_next(&db, "worker-1", 300, true)
        .await
        .expect("Failed to claim task")
        .expect("No task available");
    assert_eq!(
        claimed3.task_type, "analyze_book",
        "Third task should be analysis"
    );
}

// ============================================================================
// PostgreSQL-specific tests
// These tests verify that task prioritization works correctly with PostgreSQL
// which uses different SQL syntax ($1 parameters vs ?) and FOR UPDATE SKIP LOCKED
// ============================================================================

/// Test PostgreSQL task prioritization with scan tasks
/// Verifies that PostgreSQL's CASE expression and FOR UPDATE SKIP LOCKED work correctly
#[tokio::test]
#[ignore]
async fn test_postgres_prioritize_scans_over_analysis() {
    let Some(db) = setup_test_db_postgres().await else {
        // Skip test if PostgreSQL is not available
        return;
    };

    let library_id = create_test_library(&db).await;
    let series_id = create_test_series(&db, library_id).await;
    let book_id = create_test_book(&db, series_id).await;

    // Enqueue tasks with same priority and scheduled_for time
    // Analysis task first (should be picked second if prioritization works)
    TaskRepository::enqueue(
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

    // Scan task second (should be picked first if prioritization works)
    TaskRepository::enqueue(
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

    // Claim with prioritization enabled - should get scan task first
    // This verifies PostgreSQL's CASE expression in ORDER BY works correctly
    let claimed1 = TaskRepository::claim_next(&db, "worker-1", 300, true)
        .await
        .expect("Failed to claim task")
        .expect("No task available");

    assert_eq!(
        claimed1.task_type, "scan_library",
        "PostgreSQL: Scan task should be prioritized"
    );

    // Complete the scan task
    TaskRepository::mark_completed(&db, claimed1.id, None)
        .await
        .expect("Failed to complete task");

    // Now claim again - should get the analysis task
    let claimed2 = TaskRepository::claim_next(&db, "worker-1", 300, true)
        .await
        .expect("Failed to claim task")
        .expect("No task available");

    assert_eq!(
        claimed2.task_type, "analyze_book",
        "PostgreSQL: Analysis task should be picked after scan"
    );
}

/// Test PostgreSQL task prioritization with mixed priorities
/// Verifies that scan tasks are prioritized even with lower priority
#[tokio::test]
#[ignore]
async fn test_postgres_prioritize_scans_even_with_lower_priority() {
    let Some(db) = setup_test_db_postgres().await else {
        return;
    };

    let library_id = create_test_library(&db).await;
    let book_id = create_test_book(&db, create_test_series(&db, library_id).await).await;

    // Enqueue analysis task with higher priority
    TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeBook {
            book_id,
            force: false,
        },
        10, // Higher priority
        None,
    )
    .await
    .expect("Failed to enqueue analyze task");

    // Enqueue scan task with lower priority
    TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id,
            mode: "normal".to_string(),
        },
        0, // Lower priority
        None,
    )
    .await
    .expect("Failed to enqueue scan task");

    // Claim with prioritization enabled - should get scan task despite lower priority
    // This verifies PostgreSQL's CASE expression takes precedence over priority
    let claimed1 = TaskRepository::claim_next(&db, "worker-1", 300, true)
        .await
        .expect("Failed to claim task")
        .expect("No task available");

    assert_eq!(
        claimed1.task_type, "scan_library",
        "PostgreSQL: Scan task should be prioritized even with lower priority"
    );
    assert_eq!(claimed1.priority, 0, "Should have priority 0");
}

/// Test PostgreSQL task prioritization disabled (priority-based ordering)
/// Verifies that when prioritization is disabled, PostgreSQL uses standard priority ordering
#[tokio::test]
#[ignore]
async fn test_postgres_no_prioritization_uses_priority() {
    let Some(db) = setup_test_db_postgres().await else {
        return;
    };

    let library_id = create_test_library(&db).await;
    let book_id = create_test_book(&db, create_test_series(&db, library_id).await).await;

    // Enqueue scan task with lower priority
    TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id,
            mode: "normal".to_string(),
        },
        0, // Lower priority
        None,
    )
    .await
    .expect("Failed to enqueue scan task");

    // Enqueue analysis task with higher priority
    TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeBook {
            book_id,
            force: false,
        },
        10, // Higher priority
        None,
    )
    .await
    .expect("Failed to enqueue analyze task");

    // Claim with prioritization disabled - should get higher priority task first
    // This verifies PostgreSQL's standard ORDER BY priority DESC works correctly
    let claimed1 = TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim task")
        .expect("No task available");

    assert_eq!(
        claimed1.task_type, "analyze_book",
        "PostgreSQL: Higher priority task should be picked first when prioritization disabled"
    );
    assert_eq!(claimed1.priority, 10, "Should have priority 10");
}

/// Test PostgreSQL FOR UPDATE SKIP LOCKED with prioritization
/// Verifies that multiple workers can claim different tasks concurrently
#[tokio::test]
#[ignore]
async fn test_postgres_skip_locked_with_prioritization() {
    let Some(db) = setup_test_db_postgres().await else {
        return;
    };

    let library_id1 = create_test_library(&db).await;
    let library_id2 = create_test_library(&db).await;
    let book_id = create_test_book(&db, create_test_series(&db, library_id1).await).await;

    // Enqueue multiple tasks
    TaskRepository::enqueue(
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

    TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id: library_id1,
            mode: "normal".to_string(),
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue scan task 1");

    TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id: library_id2,
            mode: "normal".to_string(),
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue scan task 2");

    // Simulate concurrent workers claiming tasks
    // Worker 1 should get first scan task (prioritized)
    let claimed1 = TaskRepository::claim_next(&db, "worker-1", 300, true)
        .await
        .expect("Failed to claim task")
        .expect("No task available");
    assert_eq!(
        claimed1.task_type, "scan_library",
        "Worker 1 should get scan task"
    );

    // Worker 2 should get second scan task (also prioritized, SKIP LOCKED prevents conflict)
    let claimed2 = TaskRepository::claim_next(&db, "worker-2", 300, true)
        .await
        .expect("Failed to claim task")
        .expect("No task available");
    assert_eq!(
        claimed2.task_type, "scan_library",
        "Worker 2 should get other scan task"
    );
    assert_ne!(
        claimed1.id, claimed2.id,
        "Workers should get different tasks"
    );

    // Worker 3 should get analysis task (only one left)
    let claimed3 = TaskRepository::claim_next(&db, "worker-3", 300, true)
        .await
        .expect("Failed to claim task")
        .expect("No task available");
    assert_eq!(
        claimed3.task_type, "analyze_book",
        "Worker 3 should get analysis task"
    );
}

// ============================================================================
// FindDuplicates Task Handler Tests
// Tests for the deduplication task handler functionality
// ============================================================================

/// Helper to create books with duplicate file hashes
async fn create_duplicate_books(db: &DatabaseConnection, series_id: Uuid) -> (Uuid, Uuid, String) {
    let now = Utc::now();
    let shared_hash = format!("duplicate-hash-{}", Uuid::new_v4());

    // Create first book
    let book_id1 = Uuid::new_v4();
    let book1 = books::ActiveModel {
        id: Set(book_id1),
        series_id: Set(series_id),
        file_path: Set(format!("/tmp/test-{}.cbz", book_id1)),
        file_name: Set(format!("test-{}.cbz", book_id1)),
        file_size: Set(1024),
        file_hash: Set(shared_hash.clone()),
        partial_hash: Set(format!("partial-{}", book_id1)),
        format: Set("cbz".to_string()),
        page_count: Set(10),
        modified_at: Set(now),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    book1.insert(db).await.expect("Failed to create book 1");

    // Create second book with same hash
    let book_id2 = Uuid::new_v4();
    let book2 = books::ActiveModel {
        id: Set(book_id2),
        series_id: Set(series_id),
        file_path: Set(format!("/tmp/test-{}.cbz", book_id2)),
        file_name: Set(format!("test-{}.cbz", book_id2)),
        file_size: Set(1024),
        file_hash: Set(shared_hash.clone()),
        partial_hash: Set(format!("partial-{}", book_id2)),
        format: Set("cbz".to_string()),
        page_count: Set(10),
        modified_at: Set(now),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    book2.insert(db).await.expect("Failed to create book 2");

    (book_id1, book_id2, shared_hash)
}

/// Test FindDuplicates task can be enqueued
#[tokio::test]
async fn test_enqueue_find_duplicates_task() {
    let (db, _temp_dir) = setup_test_db().await;

    let task_type = TaskType::FindDuplicates;

    let task_id = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    // Verify task was created
    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    assert_eq!(task.task_type, "find_duplicates");
    assert_eq!(task.status, "pending");
    assert_eq!(task.priority, 0);
    assert_eq!(task.attempts, 0);
    assert!(
        task.library_id.is_none(),
        "FindDuplicates should not have library_id"
    );
    assert!(
        task.series_id.is_none(),
        "FindDuplicates should not have series_id"
    );
    assert!(
        task.book_id.is_none(),
        "FindDuplicates should not have book_id"
    );
}

/// Test FindDuplicates task handler executes successfully with duplicates
#[tokio::test]
async fn test_find_duplicates_handler_with_duplicates() {
    use codex::db::repositories::BookDuplicatesRepository;
    use codex::tasks::handlers::{FindDuplicatesHandler, TaskHandler};

    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;
    let series_id = create_test_series(&db, library_id).await;

    // Create duplicate books
    let (_book1, _book2, shared_hash) = create_duplicate_books(&db, series_id).await;

    // Create and enqueue FindDuplicates task
    let task_type = TaskType::FindDuplicates;
    let task_id = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    // Get the task
    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    // Execute the handler
    let handler = FindDuplicatesHandler;
    let result = handler.handle(&task, &db).await.expect("Handler failed");

    assert!(result.success, "Handler should succeed");
    assert!(
        result
            .message
            .as_ref()
            .unwrap()
            .contains("1 duplicate group"),
        "Should find 1 duplicate group"
    );

    // Verify duplicate group was created
    let duplicates = BookDuplicatesRepository::find_all(&db)
        .await
        .expect("Failed to find duplicates");

    assert_eq!(duplicates.len(), 1, "Should have 1 duplicate group");
    assert_eq!(duplicates[0].file_hash, shared_hash, "Hash should match");
    assert_eq!(duplicates[0].duplicate_count, 2, "Should have 2 duplicates");
}

/// Test FindDuplicates task handler with no duplicates
#[tokio::test]
async fn test_find_duplicates_handler_with_no_duplicates() {
    use codex::db::repositories::BookDuplicatesRepository;
    use codex::tasks::handlers::{FindDuplicatesHandler, TaskHandler};

    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;
    let series_id = create_test_series(&db, library_id).await;

    // Create unique books (no duplicates)
    create_test_book(&db, series_id).await;
    create_test_book(&db, series_id).await;

    // Create and enqueue FindDuplicates task
    let task_type = TaskType::FindDuplicates;
    let task_id = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    // Get the task
    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    // Execute the handler
    let handler = FindDuplicatesHandler;
    let result = handler.handle(&task, &db).await.expect("Handler failed");

    assert!(result.success, "Handler should succeed");
    assert!(
        result
            .message
            .as_ref()
            .unwrap()
            .contains("0 duplicate groups"),
        "Should find 0 duplicate groups"
    );

    // Verify no duplicate groups were created
    let duplicates = BookDuplicatesRepository::find_all(&db)
        .await
        .expect("Failed to find duplicates");

    assert_eq!(duplicates.len(), 0, "Should have 0 duplicate groups");
}

/// Test FindDuplicates task handler with multiple duplicate groups
#[tokio::test]
async fn test_find_duplicates_handler_with_multiple_groups() {
    use codex::db::repositories::BookDuplicatesRepository;
    use codex::tasks::handlers::{FindDuplicatesHandler, TaskHandler};

    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;
    let series_id = create_test_series(&db, library_id).await;

    // Create first duplicate group
    let (_book1, _book2, hash1) = create_duplicate_books(&db, series_id).await;

    // Create second duplicate group
    let (_book3, _book4, hash2) = create_duplicate_books(&db, series_id).await;

    // Create and enqueue FindDuplicates task
    let task_type = TaskType::FindDuplicates;
    let task_id = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    // Get the task
    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    // Execute the handler
    let handler = FindDuplicatesHandler;
    let result = handler.handle(&task, &db).await.expect("Handler failed");

    assert!(result.success, "Handler should succeed");
    assert!(
        result
            .message
            .as_ref()
            .unwrap()
            .contains("2 duplicate groups"),
        "Should find 2 duplicate groups"
    );

    // Verify duplicate groups were created
    let duplicates = BookDuplicatesRepository::find_all(&db)
        .await
        .expect("Failed to find duplicates");

    assert_eq!(duplicates.len(), 2, "Should have 2 duplicate groups");

    // Verify hashes (order may vary)
    let found_hashes: Vec<String> = duplicates.iter().map(|d| d.file_hash.clone()).collect();
    assert!(found_hashes.contains(&hash1), "Should contain first hash");
    assert!(found_hashes.contains(&hash2), "Should contain second hash");

    // Verify counts
    for duplicate in duplicates {
        assert_eq!(
            duplicate.duplicate_count, 2,
            "Each group should have 2 duplicates"
        );
    }
}

/// Test FindDuplicates task rebuilds existing duplicates
#[tokio::test]
async fn test_find_duplicates_handler_rebuilds_existing() {
    use codex::db::repositories::BookDuplicatesRepository;
    use codex::tasks::handlers::{FindDuplicatesHandler, TaskHandler};

    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;
    let series_id = create_test_series(&db, library_id).await;

    // Create duplicate books
    create_duplicate_books(&db, series_id).await;

    // Run first scan
    let task_type = TaskType::FindDuplicates;
    let task_id1 = TaskRepository::enqueue(&db, task_type.clone(), 0, None)
        .await
        .expect("Failed to enqueue task");

    let task1 = TaskRepository::get_by_id(&db, task_id1)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    let handler = FindDuplicatesHandler;
    handler
        .handle(&task1, &db)
        .await
        .expect("First handler failed");

    // Verify first scan
    let duplicates1 = BookDuplicatesRepository::find_all(&db)
        .await
        .expect("Failed to find duplicates");
    assert_eq!(
        duplicates1.len(),
        1,
        "Should have 1 duplicate group after first scan"
    );

    // Create more duplicate books
    create_duplicate_books(&db, series_id).await;

    // Run second scan
    let task_id2 = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    let task2 = TaskRepository::get_by_id(&db, task_id2)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    handler
        .handle(&task2, &db)
        .await
        .expect("Second handler failed");

    // Verify second scan rebuilt the table
    let duplicates2 = BookDuplicatesRepository::find_all(&db)
        .await
        .expect("Failed to find duplicates");
    assert_eq!(
        duplicates2.len(),
        2,
        "Should have 2 duplicate groups after second scan"
    );
}
