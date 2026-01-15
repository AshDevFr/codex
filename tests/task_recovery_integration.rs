mod common;

use chrono::{Duration, Utc};
use codex::db::entities::{libraries, tasks};
use codex::db::repositories::TaskRepository;
use codex::tasks::types::TaskType;
use common::setup_test_db;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use std::time::Duration as StdDuration;
use tokio::time::sleep;
use uuid::Uuid;

/// Helper to create a test library
async fn create_test_library(db: &DatabaseConnection) -> Uuid {
    let library_id = Uuid::new_v4();
    let now = Utc::now();
    let library = libraries::ActiveModel {
        id: Set(library_id),
        name: Set("Test Library".to_string()),
        path: Set("/tmp/test-library".to_string()),
        series_strategy: Set("series_volume".to_string()),
        book_strategy: Set("filename".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    library.insert(db).await.expect("Failed to create library");
    library_id
}

/// Test that a worker crash scenario is handled correctly
#[tokio::test]
async fn test_worker_crash_recovery_scenario() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    // Step 1: Enqueue a task
    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: "normal".to_string(),
    };

    let task_id = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue task");

    // Step 2: Worker claims the task
    let claimed_task = TaskRepository::claim_next(&db, "worker-crashed", 300, false)
        .await
        .expect("Failed to claim task")
        .expect("No task available");

    assert_eq!(claimed_task.id, task_id);
    assert_eq!(claimed_task.status, "processing");
    assert_eq!(claimed_task.locked_by, Some("worker-crashed".to_string()));

    // Step 3: Simulate worker crash by setting locked_until to far past
    // (worker crashed and never completed the task)
    {
        let task = TaskRepository::get_by_id(&db, task_id)
            .await
            .unwrap()
            .unwrap();
        let mut active: tasks::ActiveModel = task.into();
        // Set to 15 minutes ago (well past the 10 minute stale threshold)
        active.locked_until = Set(Some(Utc::now() - Duration::minutes(15)));
        active.update(&db).await.unwrap();
    }

    // Step 4: Verify task shows as stale in stats
    let stats_before = TaskRepository::get_stats(&db)
        .await
        .expect("Failed to get stats");
    assert_eq!(stats_before.processing, 1);
    assert_eq!(stats_before.stale, 1);
    assert_eq!(stats_before.pending, 0);

    // Step 5: Run stale task recovery
    let recovered = TaskRepository::recover_stale_tasks(&db, 600)
        .await
        .expect("Failed to recover stale tasks");

    assert_eq!(recovered, 1);

    // Step 6: Verify task is back to pending and available
    let recovered_task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    assert_eq!(recovered_task.status, "pending");
    assert_eq!(recovered_task.locked_by, None);
    assert_eq!(recovered_task.locked_until, None);
    assert_eq!(recovered_task.attempts, 1); // Still has 1 attempt from crash

    // Step 7: New worker can claim and complete the task
    let reclaimed_task = TaskRepository::claim_next(&db, "worker-healthy", 300, false)
        .await
        .expect("Failed to reclaim task")
        .expect("No task available");

    assert_eq!(reclaimed_task.id, task_id);
    assert_eq!(reclaimed_task.locked_by, Some("worker-healthy".to_string()));
    assert_eq!(reclaimed_task.attempts, 2); // Incremented on reclaim

    TaskRepository::mark_completed(&db, task_id, None)
        .await
        .expect("Failed to complete task");

    // Step 8: Verify final state
    let final_task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    assert_eq!(final_task.status, "completed");
    assert!(final_task.completed_at.is_some());

    let stats_after = TaskRepository::get_stats(&db)
        .await
        .expect("Failed to get stats");
    assert_eq!(stats_after.completed, 1);
    assert_eq!(stats_after.stale, 0);
}

