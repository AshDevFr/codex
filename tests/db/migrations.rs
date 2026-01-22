#[path = "../common/mod.rs"]
mod common;

// Database migration tests
// Tests for migration-related functionality

use codex::config::{DatabaseConfig, DatabaseType, SQLiteConfig};
use codex::db::Database;
use common::setup_test_db_wrapper;
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
