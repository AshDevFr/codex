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
        ..DatabaseConfig::default()
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
        ..DatabaseConfig::default()
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
        ..DatabaseConfig::default()
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

// -- Migration 067 (split_book_count) tests --
// These tests verify that the migration adds the new volume + chapter columns
// and backfills total_volume_count from the legacy total_book_count, preserving
// the lock state. Chapter columns must remain NULL/false.

/// Helper: create a SQLite database and run all migrations EXCEPT the last one (067).
/// Returns the Database and TempDir (must keep alive).
async fn setup_db_before_migration_067() -> (Database, TempDir) {
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
        ..DatabaseConfig::default()
    };

    let db = Database::new(&config).await.unwrap();
    let conn = db.sea_orm_connection();

    // Run all migrations except 067 + 068 (the count-split + drop pair).
    // Total migrations after adding 068 is 65; running 63 leaves 067 and 068
    // both pending so each test below can apply them step-by-step via Some(1).
    Migrator::up(conn, Some(63)).await.unwrap();

    (db, temp_dir)
}

#[tokio::test]
async fn test_migration_067_backfill_sqlite() {
    let (db, _temp_dir) = setup_db_before_migration_067().await;
    let conn = db.sea_orm_connection();

    // Pre-conditions: legacy column exists, new columns do not.
    assert!(sqlite_has_column(conn, "series_metadata", "total_book_count").await);
    assert!(sqlite_has_column(conn, "series_metadata", "total_book_count_lock").await);
    assert!(!sqlite_has_column(conn, "series_metadata", "total_volume_count").await);
    assert!(!sqlite_has_column(conn, "series_metadata", "total_volume_count_lock").await);
    assert!(!sqlite_has_column(conn, "series_metadata", "total_chapter_count").await);
    assert!(!sqlite_has_column(conn, "series_metadata", "total_chapter_count_lock").await);

    // Seed three series + metadata rows covering the lock/value matrix.
    conn.execute_unprepared(
        "INSERT INTO libraries (id, name, path, series_strategy, book_strategy, number_strategy, default_reading_direction, created_at, updated_at)
         VALUES (X'00000000000000000000000000000001', 'Lib', '/lib', 'series_volume', 'filename', 'file_order', 'LEFT_TO_RIGHT', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();

    // Three series IDs.
    let s_value_and_lock = "X'00000000000000000000000000000010'";
    let s_value_only = "X'00000000000000000000000000000011'";
    let s_lock_only = "X'00000000000000000000000000000012'";

    for (idx, sid) in [s_value_and_lock, s_value_only, s_lock_only]
        .iter()
        .enumerate()
    {
        let sql = format!(
            "INSERT INTO series (id, library_id, path, name, normalized_name, created_at, updated_at)
             VALUES ({sid}, X'00000000000000000000000000000001', '/path/{idx}', 'Series {idx}', 'series {idx}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
        );
        conn.execute_unprepared(&sql).await.unwrap();
    }

    // Row 1: count=14, lock=true (volume-organized series with locked count).
    conn.execute_unprepared(&format!(
        "INSERT INTO series_metadata (series_id, title, total_book_count, total_book_count_lock, created_at, updated_at)
         VALUES ({s_value_and_lock}, 'Locked', 14, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    )).await.unwrap();

    // Row 2: count=42, lock=false (typical volume-organized series).
    conn.execute_unprepared(&format!(
        "INSERT INTO series_metadata (series_id, title, total_book_count, total_book_count_lock, created_at, updated_at)
         VALUES ({s_value_only}, 'Open', 42, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    )).await.unwrap();

    // Row 3: count=NULL, lock=true (chapter-organized series, user emptied + locked).
    conn.execute_unprepared(&format!(
        "INSERT INTO series_metadata (series_id, title, total_book_count, total_book_count_lock, created_at, updated_at)
         VALUES ({s_lock_only}, 'Empty Locked', NULL, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    )).await.unwrap();

    // Run migration 067 only (one step), so the legacy column is still present
    // and we can verify backfill semantics in isolation. Migration 068 (drop)
    // is exercised by `test_migration_068_drop_legacy_sqlite` below.
    Migrator::up(conn, Some(1)).await.unwrap();

    // Post-conditions: new columns present.
    assert!(sqlite_has_column(conn, "series_metadata", "total_volume_count").await);
    assert!(sqlite_has_column(conn, "series_metadata", "total_volume_count_lock").await);
    assert!(sqlite_has_column(conn, "series_metadata", "total_chapter_count").await);
    assert!(sqlite_has_column(conn, "series_metadata", "total_chapter_count_lock").await);
    // Legacy columns still present after 067 (dropped by 068).
    assert!(sqlite_has_column(conn, "series_metadata", "total_book_count").await);
    assert!(sqlite_has_column(conn, "series_metadata", "total_book_count_lock").await);

    // Helper closure to read a single row's split-count state.
    let read_state = |sid: &'static str| {
        let sql = format!(
            "SELECT total_volume_count, total_volume_count_lock, total_chapter_count, total_chapter_count_lock FROM series_metadata WHERE series_id = {sid}"
        );
        async move {
            let row = conn
                .query_one(Statement::from_string(DatabaseBackend::Sqlite, sql))
                .await
                .unwrap()
                .unwrap();
            let vol: Option<i32> = row.try_get("", "total_volume_count").unwrap();
            let vol_lock: bool = row.try_get("", "total_volume_count_lock").unwrap();
            let chap: Option<f32> = row.try_get("", "total_chapter_count").unwrap();
            let chap_lock: bool = row.try_get("", "total_chapter_count_lock").unwrap();
            (vol, vol_lock, chap, chap_lock)
        }
    };

    // Row 1: value + lock both copy across.
    let (vol, vol_lock, chap, chap_lock) = read_state(s_value_and_lock).await;
    assert_eq!(vol, Some(14));
    assert!(vol_lock);
    assert!(chap.is_none(), "chapter count must stay NULL on backfill");
    assert!(!chap_lock, "chapter lock must stay false on backfill");

    // Row 2: value copies, lock stays false.
    let (vol, vol_lock, chap, chap_lock) = read_state(s_value_only).await;
    assert_eq!(vol, Some(42));
    assert!(!vol_lock);
    assert!(chap.is_none());
    assert!(!chap_lock);

    // Row 3: NULL + locked → volume NULL + locked (the chapter-organized workaround state
    // lands cleanly on the new schema).
    let (vol, vol_lock, chap, chap_lock) = read_state(s_lock_only).await;
    assert!(vol.is_none());
    assert!(vol_lock);
    assert!(chap.is_none());
    assert!(!chap_lock);

    db.close().await;
}

// -- Migration 068 (drop_book_count) tests --
// Verifies the hard-removal migration drops the legacy total_book_count
// + total_book_count_lock columns while leaving the split-count columns intact.

#[tokio::test]
async fn test_migration_068_drop_legacy_sqlite() {
    let (db, _temp_dir) = setup_db_before_migration_067().await;
    let conn = db.sea_orm_connection();

    // Apply 067 first so the new columns exist alongside the legacy pair.
    Migrator::up(conn, Some(1)).await.unwrap();
    assert!(sqlite_has_column(conn, "series_metadata", "total_book_count").await);
    assert!(sqlite_has_column(conn, "series_metadata", "total_book_count_lock").await);
    assert!(sqlite_has_column(conn, "series_metadata", "total_volume_count").await);
    assert!(sqlite_has_column(conn, "series_metadata", "total_chapter_count").await);

    // Apply 068 (drop the legacy columns).
    Migrator::up(conn, None).await.unwrap();

    // Legacy columns are gone; split-count columns survive.
    assert!(!sqlite_has_column(conn, "series_metadata", "total_book_count").await);
    assert!(!sqlite_has_column(conn, "series_metadata", "total_book_count_lock").await);
    assert!(sqlite_has_column(conn, "series_metadata", "total_volume_count").await);
    assert!(sqlite_has_column(conn, "series_metadata", "total_volume_count_lock").await);
    assert!(sqlite_has_column(conn, "series_metadata", "total_chapter_count").await);
    assert!(sqlite_has_column(conn, "series_metadata", "total_chapter_count_lock").await);

    db.close().await;
}

// -- Migration 069 (add_book_chapter) tests --
// Adds `chapter` and `chapter_lock` to book_metadata. Verifies up/down
// behavior and default values for existing rows.

/// Helper: run all migrations through 068 so tests can apply 069 in isolation.
async fn setup_db_before_migration_069() -> (Database, TempDir) {
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
        ..DatabaseConfig::default()
    };

    let db = Database::new(&config).await.unwrap();
    let conn = db.sea_orm_connection();

    // Run all migrations through 068 (= 65 entries in the migration list, since
    // sequence numbers skip a few). Leaves 069 + 070 pending; the per-migration
    // tests apply them with `Some(1)` to step through assertions.
    Migrator::up(conn, Some(65)).await.unwrap();

    (db, temp_dir)
}

#[tokio::test]
async fn test_migration_069_adds_chapter_columns_sqlite() {
    let (db, _temp_dir) = setup_db_before_migration_069().await;
    let conn = db.sea_orm_connection();

    // Pre-conditions: chapter columns do not yet exist; volume + volume_lock do.
    assert!(sqlite_has_column(conn, "book_metadata", "volume").await);
    assert!(sqlite_has_column(conn, "book_metadata", "volume_lock").await);
    assert!(!sqlite_has_column(conn, "book_metadata", "chapter").await);
    assert!(!sqlite_has_column(conn, "book_metadata", "chapter_lock").await);

    // Seed a library, series, book, and book_metadata row using the pre-069 schema
    // so we can verify the new columns get default values applied to existing rows.
    conn.execute_unprepared(
        "INSERT INTO libraries (id, name, path, series_strategy, book_strategy, number_strategy, default_reading_direction, created_at, updated_at)
         VALUES (X'00000000000000000000000000000001', 'Lib', '/lib', 'series_volume', 'filename', 'file_order', 'LEFT_TO_RIGHT', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();

    conn.execute_unprepared(
        "INSERT INTO series (id, library_id, path, name, normalized_name, created_at, updated_at)
         VALUES (X'00000000000000000000000000000010', X'00000000000000000000000000000001', '/path', 'S', 's', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();

    conn.execute_unprepared(
        "INSERT INTO books (id, series_id, library_id, file_path, file_name, file_size, file_hash, partial_hash, format, page_count, deleted, analyzed, modified_at, created_at, updated_at)
         VALUES (X'00000000000000000000000000000020', X'00000000000000000000000000000010', X'00000000000000000000000000000001', '/path/v01.cbz', 'v01.cbz', 1024, 'h', '', 'cbz', 10, 0, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();

    conn.execute_unprepared(
        "INSERT INTO book_metadata (id, book_id, search_title, volume, volume_lock, created_at, updated_at)
         VALUES (X'00000000000000000000000000000030', X'00000000000000000000000000000020', 'v01', 1, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();

    // Apply migration 069.
    Migrator::up(conn, Some(1)).await.unwrap();

    // Post-conditions: new columns exist.
    assert!(sqlite_has_column(conn, "book_metadata", "chapter").await);
    assert!(sqlite_has_column(conn, "book_metadata", "chapter_lock").await);

    // Existing row gains NULL chapter and chapter_lock = false (the default).
    let row = conn
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT volume, chapter, chapter_lock FROM book_metadata WHERE id = X'00000000000000000000000000000030'"
                .to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    let volume: Option<i32> = row.try_get("", "volume").unwrap();
    let chapter: Option<f32> = row.try_get("", "chapter").unwrap();
    let chapter_lock: bool = row.try_get("", "chapter_lock").unwrap();
    assert_eq!(volume, Some(1));
    assert!(
        chapter.is_none(),
        "chapter must be NULL for pre-existing rows"
    );
    assert!(!chapter_lock, "chapter_lock must default to false");

    db.close().await;
}

#[tokio::test]
async fn test_migration_069_down_drops_chapter_columns_sqlite() {
    let (db, _temp_dir) = setup_db_before_migration_069().await;
    let conn = db.sea_orm_connection();

    // Apply 069 then immediately roll it back.
    Migrator::up(conn, Some(1)).await.unwrap();
    assert!(sqlite_has_column(conn, "book_metadata", "chapter").await);
    assert!(sqlite_has_column(conn, "book_metadata", "chapter_lock").await);

    Migrator::down(conn, Some(1)).await.unwrap();

    // Down drops the two new columns; volume + volume_lock still around.
    assert!(!sqlite_has_column(conn, "book_metadata", "chapter").await);
    assert!(!sqlite_has_column(conn, "book_metadata", "chapter_lock").await);
    assert!(sqlite_has_column(conn, "book_metadata", "volume").await);
    assert!(sqlite_has_column(conn, "book_metadata", "volume_lock").await);

    db.close().await;
}

// -- Migration 070 (backfill_book_volume_chapter) tests --
// Re-parse each book's filename and populate `book_metadata.volume` /
// `chapter` where currently NULL. Idempotent and strictly additive; never
// overwrites a populated value.

#[tokio::test]
async fn test_migration_070_backfills_from_filename_sqlite() {
    let (db, _temp_dir) = setup_db_before_migration_069().await;
    let conn = db.sea_orm_connection();

    // Apply 069 first (adds the columns) so we can populate test rows pre-070.
    Migrator::up(conn, Some(1)).await.unwrap();
    assert!(sqlite_has_column(conn, "book_metadata", "chapter").await);

    // Seed library + series + a handful of books covering each parse case.
    conn.execute_unprepared(
        "INSERT INTO libraries (id, name, path, series_strategy, book_strategy, number_strategy, default_reading_direction, created_at, updated_at)
         VALUES (X'00000000000000000000000000000001', 'Lib', '/lib', 'series_volume', 'filename', 'file_order', 'LEFT_TO_RIGHT', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();

    conn.execute_unprepared(
        "INSERT INTO series (id, library_id, path, name, normalized_name, created_at, updated_at)
         VALUES (X'00000000000000000000000000000010', X'00000000000000000000000000000001', '/path', 'S', 's', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();

    let cases: &[(&str, &str, &str)] = &[
        // (book_id_hex, file_name, comment)
        // Volume only.
        (
            "11111111111111111111111111111111",
            "Series v01.cbz",
            "vol-only",
        ),
        // Chapter only.
        (
            "22222222222222222222222222222222",
            "Series c042.cbz",
            "chap-only",
        ),
        // Both.
        (
            "33333333333333333333333333333333",
            "Series v15 - c126 (2023).cbz",
            "both",
        ),
        // Bare number — neither populated.
        ("44444444444444444444444444444444", "Naruto 042.cbz", "bare"),
    ];

    for (id, file_name, _comment) in cases {
        conn.execute_unprepared(&format!(
            "INSERT INTO books (id, series_id, library_id, file_path, file_name, file_size, file_hash, partial_hash, format, page_count, deleted, analyzed, modified_at, created_at, updated_at)
             VALUES (X'{id}', X'00000000000000000000000000000010', X'00000000000000000000000000000001', '/path/{file_name}', '{file_name}', 1024, 'h', '', 'cbz', 10, 0, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
        )).await.unwrap();

        let metadata_id = format!("aa{}", &id[2..]);
        conn.execute_unprepared(&format!(
            "INSERT INTO book_metadata (id, book_id, search_title, created_at, updated_at)
             VALUES (X'{metadata_id}', X'{id}', '{file_name}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
        )).await.unwrap();
    }

    // Pre-set volume = 99 for one book — the migration must NOT overwrite this.
    let preset_book_id = "55555555555555555555555555555555";
    let preset_meta_id = "bb555555555555555555555555555555";
    conn.execute_unprepared(&format!(
        "INSERT INTO books (id, series_id, library_id, file_path, file_name, file_size, file_hash, partial_hash, format, page_count, deleted, analyzed, modified_at, created_at, updated_at)
         VALUES (X'{preset_book_id}', X'00000000000000000000000000000010', X'00000000000000000000000000000001', '/path/Series v07.cbz', 'Series v07.cbz', 1024, 'h', '', 'cbz', 10, 0, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    )).await.unwrap();
    conn.execute_unprepared(&format!(
        "INSERT INTO book_metadata (id, book_id, search_title, volume, volume_lock, created_at, updated_at)
         VALUES (X'{preset_meta_id}', X'{preset_book_id}', 'Series v07', 99, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    )).await.unwrap();

    // Apply migration 070.
    Migrator::up(conn, Some(1)).await.unwrap();

    // Verify each parse case landed correctly.
    let expected: &[(&str, Option<i32>, Option<f32>)] = &[
        ("11111111111111111111111111111111", Some(1), None),
        ("22222222222222222222222222222222", None, Some(42.0)),
        ("33333333333333333333333333333333", Some(15), Some(126.0)),
        ("44444444444444444444444444444444", None, None),
    ];

    for (id, want_vol, want_chap) in expected {
        let row = conn
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!("SELECT volume, chapter FROM book_metadata WHERE book_id = X'{id}'"),
            ))
            .await
            .unwrap()
            .unwrap();
        let vol: Option<i32> = row.try_get("", "volume").unwrap();
        let chap: Option<f32> = row.try_get("", "chapter").unwrap();
        assert_eq!(vol, *want_vol, "volume mismatch for {id}");
        assert_eq!(chap, *want_chap, "chapter mismatch for {id}");
    }

    // Pre-set volume must be preserved (additive only — never overwrites).
    let row = conn
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("SELECT volume FROM book_metadata WHERE book_id = X'{preset_book_id}'"),
        ))
        .await
        .unwrap()
        .unwrap();
    let vol: Option<i32> = row.try_get("", "volume").unwrap();
    assert_eq!(
        vol,
        Some(99),
        "backfill must not overwrite a manually-set volume"
    );

    db.close().await;
}

#[tokio::test]
async fn test_migration_070_is_idempotent_sqlite() {
    let (db, _temp_dir) = setup_db_before_migration_069().await;
    let conn = db.sea_orm_connection();

    // Apply 069.
    Migrator::up(conn, Some(1)).await.unwrap();

    conn.execute_unprepared(
        "INSERT INTO libraries (id, name, path, series_strategy, book_strategy, number_strategy, default_reading_direction, created_at, updated_at)
         VALUES (X'00000000000000000000000000000001', 'Lib', '/lib', 'series_volume', 'filename', 'file_order', 'LEFT_TO_RIGHT', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();
    conn.execute_unprepared(
        "INSERT INTO series (id, library_id, path, name, normalized_name, created_at, updated_at)
         VALUES (X'00000000000000000000000000000010', X'00000000000000000000000000000001', '/path', 'S', 's', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();
    conn.execute_unprepared(
        "INSERT INTO books (id, series_id, library_id, file_path, file_name, file_size, file_hash, partial_hash, format, page_count, deleted, analyzed, modified_at, created_at, updated_at)
         VALUES (X'00000000000000000000000000000020', X'00000000000000000000000000000010', X'00000000000000000000000000000001', '/path/Series v05 - c100.cbz', 'Series v05 - c100.cbz', 1024, 'h', '', 'cbz', 10, 0, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();
    conn.execute_unprepared(
        "INSERT INTO book_metadata (id, book_id, search_title, created_at, updated_at)
         VALUES (X'00000000000000000000000000000030', X'00000000000000000000000000000020', 'sv05c100', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();

    // First pass.
    Migrator::up(conn, Some(1)).await.unwrap();
    // Second pass (down + up) — re-running must produce the same result.
    Migrator::down(conn, Some(1)).await.unwrap();
    Migrator::up(conn, Some(1)).await.unwrap();

    let row = conn
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT volume, chapter FROM book_metadata WHERE book_id = X'00000000000000000000000000000020'".to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    let vol: Option<i32> = row.try_get("", "volume").unwrap();
    let chap: Option<f32> = row.try_get("", "chapter").unwrap();
    assert_eq!(vol, Some(5));
    assert_eq!(chap, Some(100.0));

    db.close().await;
}

#[tokio::test]
async fn test_migration_089_renames_books_file_path_to_path_sqlite() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // All migrations have already run on the wrapper — verify the rename
    // landed: `path` exists, `file_path` does not, the unique index has the
    // new name, and the old one is gone.
    assert!(sqlite_has_column(conn, "books", "path").await);
    assert!(!sqlite_has_column(conn, "books", "file_path").await);

    let new_idx = conn
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_books_library_path_unique'".to_string(),
        ))
        .await
        .unwrap();
    assert!(
        new_idx.is_some(),
        "expected new unique index idx_books_library_path_unique to exist"
    );

    let old_idx = conn
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_books_library_file_path_unique'".to_string(),
        ))
        .await
        .unwrap();
    assert!(
        old_idx.is_none(),
        "old index idx_books_library_file_path_unique should have been dropped"
    );

    db.close().await;
}

#[tokio::test]
async fn test_migration_089_down_restores_file_path_sqlite() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Roll back every migration applied after (and including) 089 so its `down`
    // runs. Computed dynamically so adding later migrations doesn't break this.
    let migrations = Migrator::migrations();
    let idx_089 = migrations
        .iter()
        .position(|m| m.name().contains("rename_books_file_path_to_path"))
        .expect("migration 089 should exist");
    let steps = migrations.len() - idx_089;
    Migrator::down(conn, Some(steps as u32)).await.unwrap();

    assert!(sqlite_has_column(conn, "books", "file_path").await);
    assert!(!sqlite_has_column(conn, "books", "path").await);

    let old_idx = conn
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_books_library_file_path_unique'".to_string(),
        ))
        .await
        .unwrap();
    assert!(
        old_idx.is_some(),
        "down migration should restore the old unique index"
    );

    db.close().await;
}

// ============================================================================
// read_completions: table creation and backfill
// ============================================================================

/// Number of steps needed to roll back to just before the named migration.
fn steps_back_to(name: &str) -> u32 {
    let migrations = Migrator::migrations();
    let idx = migrations
        .iter()
        .position(|m| m.name().contains(name))
        .unwrap_or_else(|| panic!("migration {name} should exist"));
    (migrations.len() - idx) as u32
}

async fn sqlite_row_count(conn: &sea_orm::DatabaseConnection, sql: &str) -> i64 {
    let row = conn
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            sql.to_string(),
        ))
        .await
        .unwrap()
        .expect("count query should return a row");
    row.try_get::<i64>("", "cnt").unwrap()
}

/// The table and both of its indexes exist after migrating, and `down` removes
/// them cleanly.
#[tokio::test]
async fn test_read_completions_table_up_and_down_sqlite() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    for index in [
        "idx_read_completions_user_book",
        "idx_read_completions_user_date",
    ] {
        let found = conn
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!("SELECT name FROM sqlite_master WHERE type='index' AND name='{index}'"),
            ))
            .await
            .unwrap();
        assert!(found.is_some(), "{index} should exist after migrating up");
    }

    // Roll back through the create migration and confirm the table is gone.
    let steps = steps_back_to("create_read_completions");
    Migrator::down(conn, Some(steps)).await.unwrap();

    let table = conn
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type='table' AND name='read_completions'"
                .to_string(),
        ))
        .await
        .unwrap();
    assert!(table.is_none(), "down should drop read_completions");

    // And it comes back.
    Migrator::up(conn, None).await.unwrap();
    assert_eq!(
        sqlite_row_count(conn, "SELECT COUNT(*) as cnt FROM read_completions").await,
        0
    );

    db.close().await;
}

/// The backfill banks one completion per completed `read_progress` row, and
/// falls back to `updated_at` for the legacy rows that have `completed = true`
/// with a NULL `completed_at`.
#[tokio::test]
async fn test_read_completions_backfill_sqlite() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Roll back the two read_completions migrations so we can seed the
    // pre-migration state, then run them forward again.
    let steps = steps_back_to("create_read_completions");
    Migrator::down(conn, Some(steps)).await.unwrap();

    conn.execute_unprepared(
        "INSERT INTO libraries (id, name, path, series_strategy, book_strategy, number_strategy, default_reading_direction, created_at, updated_at)
         VALUES (X'00000000000000000000000000000001', 'Lib', '/test', 'series_volume', 'filename', 'file_order', 'LEFT_TO_RIGHT', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();
    conn.execute_unprepared(
        "INSERT INTO series (id, library_id, path, name, normalized_name, created_at, updated_at)
         VALUES (X'00000000000000000000000000000002', X'00000000000000000000000000000001', '/test/s', 'S', 's', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();
    conn.execute_unprepared(
        "INSERT INTO users (id, username, email, password_hash, role, is_active, email_verified, permissions, created_at, updated_at)
         VALUES (X'00000000000000000000000000000009', 'reader', 'r@example.com', 'x', 'admin', 1, 0, '[]', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();

    for (n, hash) in [(3u8, "h1"), (4, "h2"), (5, "h3")] {
        conn.execute_unprepared(&format!(
            "INSERT INTO books (id, series_id, library_id, path, file_name, file_size, file_hash, partial_hash, format, page_count, deleted, analyzed, modified_at, created_at, updated_at)
             VALUES (X'0000000000000000000000000000000{n}', X'00000000000000000000000000000002', X'00000000000000000000000000000001', '/test/b{n}.cbz', 'b{n}.cbz', 1024, '{hash}', '', 'cbz', 10, 0, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
        )).await.unwrap();
    }

    // Book 3: completed, with a real completed_at.
    conn.execute_unprepared(
        "INSERT INTO read_progress (id, user_id, book_id, current_page, completed, started_at, updated_at, completed_at)
         VALUES (X'00000000000000000000000000000013', X'00000000000000000000000000000009', X'00000000000000000000000000000003', 10, 1, '2026-02-01T00:00:00Z', '2026-02-05T00:00:00Z', '2026-02-04T00:00:00Z')"
    ).await.unwrap();
    // Book 4: legacy completed row with NULL completed_at.
    conn.execute_unprepared(
        "INSERT INTO read_progress (id, user_id, book_id, current_page, completed, started_at, updated_at, completed_at)
         VALUES (X'00000000000000000000000000000014', X'00000000000000000000000000000009', X'00000000000000000000000000000004', 10, 1, '2026-03-01T00:00:00Z', '2026-03-09T00:00:00Z', NULL)"
    ).await.unwrap();
    // Book 5: in progress, must not be backfilled.
    conn.execute_unprepared(
        "INSERT INTO read_progress (id, user_id, book_id, current_page, completed, started_at, updated_at, completed_at)
         VALUES (X'00000000000000000000000000000015', X'00000000000000000000000000000009', X'00000000000000000000000000000005', 4, 0, '2026-04-01T00:00:00Z', '2026-04-02T00:00:00Z', NULL)"
    ).await.unwrap();

    Migrator::up(conn, None).await.unwrap();

    assert_eq!(
        sqlite_row_count(conn, "SELECT COUNT(*) as cnt FROM read_completions").await,
        2,
        "only the two completed rows should be backfilled"
    );

    // The in-progress book has no completion.
    assert_eq!(
        sqlite_row_count(
            conn,
            "SELECT COUNT(*) as cnt FROM read_completions WHERE book_id = X'00000000000000000000000000000005'"
        )
        .await,
        0
    );

    // The row with a real completed_at keeps it.
    assert_eq!(
        sqlite_row_count(
            conn,
            "SELECT COUNT(*) as cnt FROM read_completions \
             WHERE book_id = X'00000000000000000000000000000003' \
               AND completed_at LIKE '2026-02-04%' AND started_at LIKE '2026-02-01%'"
        )
        .await,
        1
    );

    // The legacy NULL row falls back to updated_at, not to its started_at and
    // not to NULL (the column is NOT NULL, so a bad fallback would have failed
    // the insert outright).
    assert_eq!(
        sqlite_row_count(
            conn,
            "SELECT COUNT(*) as cnt FROM read_completions \
             WHERE book_id = X'00000000000000000000000000000004' \
               AND completed_at LIKE '2026-03-09%'"
        )
        .await,
        1,
        "a NULL completed_at should fall back to updated_at"
    );

    db.close().await;
}

/// Re-running the backfill does not duplicate history. This matters because the
/// write hook starts banking completions live, so the backfill's NOT EXISTS
/// filter is the only thing keeping a re-run from doubling every entry.
#[tokio::test]
async fn test_read_completions_backfill_is_idempotent_sqlite() {
    use migration::{MigrationTrait, SchemaManager};

    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    let steps = steps_back_to("create_read_completions");
    Migrator::down(conn, Some(steps)).await.unwrap();

    conn.execute_unprepared(
        "INSERT INTO libraries (id, name, path, series_strategy, book_strategy, number_strategy, default_reading_direction, created_at, updated_at)
         VALUES (X'00000000000000000000000000000001', 'Lib', '/test', 'series_volume', 'filename', 'file_order', 'LEFT_TO_RIGHT', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();
    conn.execute_unprepared(
        "INSERT INTO series (id, library_id, path, name, normalized_name, created_at, updated_at)
         VALUES (X'00000000000000000000000000000002', X'00000000000000000000000000000001', '/test/s', 'S', 's', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();
    conn.execute_unprepared(
        "INSERT INTO users (id, username, email, password_hash, role, is_active, email_verified, permissions, created_at, updated_at)
         VALUES (X'00000000000000000000000000000009', 'reader', 'r@example.com', 'x', 'admin', 1, 0, '[]', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();
    conn.execute_unprepared(
        "INSERT INTO books (id, series_id, library_id, path, file_name, file_size, file_hash, partial_hash, format, page_count, deleted, analyzed, modified_at, created_at, updated_at)
         VALUES (X'00000000000000000000000000000003', X'00000000000000000000000000000002', X'00000000000000000000000000000001', '/test/b.cbz', 'b.cbz', 1024, 'h1', '', 'cbz', 10, 0, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
    ).await.unwrap();
    conn.execute_unprepared(
        "INSERT INTO read_progress (id, user_id, book_id, current_page, completed, started_at, updated_at, completed_at)
         VALUES (X'00000000000000000000000000000013', X'00000000000000000000000000000009', X'00000000000000000000000000000003', 10, 1, '2026-02-01T00:00:00Z', '2026-02-05T00:00:00Z', '2026-02-04T00:00:00Z')"
    ).await.unwrap();

    Migrator::up(conn, None).await.unwrap();
    assert_eq!(
        sqlite_row_count(conn, "SELECT COUNT(*) as cnt FROM read_completions").await,
        1
    );

    // Invoke the backfill's `up` a second time directly; the migration table
    // would otherwise refuse to re-run it.
    let manager = SchemaManager::new(conn);
    migration::m20260729_000104_backfill_read_completions::Migration
        .up(&manager)
        .await
        .unwrap();

    assert_eq!(
        sqlite_row_count(conn, "SELECT COUNT(*) as cnt FROM read_completions").await,
        1,
        "re-running the backfill must not duplicate a banked completion"
    );

    db.close().await;
}

/// Exercise the backfill's INSERT path on a given connection.
///
/// Seeds through SeaORM rather than raw SQL so the same body runs on SQLite and
/// PostgreSQL: the hex-literal seeding used above is SQLite-only, and a
/// zero-row backfill short-circuits before building its INSERT, leaving the
/// statement that actually differs between dialects untested.
async fn assert_backfill_inserts(conn: &sea_orm::DatabaseConnection) {
    use chrono::{TimeZone, Utc};
    use codex::db::entities::{books, libraries, read_progress, users};
    use codex::db::repositories::{LibraryRepository, ReadCompletionRepository, SeriesRepository};
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    use uuid::Uuid;

    let user_id = Uuid::new_v4();
    users::ActiveModel {
        id: Set(user_id),
        username: Set(format!("reader-{user_id}")),
        email: Set(format!("{user_id}@example.com")),
        password_hash: Set("x".to_string()),
        role: Set("admin".to_string()),
        is_active: Set(true),
        email_verified: Set(false),
        permissions: Set(serde_json::json!([])),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        last_login_at: Set(None),
    }
    .insert(conn)
    .await
    .unwrap();

    // Via the repositories: they own the column defaults these tables require.
    let library = LibraryRepository::create(
        conn,
        &format!("Lib {}", Uuid::new_v4()),
        &format!("/test/{}", Uuid::new_v4()),
        codex::models::ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let library_id = library.id;
    let series_id = SeriesRepository::create(conn, library_id, "S", None)
        .await
        .unwrap()
        .id;

    let mut book_ids = Vec::new();
    for _ in 0..3 {
        let id = Uuid::new_v4();
        books::ActiveModel {
            id: Set(id),
            series_id: Set(series_id),
            library_id: Set(library_id),
            path: Set(format!("/test/{id}.cbz")),
            file_name: Set("b.cbz".to_string()),
            file_size: Set(1024),
            file_hash: Set(format!("hash-{id}")),
            partial_hash: Set(String::new()),
            format: Set("cbz".to_string()),
            page_count: Set(10),
            deleted: Set(false),
            analyzed: Set(false),
            modified_at: Set(Utc::now()),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            ..Default::default()
        }
        .insert(conn)
        .await
        .unwrap();
        book_ids.push(id);
    }

    let started = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();
    let finished = Utc.with_ymd_and_hms(2026, 2, 4, 0, 0, 0).unwrap();
    let touched = Utc.with_ymd_and_hms(2026, 3, 9, 0, 0, 0).unwrap();

    // Completed with a real completed_at.
    read_progress::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user_id),
        book_id: Set(book_ids[0]),
        current_page: Set(10),
        progress_percentage: Set(None),
        completed: Set(true),
        started_at: Set(started),
        updated_at: Set(finished),
        completed_at: Set(Some(finished)),
        r2_progression: Set(None),
    }
    .insert(conn)
    .await
    .unwrap();

    // Legacy completed row with a NULL completed_at.
    read_progress::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user_id),
        book_id: Set(book_ids[1]),
        current_page: Set(10),
        progress_percentage: Set(None),
        completed: Set(true),
        started_at: Set(started),
        updated_at: Set(touched),
        completed_at: Set(None),
        r2_progression: Set(None),
    }
    .insert(conn)
    .await
    .unwrap();

    // In progress: must not be backfilled.
    read_progress::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user_id),
        book_id: Set(book_ids[2]),
        current_page: Set(4),
        progress_percentage: Set(None),
        completed: Set(false),
        started_at: Set(started),
        updated_at: Set(touched),
        completed_at: Set(None),
        r2_progression: Set(None),
    }
    .insert(conn)
    .await
    .unwrap();

    // Drop and re-create the table so the backfill runs with rows present.
    let steps = steps_back_to("create_read_completions");
    Migrator::down(conn, Some(steps)).await.unwrap();
    Migrator::up(conn, None).await.unwrap();

    assert_eq!(
        ReadCompletionRepository::count_for_book(conn, user_id, book_ids[0])
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        ReadCompletionRepository::count_for_book(conn, user_id, book_ids[1])
            .await
            .unwrap(),
        1,
        "a legacy NULL completed_at must still be backfilled"
    );
    assert_eq!(
        ReadCompletionRepository::count_for_book(conn, user_id, book_ids[2])
            .await
            .unwrap(),
        0,
        "an in-progress book must not be backfilled"
    );

    let entries = ReadCompletionRepository::list_for_book(conn, user_id, book_ids[1])
        .await
        .unwrap();
    assert_eq!(
        entries[0].completed_at, touched,
        "the NULL completed_at should fall back to updated_at"
    );

    // Clean up so a shared Postgres database doesn't accumulate fixtures.
    users::Entity::delete_by_id(user_id)
        .exec(conn)
        .await
        .unwrap();
    libraries::Entity::delete_by_id(library_id)
        .exec(conn)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_read_completions_backfill_inserts_sqlite() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    assert_backfill_inserts(db.sea_orm_connection()).await;
    db.close().await;
}

/// The same on PostgreSQL, where the multi-row INSERT renders `$1..$N`
/// placeholders rather than `?`. Skipped when no test database is running.
#[tokio::test]
#[ignore]
async fn test_read_completions_backfill_inserts_postgres() {
    let Some(conn) = common::setup_test_db_postgres().await else {
        return;
    };
    assert_backfill_inserts(&conn).await;
}

/// The library reading-direction normalization.
///
/// `LibraryRepository::create` defaulted this column to the Komga-style
/// `LEFT_TO_RIGHT` while the web form wrote `ltr`, so the same column held two
/// vocabularies and the reader could parse only one of them. The migration
/// converges the Komga-style rows onto the lowercase form Codex stores.
#[tokio::test]
async fn test_library_reading_direction_normalization_sqlite() {
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
        ..DatabaseConfig::default()
    };

    let db = Database::new(&config).await.unwrap();
    let conn = db.sea_orm_connection();

    // Run everything up to, but not including, the normalization. Locating it
    // by name keeps this correct as later migrations are added.
    let migrations = Migrator::migrations();
    let index = migrations
        .iter()
        .position(|m| m.name() == "m20260826_000111_normalize_library_reading_direction")
        .expect("normalization migration should be registered");
    Migrator::up(conn, Some(index as u32)).await.unwrap();

    // Seed one row per vocabulary, plus a value the migration must not touch.
    let seeds = [
        (1u8, "LEFT_TO_RIGHT"),
        (2, "RIGHT_TO_LEFT"),
        (3, "VERTICAL"),
        (4, "TOP_TO_BOTTOM"),
        (5, "WEBTOON"),
        (6, "rtl"),
        (7, "sideways"),
    ];
    for (n, direction) in seeds {
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO libraries (id, name, path, series_strategy, book_strategy, number_strategy, default_reading_direction, created_at, updated_at) \
             VALUES (?, ?, ?, 'series_volume', 'filename', 'file_order', ?, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [
                vec![n; 16].into(),
                format!("Lib {}", n).into(),
                format!("/lib{}", n).into(),
                direction.into(),
            ],
        ))
        .await
        .unwrap();
    }

    Migrator::up(conn, Some(1)).await.unwrap();

    let expected = [
        (1u8, "ltr"),
        (2, "rtl"),
        (3, "ttb"),
        (4, "ttb"),
        (5, "webtoon"),
        // Already canonical, left alone.
        (6, "rtl"),
        // Unrecognized values are not guessed at. Resolution treats an
        // unparseable value as absent and falls through to the next layer.
        (7, "sideways"),
    ];
    for (n, want) in expected {
        let row = conn
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT default_reading_direction AS d FROM libraries WHERE id = ?",
                [vec![n; 16].into()],
            ))
            .await
            .unwrap()
            .unwrap();
        let got: String = row.try_get("", "d").unwrap();
        assert_eq!(got, want, "library {} should normalize to {}", n, want);
    }

    db.close().await;
}
