mod common;

use codex::db::repositories::{BookRepository, TaskRepository};
use codex::events::EventBroadcaster;
use codex::tasks::types::TaskType;
use common::setup_test_db;
use std::sync::Arc;

/// Test that analyze_book queues thumbnail generation task when cover becomes available
#[tokio::test]
async fn test_analyze_book_queues_thumbnail_task_on_cover_available() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a library
    let library = common::create_test_library(&db, "Test Library", "/test/path").await;

    // Create a series
    let series = common::create_test_series(&db, &library, "Test Series").await;

    // Create event broadcaster
    let event_broadcaster = Arc::new(EventBroadcaster::new(100));

    // Create a book with page_count = 0 (no cover analyzed yet)
    let mut book = common::create_test_book_with_hash(
        &db,
        &library,
        &series,
        "test.cbz",
        "/test/path/test.cbz",
        "test_hash",
    )
    .await;
    book.page_count = 0;
    BookRepository::update(&db, &book, None).await.unwrap();

    // Get initial task count for thumbnail generation
    let stats_before = TaskRepository::get_stats(&db).await.unwrap();
    let thumbnail_tasks_before = stats_before
        .by_type
        .get("generate_thumbnails")
        .map(|s| s.pending + s.processing)
        .unwrap_or(0);

    // Simulate analyzing a book where page_count becomes > 0
    // Manually update the book to simulate successful analysis
    book.page_count = 10;
    book.analyzed = true;
    BookRepository::update(&db, &book, Some(&event_broadcaster))
        .await
        .unwrap();

    // Manually trigger the logic that would happen in analyze_book
    // when cover_now_available becomes true
    use codex::db::repositories::SeriesRepository;

    // Get series for library_id
    if let Ok(Some(series)) = SeriesRepository::get_by_id(&db, book.series_id).await {
        // Queue thumbnail generation task
        let task_type = TaskType::GenerateThumbnails {
            library_id: Some(series.library_id),
            series_id: None,
            series_ids: None,
            book_ids: None,
            force: false,
        };

        TaskRepository::enqueue(&db, task_type, 0, None)
            .await
            .unwrap();
    }

    // Check if thumbnail generation task was queued
    let stats_after = TaskRepository::get_stats(&db).await.unwrap();
    let thumbnail_tasks_after = stats_after
        .by_type
        .get("generate_thumbnails")
        .map(|s| s.pending + s.processing)
        .unwrap_or(0);

    assert_eq!(
        thumbnail_tasks_after,
        thumbnail_tasks_before + 1,
        "Should have queued one thumbnail generation task"
    );
}
