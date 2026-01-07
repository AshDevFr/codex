use std::collections::HashMap;
use tempfile::TempDir;

pub use codex::config::{DatabaseConfig, DatabaseType, PostgresConfig, SQLiteConfig};
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

/// Create a test PostgreSQL database with migrations applied
/// Returns the DatabaseConnection and a cleanup guard
///
/// This connects to a PostgreSQL instance that must be running.
/// Set POSTGRES_TEST_URL environment variable to override the default URL.
/// Default: postgres://codex:codex@localhost:54321/codex_test
///
/// Use this for tests that need to verify PostgreSQL-specific behavior,
/// especially for queries with JOINs and aggregations which may behave
/// differently than SQLite.
pub async fn setup_test_db_postgres() -> Option<sea_orm::DatabaseConnection> {
    // Check if PostgreSQL testing is enabled via environment variable
    let postgres_url = std::env::var("POSTGRES_TEST_URL")
        .unwrap_or_else(|_| "postgres://codex:codex@localhost:54321/codex_test".to_string());

    // Try to connect - if it fails, skip the test gracefully
    let config = DatabaseConfig {
        db_type: DatabaseType::Postgres,
        sqlite: None,
        postgres: Some(PostgresConfig {
            host: extract_host(&postgres_url),
            port: extract_port(&postgres_url),
            username: extract_username(&postgres_url),
            password: extract_password(&postgres_url),
            database_name: extract_database(&postgres_url),
        }),
    };

    // Try to create database connection
    let db = match Database::new(&config).await {
        Ok(db) => db,
        Err(_) => {
            eprintln!("⚠️  PostgreSQL test database not available, skipping test");
            eprintln!("   To run PostgreSQL tests, start the test database:");
            eprintln!("   docker-compose up -d postgres-test");
            return None;
        }
    };

    // Run migrations
    if let Err(e) = db.run_migrations().await {
        eprintln!(
            "⚠️  Failed to run migrations on PostgreSQL test database: {}",
            e
        );
        return None;
    }

    // Clean up any existing test data
    let conn = db.sea_orm_connection();
    let _ = cleanup_test_data(conn).await;

    Some(conn.clone())
}

/// Helper to extract host from PostgreSQL URL
fn extract_host(url: &str) -> String {
    url.split('@')
        .nth(1)
        .and_then(|s| s.split(':').next())
        .unwrap_or("localhost")
        .to_string()
}

/// Helper to extract port from PostgreSQL URL
fn extract_port(url: &str) -> u16 {
    url.split('@')
        .nth(1)
        .and_then(|s| s.split(':').nth(1))
        .and_then(|s| s.split('/').next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(5432)
}

/// Helper to extract username from PostgreSQL URL
fn extract_username(url: &str) -> String {
    url.split("://")
        .nth(1)
        .and_then(|s| s.split(':').next())
        .unwrap_or("codex")
        .to_string()
}

/// Helper to extract password from PostgreSQL URL
fn extract_password(url: &str) -> String {
    url.split("://")
        .nth(1)
        .and_then(|s| s.split(':').nth(1))
        .and_then(|s| s.split('@').next())
        .unwrap_or("codex")
        .to_string()
}

/// Helper to extract database name from PostgreSQL URL
fn extract_database(url: &str) -> String {
    url.split('/').last().unwrap_or("codex_test").to_string()
}

/// Clean up test data from PostgreSQL database
async fn cleanup_test_data(db: &sea_orm::DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    use sea_orm::ConnectionTrait;

    // Delete in correct order to respect foreign keys
    db.execute_unprepared("TRUNCATE TABLE pages CASCADE")
        .await?;
    db.execute_unprepared("TRUNCATE TABLE books CASCADE")
        .await?;
    db.execute_unprepared("TRUNCATE TABLE book_metadata_records CASCADE")
        .await?;
    db.execute_unprepared("TRUNCATE TABLE series CASCADE")
        .await?;
    db.execute_unprepared("TRUNCATE TABLE libraries CASCADE")
        .await?;
    db.execute_unprepared("TRUNCATE TABLE read_progress CASCADE")
        .await?;
    db.execute_unprepared("TRUNCATE TABLE users CASCADE")
        .await?;
    db.execute_unprepared("TRUNCATE TABLE api_keys CASCADE")
        .await?;
    db.execute_unprepared("TRUNCATE TABLE email_verification_tokens CASCADE")
        .await?;
    db.execute_unprepared("TRUNCATE TABLE tasks CASCADE")
        .await?;

    Ok(())
}
