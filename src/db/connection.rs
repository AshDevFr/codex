use anyhow::{Context, Result};

use crate::config::{DatabaseConfig, DatabaseType};

use super::postgres::PostgresDatabase;
use super::sqlite::SqliteDatabase;

/// Unified database connection wrapper
#[derive(Clone, Debug)]
pub enum Database {
    Sqlite(SqliteDatabase),
    Postgres(PostgresDatabase),
}

impl Database {
    /// Create a new database connection from configuration
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        match config.db_type {
            DatabaseType::SQLite => {
                let sqlite_config = config
                    .sqlite
                    .as_ref()
                    .context("SQLite configuration is required when db_type is sqlite")?;

                let db = SqliteDatabase::new(sqlite_config).await?;
                Ok(Database::Sqlite(db))
            }
            DatabaseType::Postgres => {
                let postgres_config = config
                    .postgres
                    .as_ref()
                    .context("PostgreSQL configuration is required when db_type is postgres")?;

                let db = PostgresDatabase::new(postgres_config).await?;
                Ok(Database::Postgres(db))
            }
        }
    }

    /// Run database migrations
    pub async fn run_migrations(&self) -> Result<()> {
        match self {
            Database::Sqlite(db) => db.run_migrations().await,
            Database::Postgres(db) => db.run_migrations().await,
        }
    }

    /// Close the database connection
    pub async fn close(self) {
        match self {
            Database::Sqlite(db) => db.close().await,
            Database::Postgres(db) => db.close().await,
        }
    }

    /// Check if the database connection is healthy
    pub async fn health_check(&self) -> Result<()> {
        match self {
            Database::Sqlite(db) => db.health_check().await,
            Database::Postgres(db) => db.health_check().await,
        }
    }

    /// Get reference to SQLite pool (if using SQLite)
    pub fn sqlite_pool(&self) -> Option<&sqlx::SqlitePool> {
        match self {
            Database::Sqlite(db) => Some(db.pool()),
            Database::Postgres(_) => None,
        }
    }

    /// Get reference to PostgreSQL pool (if using PostgreSQL)
    pub fn postgres_pool(&self) -> Option<&sqlx::PgPool> {
        match self {
            Database::Sqlite(_) => None,
            Database::Postgres(db) => Some(db.pool()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DatabaseConfig, DatabaseType, SQLiteConfig};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_database_new_sqlite() {
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

        // Check that the file was created
        assert!(db_path.exists());

        // Health check
        assert!(db.health_check().await.is_ok());

        // Check pool access
        assert!(db.sqlite_pool().is_some());

        db.close().await;
    }

    #[tokio::test]
    #[ignore] // Requires PostgreSQL server
    async fn test_database_new_postgres() {
        let config = DatabaseConfig {
            db_type: DatabaseType::Postgres,
            postgres: Some(crate::config::PostgresConfig {
                host: std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string()),
                port: std::env::var("POSTGRES_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(5432),
                username: std::env::var("POSTGRES_USER").unwrap_or_else(|_| "codex_test".to_string()),
                password: std::env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "codex_test".to_string()),
                database_name: std::env::var("POSTGRES_DB").unwrap_or_else(|_| "codex_test".to_string()),
            }),
            sqlite: None,
        };

        let db = Database::new(&config).await.unwrap();

        // Health check
        assert!(db.health_check().await.is_ok());

        // Check pool access
        assert!(db.postgres_pool().is_some());
        assert!(db.sqlite_pool().is_none());

        db.close().await;
    }
}
