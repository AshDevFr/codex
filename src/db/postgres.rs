use anyhow::{Context, Result};
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use std::str::FromStr;
use tracing::info;

use crate::config::PostgresConfig;

/// PostgreSQL-specific database connection
#[derive(Clone, Debug)]
pub struct PostgresDatabase {
    pool: PgPool,
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

        // Check if migrations have been run by checking for the _sqlx_migrations table
        let needs_migration = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = '_sqlx_migrations'"
        )
        .fetch_one(&pool)
        .await
        .unwrap_or(0) == 0;

        let db = Self { pool };

        if needs_migration {
            info!("Database schema not found. Running migrations...");
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
    pub fn pool(&self) -> &PgPool {
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
