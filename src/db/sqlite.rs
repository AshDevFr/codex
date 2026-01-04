use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;
use tokio::fs;
use tracing::info;

use crate::config::SQLiteConfig;

/// SQLite-specific database connection
#[derive(Clone, Debug)]
pub struct SqliteDatabase {
    pool: SqlitePool,
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

        let db = Self { pool };

        // If database was just created, run migrations
        if !db_exists {
            info!("Database file not found. Creating new database and running migrations...");
            db.run_migrations().await?;
        }

        Ok(db)
    }

    /// Run database migrations
    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .context("Failed to run database migrations")?;

        info!("Database migrations completed successfully");
        Ok(())
    }

    /// Get a reference to the underlying connection pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Close the database connection
    pub async fn close(self) {
        self.pool.close().await;
    }

    /// Check if the database connection is healthy
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
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

        // Verify foreign keys are enabled
        let row: (i32,) = sqlx::query_as("PRAGMA foreign_keys")
            .fetch_one(db.pool())
            .await
            .unwrap();

        assert_eq!(row.0, 1); // 1 means ON

        db.close().await;
    }
}