/// Test multiple workers crashing and all tasks being recovered
#[tokio::test]
async fn test_multiple_worker_crashes() {
    let (db, _temp_dir) = setup_test_db().await;
    let _library_id = create_test_library(&db).await;

    // Create 5 tasks
    let mut task_ids = Vec::new();
    // Create 5 different libraries to avoid uniqueness constraint
    for _ in 0..5 {
        let lib_id = create_test_library(&db).await;
        let task_id = TaskRepository::enqueue(
            &db,
            TaskType::ScanLibrary {
                library_id: lib_id,
                mode: "normal".to_string(),
            },
            0,
            None,
        )
        .await
        .expect("Failed to enqueue task");
        task_ids.push(task_id);
    }

    // 5 different workers claim them
    for i in 0..5 {
        let worker_name = format!("worker-{}", i);
        TaskRepository::claim_next(&db, &worker_name, 300, false)
            .await
            .expect("Failed to claim task");
    }

    // All workers crash - make all locks stale
    for task_id in &task_ids {
        let task = TaskRepository::get_by_id(&db, *task_id)
            .await
            .unwrap()
            .unwrap();
        let mut active: tasks::ActiveModel = task.into();
        active.locked_until = Set(Some(Utc::now() - Duration::minutes(15)));
        active.update(&db).await.unwrap();
    }

    // Recover all stale tasks
    let recovered = TaskRepository::recover_stale_tasks(&db, 600)
        .await
        .expect("Failed to recover stale tasks");

    assert_eq!(recovered, 5);

    // Verify all are back to pending
    for task_id in &task_ids {
        let task = TaskRepository::get_by_id(&db, *task_id)
            .await
            .expect("Failed to get task")
            .expect("Task not found");

        assert_eq!(task.status, "pending");
        assert_eq!(task.locked_by, None);
    }
}

/// Test that tasks at max attempts are marked failed, not recovered
#[tokio::test]
async fn test_crashed_task_at_max_attempts() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

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
    .expect("Failed to enqueue task");

    // Claim task
    TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim");

    // Simulate task that has already failed twice and is on its 3rd attempt
    {
        let task = TaskRepository::get_by_id(&db, task_id)
            .await
            .unwrap()
            .unwrap();
        let mut active: tasks::ActiveModel = task.into();
        active.attempts = Set(3); // At max_attempts
        active.locked_until = Set(Some(Utc::now() - Duration::minutes(15)));
        active.update(&db).await.unwrap();
    }

    // Recover stale tasks
    let recovered = TaskRepository::recover_stale_tasks(&db, 600)
        .await
        .expect("Failed to recover stale tasks");

    assert_eq!(recovered, 1);

    // Verify task is marked as permanently failed
    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    assert_eq!(task.status, "failed");
    assert_eq!(
        task.last_error,
        Some("Task stale after max attempts".to_string())
    );
    assert!(task.completed_at.is_some());
    assert_eq!(task.locked_by, None);

    // Should not be claimable again
    let claim_attempt = TaskRepository::claim_next(&db, "worker-2", 300, false)
        .await
        .expect("Failed to query tasks");

    assert!(claim_attempt.is_none());
}

/// Test periodic recovery with simulated background worker
#[tokio::test]
async fn test_periodic_stale_recovery() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    // Enqueue a task
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
    .expect("Failed to enqueue task");

    // Claim it
    TaskRepository::claim_next(&db, "worker-crashed", 300, false)
        .await
        .expect("Failed to claim");

    // Make it stale
    {
        let task = TaskRepository::get_by_id(&db, task_id)
            .await
            .unwrap()
            .unwrap();
        let mut active: tasks::ActiveModel = task.into();
        active.locked_until = Set(Some(Utc::now() - Duration::minutes(15)));
        active.update(&db).await.unwrap();
    }

    // Simulate background recovery running (normally runs every 60 seconds)
    // Run it a few times to ensure it's idempotent
    for i in 0..3 {
        let recovered = TaskRepository::recover_stale_tasks(&db, 600)
            .await
            .expect("Failed to recover stale tasks");

        if i == 0 {
            // First run should recover the task
            assert_eq!(recovered, 1);
        } else {
            // Subsequent runs should find nothing to recover
            assert_eq!(recovered, 0);
        }

        sleep(StdDuration::from_millis(100)).await;
    }

    // Verify task is in pending state
    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    assert_eq!(task.status, "pending");
}

