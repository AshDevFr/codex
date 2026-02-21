mod common;

use codex::db::repositories::TaskRepository;
use codex::tasks::types::TaskType;
use common::setup_test_db;

/// Test that task priority ordering works correctly based on default_priority()
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
    // Enqueue in reverse order (lowest priority first)

    TaskRepository::enqueue(&db, TaskType::CleanupPluginData, None)
        .await
        .unwrap();

    TaskRepository::enqueue(&db, TaskType::CleanupPdfCache, None)
        .await
        .unwrap();

    TaskRepository::enqueue(&db, TaskType::CleanupOrphanedFiles, None)
        .await
        .unwrap();

    TaskRepository::enqueue(&db, TaskType::CleanupSeriesFiles { series_id }, None)
        .await
        .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::CleanupBookFiles {
            book_id,
            thumbnail_path: None,
            series_id: None,
        },
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::UserPluginRecommendations {
            plugin_id: uuid::Uuid::new_v4(),
            user_id: uuid::Uuid::new_v4(),
        },
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::UserPluginSync {
            plugin_id: uuid::Uuid::new_v4(),
            user_id: uuid::Uuid::new_v4(),
        },
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::UserPluginRecommendationDismiss {
            plugin_id: uuid::Uuid::new_v4(),
            user_id: uuid::Uuid::new_v4(),
            external_id: "test".to_string(),
            reason: None,
        },
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::PluginAutoMatch {
            series_id,
            plugin_id: uuid::Uuid::new_v4(),
            source_scope: None,
        },
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::RefreshMetadata {
            book_id,
            source: "test".to_string(),
        },
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(&db, TaskType::FindDuplicates, None)
        .await
        .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::ReprocessSeriesTitles {
            library_id: Some(library_id),
            series_ids: None,
        },
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(&db, TaskType::ReprocessSeriesTitle { series_id }, None)
        .await
        .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::RenumberSeriesBatch {
            series_ids: Some(vec![series_id]),
        },
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(&db, TaskType::RenumberSeries { series_id }, None)
        .await
        .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::GenerateSeriesThumbnails {
            library_id: Some(library_id),
            series_ids: None,
            force: false,
        },
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::GenerateThumbnails {
            library_id: Some(library_id),
            series_id: None,
            series_ids: None,
            book_ids: None,
            force: false,
        },
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::GenerateSeriesThumbnail {
            series_id,
            force: false,
        },
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(&db, TaskType::AnalyzeSeries { series_id }, None)
        .await
        .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeBook {
            book_id,
            force: false,
        },
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::GenerateThumbnail {
            book_id,
            force: false,
        },
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(&db, TaskType::PurgeDeleted { library_id }, None)
        .await
        .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id,
            mode: "normal".to_string(),
        },
        None,
    )
    .await
    .unwrap();

    // Now claim tasks one by one and verify the order
    let expected_order = vec![
        // Scanning (priority 1000-900)
        "scan_library",
        "purge_deleted",
        // Analysis (priority 800-750)
        "analyze_book",
        "analyze_series",
        "reprocess_series_title",
        "reprocess_series_titles",
        "renumber_series",
        "renumber_series_batch",
        // Thumbnails (priority 600-570)
        "generate_thumbnail",
        "generate_series_thumbnail",
        "generate_thumbnails",
        "generate_series_thumbnails",
        // Metadata (priority 400-380)
        "find_duplicates",
        "refresh_metadata",
        "plugin_auto_match",
        // Plugins (priority 200-180)
        "user_plugin_recommendation_dismiss",
        "user_plugin_sync",
        "user_plugin_recommendations",
        // Cleanup (priority 100, FIFO by scheduled_for)
        "cleanup_plugin_data",
        "cleanup_pdf_cache",
        "cleanup_orphaned_files",
        "cleanup_series_files",
        "cleanup_book_files",
    ];

    let mut actual_order = Vec::new();

    for _ in 0..expected_order.len() {
        let task = TaskRepository::claim_next(&db, "test-worker", 300)
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

/// Test that explicit priority override works via enqueue_with_priority
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

    // Enqueue three analyze_book tasks with different explicit priorities
    TaskRepository::enqueue_with_priority(
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

    TaskRepository::enqueue_with_priority(
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

    TaskRepository::enqueue_with_priority(
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
    let task1 = TaskRepository::claim_next(&db, "test-worker", 300)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task1.priority, 50, "Should claim highest priority first");
    assert_eq!(task1.book_id, Some(book_id_2));

    let task2 = TaskRepository::claim_next(&db, "test-worker", 300)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task2.priority, 30, "Should claim medium priority second");
    assert_eq!(task2.book_id, Some(book_id_3));

    let task3 = TaskRepository::claim_next(&db, "test-worker", 300)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task3.priority, 10, "Should claim lowest priority last");
    assert_eq!(task3.book_id, Some(book_id_1));
}

/// Test that default_priority produces correct ordering across different task types
#[tokio::test]
async fn test_default_priority_ordering_across_types() {
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

    // Enqueue tasks in reverse priority order (cleanup first, scan last)
    TaskRepository::enqueue(&db, TaskType::CleanupOrphanedFiles, None)
        .await
        .unwrap();

    TaskRepository::enqueue(&db, TaskType::FindDuplicates, None)
        .await
        .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::GenerateThumbnail {
            book_id,
            force: false,
        },
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeBook {
            book_id,
            force: false,
        },
        None,
    )
    .await
    .unwrap();

    TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id,
            mode: "normal".to_string(),
        },
        None,
    )
    .await
    .unwrap();

    // Scan should come first (1000), then analyze (800), then thumbnail (600),
    // then find_duplicates (400), then cleanup (100)
    let task1 = TaskRepository::claim_next(&db, "test-worker", 300)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task1.task_type, "scan_library");
    assert_eq!(task1.priority, 1000);

    let task2 = TaskRepository::claim_next(&db, "test-worker", 300)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task2.task_type, "analyze_book");
    assert_eq!(task2.priority, 800);

    let task3 = TaskRepository::claim_next(&db, "test-worker", 300)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task3.task_type, "generate_thumbnail");
    assert_eq!(task3.priority, 600);

    let task4 = TaskRepository::claim_next(&db, "test-worker", 300)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task4.task_type, "find_duplicates");
    assert_eq!(task4.priority, 400);

    let task5 = TaskRepository::claim_next(&db, "test-worker", 300)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task5.task_type, "cleanup_orphaned_files");
    assert_eq!(task5.priority, 100);
}
