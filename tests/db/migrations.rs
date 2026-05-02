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
// Verifies the Phase 9 hard-removal migration drops the legacy total_book_count
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
// Phase 11 of metadata-count-split: adds `chapter` and `chapter_lock` to
// book_metadata. Verifies up/down behavior and default values for existing rows.

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
// Phase 12 of metadata-count-split: re-parse each book's filename and populate
// `book_metadata.volume` / `chapter` where currently NULL. Idempotent and
// strictly additive — never overwrites a populated value.

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
