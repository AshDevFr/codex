use std::collections::HashMap;
use tempfile::TempDir;

pub use codex::config::{DatabaseConfig, DatabaseType, SQLiteConfig};
pub use codex::db::Database;

/// Create a test SQLite database with migrations applied
/// Returns the DatabaseConnection and TempDir (which must be kept alive)
pub async fn setup_test_db() -> (sea_orm::DatabaseConnection, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let mut pragmas = HashMap::new();
    pragmas.insert("foreign_keys".to_string(), "ON".to_string());

    let config = DatabaseConfig {
        db_type: DatabaseType::SQLite,
        postgres: None,
        sqlite: Some(SQLiteConfig {
            path: db_path.to_str().unwrap().to_string(),
            pragmas: Some(pragmas),
        }),
    };

    let db = Database::new(&config).await.unwrap();
    db.run_migrations().await.unwrap();
    let conn = db.sea_orm_connection().clone();
    (conn, temp_dir)
}

/// Create a test Database wrapper (for tests that need it)
/// Returns Database instance and TempDir (which must be kept alive)
pub async fn setup_test_db_wrapper() -> (Database, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let config = DatabaseConfig {
        db_type: DatabaseType::SQLite,
        postgres: None,
        sqlite: Some(SQLiteConfig {
            path: db_path.to_str().unwrap().to_string(),
            pragmas: None,
        }),
    };

    let db = Database::new(&config).await.unwrap();
    db.run_migrations().await.unwrap();
    (db, temp_dir)
}
