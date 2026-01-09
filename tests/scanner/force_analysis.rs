/// Integration tests for force analysis functionality
use anyhow::Result;
use chrono::Utc;
use codex::db::entities::{books, series};
use codex::db::repositories::{BookRepository, SeriesRepository};
use codex::scanner::analyze_book;
use std::path::PathBuf;
use tempfile::TempDir;
use uuid::Uuid;

#[path = "../common/mod.rs"]
mod common;
use common::{files::create_test_cbz, *};

/// Helper to create a test book with file hash
async fn create_analyzed_book(
    db_conn: &sea_orm::DatabaseConnection,
    file_path: &str,
) -> Result<(books::Model, series::Model)> {
    // Create library
    let library = create_test_library(db_conn, "Test Library", "/test/library").await;

    // Create series
    let series = SeriesRepository::create(db_conn, library.id, "Test Series").await?;

    // Create book
    let book = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        title: Some("Test Book".to_string()),
        number: None,
        file_path: file_path.to_string(),
        file_name: "test.cbz".to_string(),
        file_size: 1024,
        file_hash: "existing_hash".to_string(), // Pre-existing hash
        partial_hash: "partial_hash".to_string(),
        format: "cbz".to_string(),
        page_count: 10,
        modified_at: Utc::now(),
        analyzed: true, // Already analyzed
        deleted: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    BookRepository::create(db_conn, &book).await?;

    Ok((book, series))
}

#[tokio::test]
async fn test_analyze_book_with_force_reanalyzes_unchanged_file() -> Result<()> {
    let (db, _temp_dir) = setup_test_db().await;
    let temp_dir = TempDir::new()?;
    let cbz_path = create_test_cbz(&temp_dir, 1, false);

    // Create an already-analyzed book with a fake hash
    let (mut book, _series) = create_analyzed_book(&db, cbz_path.to_str().unwrap()).await?;

    // Update the book to have a specific fake hash to test force re-analysis
    book.file_hash = "fake_hash_12345".to_string();
    book.analyzed = true;
    BookRepository::update(&db, &book).await?;

    // Get the book state before force analysis
    let before_analysis = BookRepository::get_by_id(&db, book.id).await?.unwrap();
    assert_eq!(before_analysis.file_hash, "fake_hash_12345");
    assert!(before_analysis.analyzed);

    // Analyze with force=true - should re-analyze even if we set analyzed=true
    let result = analyze_book(&db, book.id, true).await?;

    // Should successfully analyze 1 book
    assert_eq!(result.books_analyzed, 1);
    assert_eq!(result.errors.len(), 0);

    // Verify the book was re-analyzed (hash should be updated with actual file hash)
    let after_analysis = BookRepository::get_by_id(&db, book.id).await?.unwrap();

    // The hash will have changed because we forced re-analysis
    // (the original was "fake_hash_12345", now it should be the actual file hash)
    assert_ne!(after_analysis.file_hash, "fake_hash_12345");
    assert!(!after_analysis.file_hash.is_empty());
    assert!(after_analysis.analyzed);

    // Page count should be updated to reflect actual content (1 page)
    assert_eq!(after_analysis.page_count, 1);

    Ok(())
}

#[tokio::test]
async fn test_analyze_book_without_force_skips_if_hash_matches() -> Result<()> {
    let (db, _temp_dir) = setup_test_db().await;
    let temp_dir = TempDir::new()?;
    let cbz_path = create_test_cbz(&temp_dir, 1, false);

    // Create a book and analyze it once to get the real hash
    let (book, _series) = create_analyzed_book(&db, cbz_path.to_str().unwrap()).await?;

    // First analysis with force=true to get the actual hash
    let first_result = analyze_book(&db, book.id, true).await?;
    assert_eq!(first_result.books_analyzed, 1);

    let after_first = BookRepository::get_by_id(&db, book.id).await?.unwrap();
    let real_hash = after_first.file_hash.clone();
    assert!(!real_hash.is_empty());
    assert_ne!(real_hash, "existing_hash"); // Should have changed from our fake hash

    // Now analyze again with force=false - should skip since hash matches
    let second_result = analyze_book(&db, book.id, false).await?;

    // Should analyze the book (books_analyzed=1) but detect no actual changes via hash
    // The function still returns success, but internally it skips re-analysis
    assert_eq!(second_result.books_analyzed, 1);
    assert_eq!(second_result.errors.len(), 0);

    // Hash should remain the same
    let after_second = BookRepository::get_by_id(&db, book.id).await?.unwrap();
    assert_eq!(after_second.file_hash, real_hash);
    assert!(after_second.analyzed);

    Ok(())
}

