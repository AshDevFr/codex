// PostgreSQL integration tests
// These tests require a running PostgreSQL instance
// Run with: cargo test --test postgres_integration_tests -- --ignored

use chrono::Utc;
use codex::api::routes::v1::dto::series::{SeriesSortField, SeriesSortParam, SortDirection};
use codex::config::{DatabaseConfig, DatabaseType, PostgresConfig};
use codex::db::entities::{books, libraries, series};
use codex::db::{
    Database,
    repositories::{
        BookRepository, LibraryRepository, SeriesRepository, UserSeriesRatingRepository,
    },
};
use codex::models::ScanningStrategy;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, Statement};
use uuid::Uuid;

/// Helper to create a test database
///
/// Uses PostgreSQL advisory locks to synchronize migration execution across
/// processes (required for cargo-nextest which runs tests in separate processes).
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
            ..PostgresConfig::default()
        }),
        sqlite: None,
    };

    let db = Database::new(&config).await.unwrap();

    // Use PostgreSQL advisory lock to serialize migrations across processes
    // This is necessary because cargo-nextest runs tests in separate processes,
    // so an in-process mutex doesn't work. Advisory locks are database-level
    // and work across all connections.
    //
    // Lock ID 12345 is arbitrary but must be consistent across all tests.
    // pg_advisory_lock blocks until the lock is available.
    let conn = db.sea_orm_connection();
    conn.execute(Statement::from_string(
        sea_orm::DatabaseBackend::Postgres,
        "SELECT pg_advisory_lock(12345)".to_string(),
    ))
    .await
    .expect("Failed to acquire advisory lock");

    // Run migrations while holding the lock
    let migration_result = db.run_migrations().await;

    // Release the advisory lock (this happens automatically when connection closes,
    // but we release it explicitly to allow other tests to proceed sooner)
    conn.execute(Statement::from_string(
        sea_orm::DatabaseBackend::Postgres,
        "SELECT pg_advisory_unlock(12345)".to_string(),
    ))
    .await
    .expect("Failed to release advisory lock");

    // Now check migration result
    migration_result.expect("Failed to run database migrations");

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

    // Create book (title and number are now in book_metadata table)
    let now = Utc::now();
    let book_model = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        library_id: library.id,
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
        analysis_errors: None,
        modified_at: now,
        created_at: now,
        updated_at: now,
        thumbnail_path: None,
        thumbnail_generated_at: None,
        koreader_hash: None,
        epub_positions: None,
        epub_spine_items: None,
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
    // Series name is now in series_metadata table
    assert!(series_result.is_some());

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

    // Count doesn't matter, just that query works - assert it exists
    let _ = count;

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

    // Record initial state (database may have leftover data from previous test runs)
    let initial_total_size = MetricsRepository::total_book_size(conn).await.unwrap();
    let initial_book_count = MetricsRepository::count_books(conn).await.unwrap();

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
    let book_sizes = [1_000_000_i64, 2_500_000, 500_000, 3_000_000]; // Total: 7,000,000

    for (idx, size) in book_sizes.iter().enumerate() {
        // Title and number are now in book_metadata table
        let book_model = books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
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
            analysis_errors: None,
            modified_at: now,
            created_at: now,
            updated_at: now,
            thumbnail_path: None,
            thumbnail_generated_at: None,
            koreader_hash: None,
            epub_positions: None,
            epub_spine_items: None,
        };

        BookRepository::create(conn, &book_model, None)
            .await
            .unwrap();
    }

    // Test total_book_size - this is where the PostgreSQL NUMERIC type issue would occur
    // We test the increment rather than absolute value since database may have other data
    // Note: Since PostgreSQL tests share a database and run concurrently, other tests may
    // create/delete books during this test. We check that the increment is at least our
    // 7_000_000 bytes (the library-specific check below is the authoritative validation).
    let total_size = MetricsRepository::total_book_size(conn).await.unwrap();
    let size_increment = total_size - initial_total_size;
    assert!(
        size_increment >= 7_000_000,
        "total_book_size increment should be at least 7_000_000 (got {})",
        size_increment
    );

    // Test book count - similarly, allow for concurrent test activity
    let book_count = MetricsRepository::count_books(conn).await.unwrap();
    let count_increment = book_count - initial_book_count;
    assert!(
        count_increment >= 4,
        "should have added at least 4 books (got {})",
        count_increment
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

/// Test rating sort with PostgreSQL
/// Verifies that rating sort queries work correctly on PostgreSQL.
/// The user's own rating uses a direct column value (no AVG), while
/// community/external ratings use AVG which returns NUMERIC on PostgreSQL
/// (requiring CAST to DOUBLE PRECISION for Rust f64 deserialization).
#[tokio::test]
#[ignore]
async fn test_postgres_rating_sort() {
    use codex::db::repositories::UserRepository;
    use codex::utils::password;

    let db = create_test_postgres_db().await;
    let conn = db.sea_orm_connection();

    // Create library and series
    let library = LibraryRepository::create(
        conn,
        "Rating Sort Test Library",
        "/test/rating-sort",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series_a = SeriesRepository::create(conn, library.id, "Series A", None)
        .await
        .unwrap();
    let series_b = SeriesRepository::create(conn, library.id, "Series B", None)
        .await
        .unwrap();
    let _series_c = SeriesRepository::create(conn, library.id, "Series C", None)
        .await
        .unwrap();

    // Create a test user
    let password_hash = password::hash_password("test123").unwrap();
    let user_model = codex::db::entities::users::Model {
        id: Uuid::new_v4(),
        username: format!("rating_sort_test_{}", Uuid::new_v4()),
        email: format!("rating_sort_{}@test.com", Uuid::new_v4()),
        password_hash,
        role: "admin".to_string(),
        is_active: true,
        email_verified: false,
        permissions: serde_json::json!([]),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_login_at: None,
    };
    let user = UserRepository::create(conn, &user_model).await.unwrap();

    // Rate series A=30, B=80 (C is unrated)
    UserSeriesRatingRepository::create(conn, user.id, series_a.id, 30, None)
        .await
        .unwrap();
    UserSeriesRatingRepository::create(conn, user.id, series_b.id, 80, None)
        .await
        .unwrap();

    // Test rating sort via list_by_ids_sorted (this is the code path that fails on PG)
    let all_ids = vec![series_a.id, series_b.id, _series_c.id];
    let sort = SeriesSortParam::new(SeriesSortField::Rating, SortDirection::Desc);

    let (sorted_series, total) =
        SeriesRepository::list_by_ids_sorted(conn, &all_ids, &sort, Some(user.id), 0, 50)
            .await
            .expect("Rating sort should work on PostgreSQL");

    assert_eq!(total, 3);
    assert_eq!(sorted_series.len(), 3);
    // Series B (80) first, Series A (30) second, Series C (unrated) last
    assert_eq!(sorted_series[0].name, "Series B");
    assert_eq!(sorted_series[1].name, "Series A");
    assert_eq!(sorted_series[2].name, "Series C");

    // Cleanup
    LibraryRepository::delete(conn, library.id).await.unwrap();
    UserRepository::delete(conn, user.id).await.unwrap();

    db.close().await;
}