/// Test that active tasks with valid locks are never recovered
#[tokio::test]
async fn test_active_tasks_not_recovered() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    // Create and claim a task
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
    .expect("Failed to enqueue task");

    let claimed_task = TaskRepository::claim_next(&db, "worker-active", 300, false)
        .await
        .expect("Failed to claim")
        .expect("No task");

    // Task has a valid lock (expires in the future)
    assert!(claimed_task.locked_until.unwrap() > Utc::now());

    // Try to recover stale tasks (with very aggressive 1 second threshold)
    let recovered = TaskRepository::recover_stale_tasks(&db, 1)
        .await
        .expect("Failed to recover stale tasks");

    // Should recover nothing - task lock is still valid
    assert_eq!(recovered, 0);

    // Verify task is still processing
    let task = TaskRepository::get_by_id(&db, task_id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");

    assert_eq!(task.status, "processing");
    assert_eq!(task.locked_by, Some("worker-active".to_string()));
}

/// Test recovery with mixed task states
#[tokio::test]
async fn test_recovery_with_mixed_states() {
    use sea_orm::ConnectionTrait;

    let (db, _temp_dir) = setup_test_db().await;

    // Create 4 different libraries to avoid uniqueness constraint
    let library_id_stale = create_test_library(&db).await;
    let library_id_active = create_test_library(&db).await;
    let library_id_completed = create_test_library(&db).await;
    let library_id_pending = create_test_library(&db).await;

    // Create 4 tasks in different states
    TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id: library_id_stale,
            mode: "normal".to_string(),
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue");

    TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id: library_id_active,
            mode: "normal".to_string(),
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue");

    TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id: library_id_completed,
            mode: "normal".to_string(),
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue");

    TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id: library_id_pending,
            mode: "normal".to_string(),
        },
        0,
        None,
    )
    .await
    .expect("Failed to enqueue");

    // Stale task: claimed but lock expired long ago
    let claimed_stale = TaskRepository::claim_next(&db, "worker-crashed", 300, false)
        .await
        .expect("Failed to claim")
        .expect("No task");
    let stale_task_id = claimed_stale.id;

    // Active task: claimed with valid lock
    let _claimed_active = TaskRepository::claim_next(&db, "worker-healthy", 300, false)
        .await
        .expect("Failed to claim")
        .expect("No task");

    // Completed task: finished successfully
    let claimed_completed = TaskRepository::claim_next(&db, "worker-done", 300, false)
        .await
        .expect("Failed to claim")
        .expect("No task");
    TaskRepository::mark_completed(&db, claimed_completed.id, None)
        .await
        .expect("Failed to complete");

    // Now make the first task stale using raw SQL to ensure immediate commit
    // This bypasses any ORM caching/transaction issues
    let stale_time = Utc::now() - Duration::minutes(15);
    db.execute(sea_orm::Statement::from_sql_and_values(
        db.get_database_backend(),
        "UPDATE tasks SET locked_until = $1 WHERE id = $2",
        vec![stale_time.into(), stale_task_id.into()],
    ))
    .await
    .expect("Failed to update task");

    // Pending task: never claimed (already pending from enqueue)

    // Run recovery
    let recovered = TaskRepository::recover_stale_tasks(&db, 600)
        .await
        .expect("Failed to recover stale tasks");

    // Only the stale task should be recovered
    assert_eq!(recovered, 1, "Expected to recover 1 stale task");

    // Get all tasks to verify states
    let all_tasks = TaskRepository::list(&db, None, None, None)
        .await
        .expect("Failed to list tasks");

    // Count tasks by status
    let mut pending_count = 0;
    let mut processing_count = 0;
    let mut completed_count = 0;

    for task in all_tasks {
        match task.status.as_str() {
            "pending" => pending_count += 1,
            "processing" => processing_count += 1,
            "completed" => completed_count += 1,
            _ => {}
        }
    }

    // Should have:
    // - 2 pending (stale recovered + never claimed)
    // - 1 processing (active task)
    // - 1 completed (completed task)
    assert_eq!(pending_count, 2, "Should have 2 pending tasks");
    assert_eq!(processing_count, 1, "Should have 1 processing task");
    assert_eq!(completed_count, 1, "Should have 1 completed task");
}

