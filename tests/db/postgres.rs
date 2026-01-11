// PostgreSQL integration tests
// These tests require a running PostgreSQL instance
// Run with: cargo test --test postgres_integration_tests -- --ignored

use chrono::Utc;
use codex::config::{DatabaseConfig, DatabaseType, PostgresConfig};
use codex::db::entities::{books, libraries, series};
use codex::db::{
    repositories::{BookRepository, LibraryRepository, SeriesRepository},
    Database,
};
use codex::models::ScanningStrategy;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

// Static lock to ensure migrations only run once for PostgreSQL tests
// All tests share the same database, so we need to serialize migration execution
static POSTGRES_MIGRATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Helper to create a test database
async fn create_test_postgres_db() -> Database {
    let config = DatabaseConfig {
        db_type: DatabaseType::Postgres,
        postgres: Some(PostgresConfig {
            host: std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: std::env::var("POSTGRES_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(5432),
            username: std::env::var("POSTGRES_USER").unwrap_or_else(|_| "codex_test".to_string()),
            password: std::env::var("POSTGRES_PASSWORD")
                .unwrap_or_else(|_| "codex_test".to_string()),
            database_name: std::env::var("POSTGRES_DB")
                .unwrap_or_else(|_| "codex_test".to_string()),
        }),
        sqlite: None,
    };

    let db = Database::new(&config).await.unwrap();

    // Use a lock to ensure migrations only run once across all concurrent tests
    // This prevents race conditions when multiple tests try to run migrations simultaneously.
    // Migrator::up() is idempotent, but there can still be race conditions when creating
    // database types/extensions, so we serialize migration execution with a mutex.
    let lock = POSTGRES_MIGRATION_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().unwrap();

    // Run migrations - Migrator::up() is idempotent and will only apply pending migrations
    // The mutex ensures only one test runs migrations at a time, preventing conflicts
    db.run_migrations().await.unwrap();

    db
}

#[tokio::test]
#[ignore]
async fn test_postgres_library_insert_and_select() {
    let db = create_test_postgres_db().await;
    let conn = db.sea_orm_connection();

    // Create library using repository
    let library = LibraryRepository::create(
        conn,
        "Postgres Test Library",
        "/postgres/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    // Select using repository
    let retrieved = LibraryRepository::get_by_id(conn, library.id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(retrieved.id, library.id);
    assert_eq!(retrieved.name, "Postgres Test Library");
    assert_eq!(retrieved.path, "/postgres/test/path");

    // Cleanup
    LibraryRepository::delete(conn, library.id).await.unwrap();

    db.close().await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_series_book_relationship() {
    let db = create_test_postgres_db().await;
    let conn = db.sea_orm_connection();

    // Create library
    let library =
        LibraryRepository::create(conn, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    // Create series
    let series = SeriesRepository::create(conn, library.id, "Postgres Series", None)
        .await
        .unwrap();

    // Create book
    let now = Utc::now();
    let book_model = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        library_id: library.id,
        title: None,
        number: None,
        file_path: "/test/postgres_book.cbz".to_string(),
        file_name: "postgres_book.cbz".to_string(),
        file_size: 1024,
        file_hash: "test_hash".to_string(),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 10,
        deleted: false,
        analyzed: false,
        analysis_error: None,
        modified_at: now,
        created_at: now,
        updated_at: now,
        thumbnail_path: None,
        thumbnail_generated_at: None,
    };

    let book = BookRepository::create(conn, &book_model, None)
        .await
        .unwrap();

    // Query book with series join using SeaORM
    let book_with_series = books::Entity::find_by_id(book.id)
        .find_also_related(series::Entity)
        .one(conn)
        .await
        .unwrap()
        .unwrap();

    let (book_result, series_result) = book_with_series;
    assert_eq!(book_result.id, book.id);
    assert_eq!(book_result.file_name, "postgres_book.cbz");
    assert_eq!(series_result.unwrap().name, "Postgres Series");

    // Cleanup
    LibraryRepository::delete(conn, library.id).await.unwrap();

    db.close().await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_cascade_delete() {
    let db = create_test_postgres_db().await;
    let conn = db.sea_orm_connection();

    // Create library and series
    let library =
        LibraryRepository::create(conn, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Test Series", None)
        .await
        .unwrap();

    // Delete library (should cascade to series)
    LibraryRepository::delete(conn, library.id).await.unwrap();

    // Verify series was also deleted
    let count = series::Entity::find()
        .filter(series::Column::Id.eq(series.id))
        .count(conn)
        .await
        .unwrap();

    assert_eq!(count, 0);

    db.close().await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_health_check() {
    let db = create_test_postgres_db().await;

    // Health check should pass
    assert!(db.health_check().await.is_ok());

    db.close().await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_reconnect() {
    let db = create_test_postgres_db().await;

    // Close first connection
    db.close().await;

    // Reconnect to same database
    let db2 = create_test_postgres_db().await;

    // Should be able to query tables
    let count = libraries::Entity::find()
        .count(db2.sea_orm_connection())
        .await
        .unwrap();

    // Count doesn't matter, just that query works
    assert!(count >= 0);

    db2.close().await;
}

/// Test metrics repository with PostgreSQL
/// This test specifically verifies that SUM() aggregate functions work correctly with PostgreSQL,
/// which returns NUMERIC type instead of INTEGER like SQLite.
#[tokio::test]
#[ignore]
async fn test_postgres_metrics_repository() {
    use codex::db::repositories::MetricsRepository;

    let db = create_test_postgres_db().await;
    let conn = db.sea_orm_connection();

    // Create library and series
    let library = LibraryRepository::create(
        conn,
        "Metrics Test Library",
        "/test/metrics",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Metrics Test Series", None)
        .await
        .unwrap();

    // Create multiple books with different file sizes to test SUM aggregation
    let now = Utc::now();
    let book_sizes = vec![1_000_000_i64, 2_500_000, 500_000, 3_000_000]; // Total: 7,000,000

    for (idx, size) in book_sizes.iter().enumerate() {
        let book_model = books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
            title: Some(format!("Metrics Book {}", idx + 1)),
            number: None,
            file_path: format!("/test/metrics/book{}.cbz", idx + 1),
            file_name: format!("book{}.cbz", idx + 1),
            file_size: *size,
            file_hash: format!("metrics_hash_{}", idx + 1),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 10,
            deleted: false,
            analyzed: false,
            analysis_error: None,
            modified_at: now,
            created_at: now,
            updated_at: now,
            thumbnail_path: None,
            thumbnail_generated_at: None,
        };

        BookRepository::create(conn, &book_model, None)
            .await
            .unwrap();
    }

    // Test total_book_size - this is where the PostgreSQL NUMERIC type issue would occur
    let total_size = MetricsRepository::total_book_size(conn).await.unwrap();
    assert_eq!(
        total_size, 7_000_000,
        "total_book_size should correctly sum all book sizes"
    );

    // Test book count
    let book_count = MetricsRepository::count_books(conn).await.unwrap();
    assert!(
        book_count >= 4,
        "should have at least 4 books from this test"
    );

    // Test library_metrics - this also uses SUM aggregation
    let metrics = MetricsRepository::library_metrics(conn).await.unwrap();

    let our_library = metrics
        .iter()
        .find(|m| m.id == library.id)
        .expect("Should find our test library");
    assert_eq!(our_library.book_count, 4, "library should have 4 books");
    assert_eq!(
        our_library.total_size, 7_000_000,
        "library total_size should match sum of book sizes"
    );
    assert_eq!(our_library.series_count, 1, "library should have 1 series");

    // Cleanup
    LibraryRepository::delete(conn, library.id).await.unwrap();

    db.close().await;
}
