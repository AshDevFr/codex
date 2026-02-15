#[path = "../common/mod.rs"]
mod common;

// Database migration tests
// Tests for migration-related functionality

use codex::config::{DatabaseConfig, DatabaseType, SQLiteConfig};
use codex::db::Database;
use common::setup_test_db_wrapper;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use tempfile::TempDir;

#[tokio::test]
async fn test_migrations_complete_after_migration() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;

    // After migrations are run, they should be complete
    let complete = db
        .migrations_complete()
        .await
        .expect("Should be able to check migration status");

    assert!(complete, "Migrations should be complete after running them");
}

#[tokio::test]
async fn test_migrations_complete_on_fresh_database() {
    // Create a fresh database without running migrations
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let config = DatabaseConfig {
        db_type: DatabaseType::SQLite,
        postgres: None,
        sqlite: Some(SQLiteConfig {
            path: db_path.to_str().unwrap().to_string(),
            pragmas: None,
            ..SQLiteConfig::default()
        }),
    };

    let db = Database::new(&config).await.unwrap();

    // On a fresh database, migrations should not be complete
    let complete = db
        .migrations_complete()
        .await
        .expect("Should be able to check migration status");

    assert!(
        !complete,
        "Migrations should not be complete on a fresh database"
    );

    // Run migrations
    db.run_migrations().await.unwrap();

    // Now migrations should be complete
    let complete_after = db
        .migrations_complete()
        .await
        .expect("Should be able to check migration status");

    assert!(
        complete_after,
        "Migrations should be complete after running them"
    );
}

#[tokio::test]
async fn test_run_migrations_idempotent() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;

    // Run migrations again - should be idempotent
    let result = db.run_migrations().await;

    assert!(
        result.is_ok(),
        "Running migrations twice should be idempotent: {:?}",
        result
    );

    // Migrations should still be complete
    let complete = db
        .migrations_complete()
        .await
        .expect("Should be able to check migration status");

    assert!(
        complete,
        "Migrations should still be complete after running again"
    );
}

#[tokio::test]
async fn test_migrations_complete_after_partial_migration() {
    // This test verifies that migrations_complete correctly detects incomplete migrations
    // Note: This is harder to test without manually manipulating the migration table,
    // but we can at least verify the method works correctly for the normal case
    let (db, _temp_dir) = setup_test_db_wrapper().await;

    // Migrations should be complete
    let complete = db.migrations_complete().await.unwrap();
    assert!(complete);

    // Run migrations again (idempotent)
    db.run_migrations().await.unwrap();

    // Should still be complete
    let complete_after = db.migrations_complete().await.unwrap();
    assert!(complete_after);
}

// -- Migration 056 (consolidate_authors) tests --
// These tests verify the migration works correctly on SQLite, including:
// - Fresh run with author data to backfill
// - Recovery from partial failure (idempotency)

/// Helper: create a SQLite database and run all migrations EXCEPT the last one (056).
/// Returns the Database and TempDir (must keep alive).
async fn setup_db_before_migration_056() -> (Database, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let config = DatabaseConfig {
        db_type: DatabaseType::SQLite,
        postgres: None,
        sqlite: Some(SQLiteConfig {
            path: db_path.to_str().unwrap().to_string(),
            pragmas: None,
            ..SQLiteConfig::default()
        }),
    };

    let db = Database::new(&config).await.unwrap();
    let conn = db.sea_orm_connection();

    // Run all migrations except the last one (056 = consolidate_authors).
    // Migrator::up with Some(N) runs N migrations from the pending list.
    // There are 53 total; running 52 leaves 056 pending.
    Migrator::up(conn, Some(52)).await.unwrap();

    (db, temp_dir)
}

/// Helper: check if a column exists on a SQLite table.
async fn sqlite_has_column(conn: &sea_orm::DatabaseConnection, table: &str, column: &str) -> bool {
    let sql =
        format!("SELECT COUNT(*) as cnt FROM pragma_table_info('{table}') WHERE name = '{column}'");
    let row = conn
        .query_one(Statement::from_string(DatabaseBackend::Sqlite, sql))
        .await
        .unwrap()
        .unwrap();
    let count: i32 = row.try_get("", "cnt").unwrap();
    count > 0
}

