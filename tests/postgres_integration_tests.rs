// PostgreSQL integration tests
// These tests require a running PostgreSQL instance
// Run with: cargo test --test postgres_integration_tests -- --ignored

use codex::config::{DatabaseConfig, DatabaseType, PostgresConfig};
use codex::db::{Book, Database, Library, ScanningStrategy, Series};

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

    Database::new(&config).await.unwrap()
}

#[tokio::test]
#[ignore]
async fn test_postgres_library_insert_and_select() {
    let db = create_test_postgres_db().await;
    let pool = db.postgres_pool().unwrap();

    let library = Library::new(
        "Postgres Test Library".to_string(),
        "/postgres/test/path".to_string(),
        ScanningStrategy::KomgaCompatible,
    );

    // Insert
    sqlx::query(
        r#"
        INSERT INTO libraries (id, name, path, scanning_strategy, scanning_config, created_at, updated_at, last_scanned_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(library.id)
    .bind(&library.name)
    .bind(&library.path)
    .bind(&library.scanning_strategy)
    .bind(&library.scanning_config)
    .bind(library.created_at)
    .bind(library.updated_at)
    .bind(library.last_scanned_at)
    .execute(pool)
    .await
    .unwrap();

    // Select
    let row: (uuid::Uuid, String, String) =
        sqlx::query_as("SELECT id, name, path FROM libraries WHERE id = $1")
            .bind(library.id)
            .fetch_one(pool)
            .await
            .unwrap();

    assert_eq!(row.0, library.id);
    assert_eq!(row.1, "Postgres Test Library");
    assert_eq!(row.2, "/postgres/test/path");

    // Cleanup
    sqlx::query("DELETE FROM libraries WHERE id = $1")
        .bind(library.id)
        .execute(pool)
        .await
        .unwrap();

    db.close().await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_series_book_relationship() {
    let db = create_test_postgres_db().await;
    let pool = db.postgres_pool().unwrap();

    // Create library
    let library = Library::new(
        "Test Library".to_string(),
        "/test".to_string(),
        ScanningStrategy::KomgaCompatible,
    );

    sqlx::query(
        "INSERT INTO libraries (id, name, path, scanning_strategy, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6)"
    )
    .bind(library.id)
    .bind(&library.name)
    .bind(&library.path)
    .bind(&library.scanning_strategy)
    .bind(library.created_at)
    .bind(library.updated_at)
    .execute(pool)
    .await
    .unwrap();

    // Create series
    let series = Series::new(library.id, "Postgres Series".to_string());

    sqlx::query(
        "INSERT INTO series (id, library_id, name, normalized_name, book_count, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7)"
    )
    .bind(series.id)
    .bind(series.library_id)
    .bind(&series.name)
    .bind(&series.normalized_name)
    .bind(series.book_count)
    .bind(series.created_at)
    .bind(series.updated_at)
    .execute(pool)
    .await
    .unwrap();

    // Create book
    let book = Book::new(
        series.id,
        "/test/postgres_book.cbz".to_string(),
        "postgres_book.cbz".to_string(),
    );

    sqlx::query(
        r#"
        INSERT INTO books (id, series_id, file_path, file_name, file_size, file_hash, format, page_count, modified_at, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#
    )
    .bind(book.id)
    .bind(book.series_id)
    .bind(&book.file_path)
    .bind(&book.file_name)
    .bind(book.file_size)
    .bind(&book.file_hash)
    .bind(&book.format)
    .bind(book.page_count)
    .bind(book.modified_at)
    .bind(book.created_at)
    .bind(book.updated_at)
    .execute(pool)
    .await
    .unwrap();

    // Query book with series join
    let result: (uuid::Uuid, String, String) = sqlx::query_as(
        "SELECT books.id, books.file_name, series.name FROM books JOIN series ON books.series_id = series.id WHERE books.id = $1"
    )
    .bind(book.id)
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(result.0, book.id);
    assert_eq!(result.1, "postgres_book.cbz");
    assert_eq!(result.2, "Postgres Series");

    // Cleanup
    sqlx::query("DELETE FROM libraries WHERE id = $1")
        .bind(library.id)
        .execute(pool)
        .await
        .unwrap();

    db.close().await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_cascade_delete() {
    let db = create_test_postgres_db().await;
    let pool = db.postgres_pool().unwrap();

    // Create library and series
    let library = Library::new(
        "Test Library".to_string(),
        "/test".to_string(),
        ScanningStrategy::KomgaCompatible,
    );

    sqlx::query(
        "INSERT INTO libraries (id, name, path, scanning_strategy, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6)"
    )
    .bind(library.id)
    .bind(&library.name)
    .bind(&library.path)
    .bind(&library.scanning_strategy)
    .bind(library.created_at)
    .bind(library.updated_at)
    .execute(pool)
    .await
    .unwrap();

    let series = Series::new(library.id, "Test Series".to_string());

    sqlx::query(
        "INSERT INTO series (id, library_id, name, normalized_name, book_count, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7)"
    )
    .bind(series.id)
    .bind(series.library_id)
    .bind(&series.name)
    .bind(&series.normalized_name)
    .bind(series.book_count)
    .bind(series.created_at)
    .bind(series.updated_at)
    .execute(pool)
    .await
    .unwrap();

    // Delete library (should cascade to series)
    sqlx::query("DELETE FROM libraries WHERE id = $1")
        .bind(library.id)
        .execute(pool)
        .await
        .unwrap();

    // Verify series was also deleted
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM series WHERE id = $1")
        .bind(series.id)
        .fetch_one(pool)
        .await
        .unwrap();

    assert_eq!(count.0, 0);

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
    let pool = db2.postgres_pool().unwrap();
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM libraries")
        .fetch_one(pool)
        .await
        .unwrap();

    // Count doesn't matter, just that query works
    assert!(count.0 >= 0);

    db2.close().await;
}
