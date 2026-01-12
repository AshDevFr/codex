mod common;

use codex::db::repositories::TaskRepository;
use codex::tasks::types::TaskType;
use common::setup_test_db;

/// Test that task priority ordering works correctly when prioritize_scans is enabled
#[tokio::test]
async fn test_task_type_priority_ordering() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create test data
    let library = common::create_test_library(&db, "Test Library", "/test/path").await;
    let series = common::create_test_series(&db, &library, "Test Series").await;
    let book = common::create_test_book_with_hash(
        &db,
        &library,
        &series,
        "test.cbz",
        "/test/path/test.cbz",
        "test_hash",
    )
    .await;

    let library_id = library.id;
    let series_id = series.id;
    let book_id = book.id;

    // Enqueue tasks in reverse priority order to ensure ordering is not based on creation time
    // Priority order should be:
    // 1. scan_library
    // 2. purge_deleted
    // 3. analyze_book
    // 4. analyze_series
    // 5. generate_thumbnails
    // 6. find_duplicates
    // 7. refresh_metadata

    // Enqueue in reverse order
    TaskRepository::enqueue(
        &db,
        TaskType::RefreshMetadata {
            book_id,
            source: "test".to_string(),
        },
        0,
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(&db, TaskType::FindDuplicates, 0, None)
        .await
        .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::GenerateThumbnails {
            library_id: Some(library_id),
            series_id: None,
            force: false,
        },
        0,
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(&db, TaskType::AnalyzeSeries { series_id }, 0, None)
        .await
        .unwrap();

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
    .unwrap();

    TaskRepository::enqueue(&db, TaskType::PurgeDeleted { library_id }, 0, None)
        .await
        .unwrap();

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
    .unwrap();

    // Now claim tasks one by one and verify the order
    let expected_order = vec![
        "scan_library",
        "purge_deleted",
        "analyze_book",
        "analyze_series",
        "generate_thumbnails",
        "find_duplicates",
        "refresh_metadata",
    ];

    let mut actual_order = Vec::new();

    for _ in 0..7 {
        let task = TaskRepository::claim_next(&db, "test-worker", 300, true)
            .await
            .unwrap();

        if let Some(task) = task {
            actual_order.push(task.task_type.clone());
        }
    }

    assert_eq!(
        actual_order, expected_order,
        "Tasks should be claimed in priority order"
    );
}

/// Test that priority field still works as secondary sort when task types are the same
#[tokio::test]
async fn test_task_priority_field_with_same_type() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create test data
    let library = common::create_test_library(&db, "Test Library", "/test/path").await;
    let series = common::create_test_series(&db, &library, "Test Series").await;

    let book1 = common::create_test_book_with_hash(
        &db,
        &library,
        &series,
        "test1.cbz",
        "/test/path/test1.cbz",
        "hash1",
    )
    .await;

    let book2 = common::create_test_book_with_hash(
        &db,
        &library,
        &series,
        "test2.cbz",
        "/test/path/test2.cbz",
        "hash2",
    )
    .await;

    let book3 = common::create_test_book_with_hash(
        &db,
        &library,
        &series,
        "test3.cbz",
        "/test/path/test3.cbz",
        "hash3",
    )
    .await;

    let book_id_1 = book1.id;
    let book_id_2 = book2.id;
    let book_id_3 = book3.id;

    // Enqueue three analyze_book tasks with different priorities
    TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeBook {
            book_id: book_id_1,
            force: false,
        },
        10, // Low priority
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeBook {
            book_id: book_id_2,
            force: false,
        },
        50, // High priority
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeBook {
            book_id: book_id_3,
            force: false,
        },
        30, // Medium priority
        None,
    )
    .await
    .unwrap();

    // Claim tasks and verify they come out in priority order (highest first)
    let task1 = TaskRepository::claim_next(&db, "test-worker", 300, true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task1.priority, 50, "Should claim highest priority first");
    assert_eq!(task1.book_id, Some(book_id_2));

    let task2 = TaskRepository::claim_next(&db, "test-worker", 300, true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task2.priority, 30, "Should claim medium priority second");
    assert_eq!(task2.book_id, Some(book_id_3));

    let task3 = TaskRepository::claim_next(&db, "test-worker", 300, true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task3.priority, 10, "Should claim lowest priority last");
    assert_eq!(task3.book_id, Some(book_id_1));
}

/// Test that when prioritize_scans is false, only priority field is used
#[tokio::test]
async fn test_priority_field_only_when_prioritize_scans_disabled() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create test data
    let library = common::create_test_library(&db, "Test Library", "/test/path").await;
    let series = common::create_test_series(&db, &library, "Test Series").await;
    let book = common::create_test_book_with_hash(
        &db,
        &library,
        &series,
        "test.cbz",
        "/test/path/test.cbz",
        "hash",
    )
    .await;

    let library_id = library.id;
    let book_id = book.id;

    // Enqueue scan_library with low priority
    TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id,
            mode: "normal".to_string(),
        },
        10, // Low priority
        None,
    )
    .await
    .unwrap();

    // Enqueue analyze_book with high priority
    TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeBook {
            book_id,
            force: false,
        },
        50, // High priority
        None,
    )
    .await
    .unwrap();

    // Claim with prioritize_scans = false
    let task = TaskRepository::claim_next(&db, "test-worker", 300, false)
        .await
        .unwrap()
        .unwrap();

    // Should get analyze_book because it has higher priority field value
    assert_eq!(
        task.task_type, "analyze_book",
        "Should prioritize by priority field when prioritize_scans is false"
    );
    assert_eq!(task.priority, 50);
}