/// Test that rescanning a library doesn't create duplicate analyze tasks
#[tokio::test]
async fn test_rescan_no_duplicate_analyze_tasks() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    // For simplicity, use a library-level task instead of book-level
    // since we don't need to create books for this test
    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: "normal".to_string(),
    };

    let first_task_id = TaskRepository::enqueue(&db, task_type.clone(), 0, None)
        .await
        .expect("Failed to enqueue first scan task");

    // Verify task was created
    let stats = TaskRepository::get_stats(&db)
        .await
        .expect("Failed to get stats");
    assert_eq!(stats.total, 1);
    assert_eq!(stats.pending, 1);

    // Try to enqueue the same scan task again (simulating a re-scan)
    let second_task_id = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to handle duplicate scan task");

    // Should return the same task ID
    assert_eq!(
        first_task_id, second_task_id,
        "Duplicate task should return the same task ID"
    );

    // Verify no duplicate was created
    let stats = TaskRepository::get_stats(&db)
        .await
        .expect("Failed to get stats");
    assert_eq!(stats.total, 1, "Should still have only 1 task");
    assert_eq!(stats.pending, 1, "Should still have only 1 pending task");
}

/// Test that rescanning multiple libraries doesn't create duplicates
#[tokio::test]
async fn test_rescan_multiple_libraries_no_duplicates() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create 3 different libraries
    let mut library_ids = Vec::new();
    for _ in 0..3 {
        let lib_id = create_test_library(&db).await;
        library_ids.push(lib_id);
    }

    // First scan - enqueue scan tasks for all libraries
    for library_id in &library_ids {
        TaskRepository::enqueue(
            &db,
            TaskType::ScanLibrary {
                library_id: *library_id,
                mode: "normal".to_string(),
            },
            0,
            None,
        )
        .await
        .expect("Failed to enqueue task");
    }

    // Verify 3 tasks were created
    let stats = TaskRepository::get_stats(&db)
        .await
        .expect("Failed to get stats");
    assert_eq!(stats.total, 3, "Should have 3 tasks after first scan");

    // Second scan - try to enqueue the same tasks again
    for library_id in &library_ids {
        TaskRepository::enqueue(
            &db,
            TaskType::ScanLibrary {
                library_id: *library_id,
                mode: "normal".to_string(),
            },
            0,
            None,
        )
        .await
        .expect("Failed to handle duplicate task");
    }

    // Verify still only 3 tasks (no duplicates)
    let stats = TaskRepository::get_stats(&db)
        .await
        .expect("Failed to get stats");
    assert_eq!(
        stats.total, 3,
        "Should still have only 3 tasks after rescan"
    );
    assert_eq!(stats.pending, 3, "All 3 tasks should still be pending");
}

/// Test that completed tasks allow new tasks to be created
#[tokio::test]
async fn test_completed_task_allows_new_task() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_id = create_test_library(&db).await;

    // Enqueue a scan task
    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: "normal".to_string(),
    };
    let first_task_id = TaskRepository::enqueue(&db, task_type.clone(), 0, None)
        .await
        .expect("Failed to enqueue first task");

    // Claim and complete the task
    let claimed = TaskRepository::claim_next(&db, "worker-1", 300, false)
        .await
        .expect("Failed to claim task")
        .expect("No task to claim");
    assert_eq!(claimed.id, first_task_id);

    TaskRepository::mark_completed(&db, first_task_id, None)
        .await
        .expect("Failed to mark task completed");

    // Now enqueue the same task again - should create a new task since the previous one is completed
    let second_task_id = TaskRepository::enqueue(&db, task_type, 0, None)
        .await
        .expect("Failed to enqueue second task");

    // Should have 2 tasks total: 1 completed, 1 pending
    let stats = TaskRepository::get_stats(&db)
        .await
        .expect("Failed to get stats");
    assert_eq!(stats.total, 2, "Should have 2 tasks total");
    assert_eq!(stats.completed, 1, "Should have 1 completed task");
    assert_eq!(stats.pending, 1, "Should have 1 pending task");

    // The new task should be different from the first
    assert_ne!(
        first_task_id, second_task_id,
        "New task should have different ID after previous task completed"
    );
}