#[tokio::test]
async fn test_task_type_serialization_with_force() -> Result<()> {
    use codex::tasks::types::TaskType;
    use uuid::Uuid;

    // Test AnalyzeBook with force=true
    let task = TaskType::AnalyzeBook {
        book_id: Uuid::new_v4(),
        force: true,
    };

    let json = serde_json::to_string(&task)?;
    assert!(json.contains("\"force\":true"));

    let deserialized: TaskType = serde_json::from_str(&json)?;
    match deserialized {
        TaskType::AnalyzeBook { force, .. } => assert!(force),
        _ => panic!("Wrong task type"),
    }

    // Test AnalyzeBook with force=false (default)
    let task = TaskType::AnalyzeBook {
        book_id: Uuid::new_v4(),
        force: false,
    };

    let json = serde_json::to_string(&task)?;
    assert!(json.contains("\"force\":false"));

    // Test AnalyzeSeries with force=true
    let task = TaskType::AnalyzeSeries {
        series_id: Uuid::new_v4(),
        concurrency: 4,
        force: true,
    };

    let json = serde_json::to_string(&task)?;
    assert!(json.contains("\"force\":true"));

    let deserialized: TaskType = serde_json::from_str(&json)?;
    match deserialized {
        TaskType::AnalyzeSeries { force, .. } => assert!(force),
        _ => panic!("Wrong task type"),
    }

    Ok(())
}

#[tokio::test]
async fn test_task_type_default_force_false() -> Result<()> {
    use codex::tasks::types::TaskType;

    // Test that force defaults to false when not specified
    let json = r#"{"type":"analyze_book","book_id":"00000000-0000-0000-0000-000000000000"}"#;
    let task: TaskType = serde_json::from_str(json)?;

    match task {
        TaskType::AnalyzeBook { force, .. } => assert!(!force),
        _ => panic!("Wrong task type"),
    }

    Ok(())
}

#[tokio::test]
async fn test_deep_scan_queues_with_force() -> Result<()> {
    use codex::db::repositories::TaskRepository;
    use codex::tasks::types::TaskType;

    let (db, _temp_dir) = setup_test_db().await;
    let db_conn = &db;
    let temp_dir = TempDir::new()?;
    let cbz_path = create_test_cbz(&temp_dir, 1, false);

    // Create a test book
    let (book, _series) = create_analyzed_book(&db, cbz_path.to_str().unwrap()).await?;

    // Manually queue an analyze task with force=true (simulating deep scan behavior)
    let task_id = TaskRepository::enqueue(
        db_conn,
        TaskType::AnalyzeBook {
            book_id: book.id,
            force: true,
        },
        0,
        None,
    )
    .await?;

    // Verify the task was created with force=true in params
    let task = TaskRepository::get_by_id(db_conn, task_id).await?.unwrap();
    assert_eq!(task.task_type, "analyze_book");
    assert!(task.params.is_some());

    let params = task.params.unwrap();
    assert_eq!(params.get("force").and_then(|v| v.as_bool()), Some(true));

    Ok(())
}

#[tokio::test]
async fn test_normal_scan_queues_without_force() -> Result<()> {
    use codex::db::repositories::TaskRepository;
    use codex::tasks::types::TaskType;

    let (db, _temp_dir) = setup_test_db().await;
    let db_conn = &db;
    let temp_dir = TempDir::new()?;
    let cbz_path = create_test_cbz(&temp_dir, 1, false);

    // Create a test book
    let (book, _series) = create_analyzed_book(&db, cbz_path.to_str().unwrap()).await?;

    // Manually queue an analyze task with force=false (simulating normal scan behavior)
    let task_id = TaskRepository::enqueue(
        db_conn,
        TaskType::AnalyzeBook {
            book_id: book.id,
            force: false,
        },
        0,
        None,
    )
    .await?;

    // Verify the task was created with force=false in params
    let task = TaskRepository::get_by_id(db_conn, task_id).await?.unwrap();
    assert_eq!(task.task_type, "analyze_book");
    assert!(task.params.is_some());

    let params = task.params.unwrap();
    assert_eq!(params.get("force").and_then(|v| v.as_bool()), Some(false));

    Ok(())
}
