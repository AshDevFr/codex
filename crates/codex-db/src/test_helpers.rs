//! Test database helpers.
//!
//! Gated behind the `test-utils` feature so downstream crates can opt in via
//! a dev-dependency feature flag (`codex-db = { ..., features = ["test-utils"] }`)
//! without dragging the helpers into release builds.

use crate::Database;
use codex_config::{DatabaseConfig, DatabaseType, SQLiteConfig};
use tempfile::TempDir;

/// Helper to create a test SQLite database with migrations applied
///
/// This function creates a temporary SQLite database, runs all migrations,
/// and returns both the database connection and the temp directory (to keep it alive).
pub async fn create_test_db() -> (Database, TempDir) {
    use std::collections::HashMap;

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Enable foreign keys for SQLite (required for foreign key constraints)
    let mut pragmas = HashMap::new();
    pragmas.insert("foreign_keys".to_string(), "ON".to_string());

    let config = DatabaseConfig {
        db_type: DatabaseType::SQLite,
        postgres: None,
        sqlite: Some(SQLiteConfig {
            path: db_path.to_str().unwrap().to_string(),
            pragmas: Some(pragmas),
            ..SQLiteConfig::default()
        }),
    };

    let db = Database::new(&config).await.unwrap();
    db.run_migrations().await.unwrap();
    (db, temp_dir)
}

/// Simplified helper that returns the `DatabaseConnection` and keeps the temp dir alive.
pub async fn setup_test_db() -> sea_orm::DatabaseConnection {
    let (db, temp_dir) = create_test_db().await;
    let conn = db.sea_orm_connection().clone();
    // Leak the temp_dir so it stays alive for the duration of the test
    // This is acceptable in test code
    std::mem::forget(temp_dir);
    conn
}