#[tokio::test]
async fn test_migration_056_fresh_run_sqlite() {
    // Run all migrations up to 055, seed author data, then run 056 and verify backfill.
    let (db, _temp_dir) = setup_db_before_migration_056().await;
    let conn = db.sea_orm_connection();

    // Verify pre-conditions: old author columns exist, series_metadata lacks authors_json
    assert!(sqlite_has_column(conn, "book_metadata", "writer").await);
    assert!(sqlite_has_column(conn, "book_metadata", "writer_lock").await);
    assert!(!sqlite_has_column(conn, "series_metadata", "authors_json").await);

    // Seed a library, series, book, and book_metadata with author data.
    // Use only columns from the base table definitions (pre-migration-056 schema).
    conn.execute_unprepared(
        "INSERT INTO libraries (id, name, path, series_strategy, book_strategy, number_strategy, default_reading_direction, created_at, updated_at)
         VALUES (X'00000000000000000000000000000001', 'Test Lib', '/test', 'series_volume', 'filename', 'file_order', 'LEFT_TO_RIGHT', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();

    conn.execute_unprepared(
        "INSERT INTO series (id, library_id, path, name, normalized_name, created_at, updated_at)
         VALUES (X'00000000000000000000000000000002', X'00000000000000000000000000000001', '/test/series', 'Test Series', 'test series', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();

    conn.execute_unprepared(
        "INSERT INTO series_metadata (series_id, title, created_at, updated_at)
         VALUES (X'00000000000000000000000000000002', 'Test Series', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();

    conn.execute_unprepared(
        "INSERT INTO books (id, series_id, library_id, file_path, file_name, file_size, file_hash, partial_hash, format, page_count, deleted, analyzed, modified_at, created_at, updated_at)
         VALUES (X'00000000000000000000000000000003', X'00000000000000000000000000000002', X'00000000000000000000000000000001', '/test/book.cbz', 'book.cbz', 1024, 'hash1', '', 'cbz', 10, 0, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();

    conn.execute_unprepared(
        "INSERT INTO book_metadata (id, book_id, writer, penciller, writer_lock, created_at, updated_at)
         VALUES (X'00000000000000000000000000000004', X'00000000000000000000000000000003', 'John Doe, Jane Smith', 'Bob Artist', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();

    // Run migration 056
    Migrator::up(conn, None).await.unwrap();

    // Verify: series_metadata now has authors_json columns
    assert!(sqlite_has_column(conn, "series_metadata", "authors_json").await);
    assert!(sqlite_has_column(conn, "series_metadata", "authors_json_lock").await);

    // Verify: old individual columns are dropped
    assert!(!sqlite_has_column(conn, "book_metadata", "writer").await);
    assert!(!sqlite_has_column(conn, "book_metadata", "penciller").await);
    assert!(!sqlite_has_column(conn, "book_metadata", "writer_lock").await);

    // Verify: authors_json was backfilled
    let row = conn
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT authors_json, authors_json_lock FROM book_metadata WHERE id = X'00000000000000000000000000000004'".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    let authors_json: Option<String> = row.try_get("", "authors_json").unwrap();
    let authors_json_lock: bool = row.try_get("", "authors_json_lock").unwrap();

    let json = authors_json.expect("authors_json should be backfilled");
    // Should contain both writers and the penciller
    assert!(
        json.contains("John Doe"),
        "Should contain 'John Doe': {json}"
    );
    assert!(
        json.contains("Jane Smith"),
        "Should contain 'Jane Smith': {json}"
    );
    assert!(
        json.contains("Bob Artist"),
        "Should contain 'Bob Artist': {json}"
    );
    assert!(
        json.contains("writer"),
        "Should contain 'writer' role: {json}"
    );
    assert!(
        json.contains("penciller"),
        "Should contain 'penciller' role: {json}"
    );

    // writer_lock was true, so authors_json_lock should be consolidated to true
    assert!(
        authors_json_lock,
        "authors_json_lock should be true (writer_lock was true)"
    );

    db.close().await;
}

#[tokio::test]
async fn test_migration_056_partial_failure_recovery_sqlite() {
    // Simulate the prod failure: run migrations up to 055, manually add
    // authors_json to series_metadata (as if 056 partially ran), then
    // run 056 and verify it recovers gracefully.
    let (db, _temp_dir) = setup_db_before_migration_056().await;
    let conn = db.sea_orm_connection();

    // Simulate partial run: add the column that 056 would add in Step 1
    conn.execute_unprepared("ALTER TABLE series_metadata ADD COLUMN authors_json TEXT")
        .await
        .unwrap();

    assert!(sqlite_has_column(conn, "series_metadata", "authors_json").await);

    // Now run migration 056 — this should NOT fail with "duplicate column"
    Migrator::up(conn, None).await.unwrap();

    // Verify: both columns present on series_metadata
    assert!(sqlite_has_column(conn, "series_metadata", "authors_json").await);
    assert!(sqlite_has_column(conn, "series_metadata", "authors_json_lock").await);

    // Verify: old columns are dropped
    assert!(!sqlite_has_column(conn, "book_metadata", "writer").await);

    // Verify: running again is still idempotent
    let result = Migrator::up(conn, None).await;
    assert!(
        result.is_ok(),
        "Re-running after completion should be idempotent"
    );

    db.close().await;
}

#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn test_migration_056_fresh_run_postgres() {
    use codex::config::PostgresConfig;

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

    let db = match Database::new(&config).await {
        Ok(db) => db,
        Err(_) => {
            eprintln!("PostgreSQL test database not available, skipping test");
            return;
        }
    };
    let conn = db.sea_orm_connection();

    // Use advisory lock to serialize with other PG tests
    conn.execute(Statement::from_string(
        DatabaseBackend::Postgres,
        "SELECT pg_advisory_lock(12345)".to_string(),
    ))
    .await
    .unwrap();

    // Run all migrations
    Migrator::up(conn, None).await.unwrap();

    conn.execute(Statement::from_string(
        DatabaseBackend::Postgres,
        "SELECT pg_advisory_unlock(12345)".to_string(),
    ))
    .await
    .unwrap();

    // Verify schema: series_metadata has authors_json columns
    let row = conn
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT CAST(COUNT(*) AS INT) as cnt FROM information_schema.columns WHERE table_name = 'series_metadata' AND column_name = 'authors_json'".to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    let count: i32 = row.try_get("", "cnt").unwrap();
    assert_eq!(count, 1, "series_metadata should have authors_json column");

    // Verify: old individual columns are gone from book_metadata
    let row = conn
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT CAST(COUNT(*) AS INT) as cnt FROM information_schema.columns WHERE table_name = 'book_metadata' AND column_name = 'writer'".to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    let count: i32 = row.try_get("", "cnt").unwrap();
    assert_eq!(
        count, 0,
        "book_metadata should no longer have writer column"
    );

    db.close().await;
}
