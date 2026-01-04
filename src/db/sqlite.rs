use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sea_orm::{Database as SeaDatabase, DatabaseConnection};
use std::path::Path;
use std::str::FromStr;
use tokio::fs;

use crate::config::SQLiteConfig;

/// SQLite-specific database connection
#[derive(Clone, Debug)]
pub struct SqliteDatabase {
    pool: SqlitePool,
    sea_orm_conn: DatabaseConnection,
}

impl SqliteDatabase {
    /// Create a new SQLite database connection
    pub async fn new(config: &SQLiteConfig) -> Result<Self> {
        let path = Path::new(&config.path);

        // Check if database file exists
        let db_exists = path.exists();

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .await
                    .context("Failed to create database directory")?;
            }
        }

        // Build connection options
        let mut options = SqliteConnectOptions::from_str(&format!("sqlite://{}", config.path))?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_secs(5));

        // Apply custom pragmas if provided
        if let Some(pragmas) = &config.pragmas {
            for (key, value) in pragmas {
                options = options.pragma(key.to_string(), value.to_string());
            }
        }

        // Create connection pool
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .context("Failed to connect to SQLite database")?;

        // Create SeaORM connection from the same database URL
        let database_url = format!("sqlite://{}", config.path);
        let sea_orm_conn = SeaDatabase::connect(&database_url)
            .await
            .context("Failed to create SeaORM connection")?;

        Ok(Self { pool, sea_orm_conn })
    }


    /// Get a reference to the SeaORM database connection
    pub fn sea_orm_connection(&self) -> &DatabaseConnection {
        &self.sea_orm_conn
    }

    /// Close the database connection
    pub async fn close(self) {
        self.pool.close().await;
        // SeaORM connection will be closed automatically when dropped
    }

    /// Check if the database connection is healthy
    pub async fn health_check(&self) -> Result<()> {
        use sea_orm::ConnectionTrait;

        self.sea_orm_conn
            .execute(sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT 1".to_owned(),
            ))
            .await
            .context("Database health check failed")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_new_sqlite_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let config = SQLiteConfig {
            path: db_path.to_str().unwrap().to_string(),
            pragmas: None,
        };

        let db = SqliteDatabase::new(&config).await.unwrap();

        // Check that the file was created
        assert!(db_path.exists());

        // Health check
        assert!(db.health_check().await.is_ok());

        db.close().await;
    }

    #[tokio::test]
    async fn test_new_sqlite_with_custom_pragmas() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_pragmas.db");

        let mut pragmas = std::collections::HashMap::new();
        pragmas.insert("foreign_keys".to_string(), "ON".to_string());

        let config = SQLiteConfig {
            path: db_path.to_str().unwrap().to_string(),
            pragmas: Some(pragmas),
        };

        let db = SqliteDatabase::new(&config).await.unwrap();

        // Verify foreign keys are enabled using SeaORM
        use sea_orm::{ConnectionTrait, Statement};

        let result = db.sea_orm_conn
            .query_one(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "PRAGMA foreign_keys".to_owned(),
            ))
            .await
            .unwrap()
            .unwrap();

        let foreign_keys: i32 = result.try_get("", "foreign_keys").unwrap();
        assert_eq!(foreign_keys, 1); // 1 means ON

        db.close().await;
    }
}
