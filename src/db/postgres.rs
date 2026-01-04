use anyhow::{Context, Result};
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use sea_orm::{Database as SeaDatabase, DatabaseConnection};
use std::str::FromStr;

use crate::config::PostgresConfig;

/// PostgreSQL-specific database connection
#[derive(Clone, Debug)]
pub struct PostgresDatabase {
    pool: PgPool,
    sea_orm_conn: DatabaseConnection,
}

impl PostgresDatabase {
    /// Create a new PostgreSQL database connection
    pub async fn new(config: &PostgresConfig) -> Result<Self> {
        // Build connection string
        let connection_string = format!(
            "postgres://{}:{}@{}:{}/{}",
            config.username, config.password, config.host, config.port, config.database_name
        );

        // Build connection options
        let options = PgConnectOptions::from_str(&connection_string)?
            .application_name("codex");

        // Create connection pool
        let pool = PgPoolOptions::new()
            .max_connections(10) // PostgreSQL can handle more connections than SQLite
            .connect_with(options)
            .await
            .context("Failed to connect to PostgreSQL database")?;

        // Create SeaORM connection from the same connection string
        let sea_orm_conn = SeaDatabase::connect(&connection_string)
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
                sea_orm::DatabaseBackend::Postgres,
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

    // These tests require a running PostgreSQL instance
    // They are marked as ignored by default
    // Run with: cargo test postgres -- --ignored

    #[tokio::test]
    #[ignore]
    async fn test_new_postgres_connection() {
        let config = PostgresConfig {
            host: std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: std::env::var("POSTGRES_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(5432),
            username: std::env::var("POSTGRES_USER").unwrap_or_else(|_| "codex_test".to_string()),
            password: std::env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "codex_test".to_string()),
            database_name: std::env::var("POSTGRES_DB").unwrap_or_else(|_| "codex_test".to_string()),
        };

        let db = PostgresDatabase::new(&config).await;
        assert!(db.is_ok());

        if let Ok(db) = db {
            // Health check
            assert!(db.health_check().await.is_ok());
            db.close().await;
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_postgres_connection_failure() {
        let config = PostgresConfig {
            host: "localhost".to_string(),
            port: 5432,
            username: "invalid_user".to_string(),
            password: "invalid_password".to_string(),
            database_name: "nonexistent_db".to_string(),
        };

        let result = PostgresDatabase::new(&config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn test_postgres_health_check() {
        let config = PostgresConfig {
            host: std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: std::env::var("POSTGRES_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(5432),
            username: std::env::var("POSTGRES_USER").unwrap_or_else(|_| "codex_test".to_string()),
            password: std::env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "codex_test".to_string()),
            database_name: std::env::var("POSTGRES_DB").unwrap_or_else(|_| "codex_test".to_string()),
        };

        let db = PostgresDatabase::new(&config).await.unwrap();
        assert!(db.health_check().await.is_ok());
        db.close().await;
    }
}
