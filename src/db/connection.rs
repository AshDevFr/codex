use anyhow::{Context, Result};
use sea_orm::{ConnectionTrait, Database as SeaDatabase, DatabaseConnection};
use std::path::Path;
use tokio::fs;
use tracing::info;
use uuid::Uuid;

use super::ScanningStrategy;
use crate::config::{DatabaseConfig, DatabaseType};
use crate::db::entities;

use super::repositories::{
    BookMetadataRepository, BookRepository, LibraryRepository, PageRepository, SeriesRepository,
};
use migration::{Migrator, MigratorTrait};

/// Unified database connection wrapper
#[derive(Clone, Debug)]
pub struct Database {
    conn: DatabaseConnection,
}

impl Database {
    /// Validate pragma key to prevent SQL injection
    /// Only allows alphanumeric characters and underscores
    fn validate_pragma_key(key: &str) -> Result<()> {
        // Whitelist of commonly used SQLite pragmas
        const ALLOWED_PRAGMAS: &[&str] = &[
            "foreign_keys",
            "journal_mode",
            "synchronous",
            "cache_size",
            "temp_store",
            "locking_mode",
            "auto_vacuum",
            "busy_timeout",
            "wal_autocheckpoint",
            "query_only",
        ];

        if !ALLOWED_PRAGMAS.contains(&key) {
            anyhow::bail!(
                "Invalid pragma key '{}'. Allowed pragmas: {}",
                key,
                ALLOWED_PRAGMAS.join(", ")
            );
        }

        Ok(())
    }

    /// Create a new database connection from configuration
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        let conn = match config.db_type {
            DatabaseType::SQLite => {
                let sqlite_config = config
                    .sqlite
                    .as_ref()
                    .context("SQLite configuration is required when db_type is sqlite")?;

                // Create parent directories if they don't exist
                let path = Path::new(&sqlite_config.path);
                if let Some(parent) = path.parent() {
                    if !parent.exists() {
                        fs::create_dir_all(parent)
                            .await
                            .context("Failed to create database directory")?;
                    }
                }

                // Build SeaORM connection URL
                let database_url = format!("sqlite://{}?mode=rwc", sqlite_config.path);

                // Connect to database
                let conn = SeaDatabase::connect(&database_url)
                    .await
                    .context("Failed to create SQLite connection")?;

                // CRITICAL: Always enable foreign keys for data integrity
                // This must be set for every connection
                conn.execute(sea_orm::Statement::from_string(
                    sea_orm::DatabaseBackend::Sqlite,
                    "PRAGMA foreign_keys = ON".to_string(),
                ))
                .await
                .context("Failed to enable foreign keys")?;

                // Apply custom pragmas if provided
                if let Some(pragmas) = &sqlite_config.pragmas {
                    for (key, value) in pragmas {
                        // Skip foreign_keys as it's always enabled above
                        if key.eq_ignore_ascii_case("foreign_keys") {
                            continue;
                        }

                        // Validate pragma key to prevent SQL injection
                        Self::validate_pragma_key(key)?;

                        let pragma_sql = format!("PRAGMA {} = {}", key, value);
                        conn.execute(sea_orm::Statement::from_string(
                            sea_orm::DatabaseBackend::Sqlite,
                            pragma_sql,
                        ))
                        .await
                        .context(format!("Failed to set PRAGMA {} = {}", key, value))?;
                    }
                }

                conn
            }
            DatabaseType::Postgres => {
                let postgres_config = config
                    .postgres
                    .as_ref()
                    .context("PostgreSQL configuration is required when db_type is postgres")?;

                // Build connection string
                let connection_string = format!(
                    "postgres://{}:{}@{}:{}/{}",
                    postgres_config.username,
                    postgres_config.password,
                    postgres_config.host,
                    postgres_config.port,
                    postgres_config.database_name
                );

                SeaDatabase::connect(&connection_string)
                    .await
                    .context("Failed to create PostgreSQL connection")?
            }
        };

        Ok(Self { conn })
    }

    /// Run database migrations
    ///
    /// This will apply all pending migrations. Migrator::up() is idempotent,
    /// so it's safe to call multiple times - it will only run pending migrations.
    pub async fn run_migrations(&self) -> Result<()> {
        // Check migration status for logging
        let status = Migrator::status(&self.conn)
            .await
            .context("Failed to check migration status")?;

        // Log migration status
        info!("Migration status: {:?}", status);

        // Apply all pending migrations
        info!("Running database migrations...");
        Migrator::up(&self.conn, None)
            .await
            .context("Failed to run database migrations")?;
        info!("Database migrations completed successfully");

        Ok(())
    }

    /// Check if all migrations are complete
    ///
    /// Returns true if all migrations have been applied, false if there are pending migrations.
    pub async fn migrations_complete(&self) -> Result<bool> {
        // Get the total number of migrations defined
        let total_migrations = Migrator::migrations().len();

        // If there are no migrations defined, migrations are not complete (fresh database)
        if total_migrations == 0 {
            return Ok(false);
        }

        // Check if there are any pending migrations
        // If get_pending_migrations returns an empty vector, all migrations are applied
        let pending = Migrator::get_pending_migrations(&self.conn)
            .await
            .context("Failed to check pending migrations")?;

        // Migrations are complete if there are no pending migrations
        Ok(pending.is_empty())
    }

    /// Close the database connection
    pub async fn close(self) {
        // DatabaseConnection will be closed automatically when dropped
    }

    /// Check if the database connection is healthy
    pub async fn health_check(&self) -> Result<()> {
        self.conn
            .execute(sea_orm::Statement::from_string(
                self.conn.get_database_backend(),
                "SELECT 1".to_owned(),
            ))
            .await
            .context("Database health check failed")?;
        Ok(())
    }

    /// Get reference to SeaORM database connection
    pub fn sea_orm_connection(&self) -> &DatabaseConnection {
        &self.conn
    }

    // ============================================================================
    // Library Repository Methods
    // ============================================================================

    /// Create a new library
    pub async fn create_library(
        &self,
        name: &str,
        path: &str,
        strategy: ScanningStrategy,
    ) -> Result<entities::libraries::Model> {
        LibraryRepository::create(&self.conn, name, path, strategy).await
    }

    /// Get a library by ID
    pub async fn get_library_by_id(&self, id: Uuid) -> Result<Option<entities::libraries::Model>> {
        LibraryRepository::get_by_id(&self.conn, id).await
    }

    /// Get all libraries
    pub async fn list_libraries(&self) -> Result<Vec<entities::libraries::Model>> {
        LibraryRepository::list_all(&self.conn).await
    }

    /// Get library by path
    pub async fn get_library_by_path(
        &self,
        path: &str,
    ) -> Result<Option<entities::libraries::Model>> {
        LibraryRepository::get_by_path(&self.conn, path).await
    }

    /// Update library
    pub async fn update_library(&self, library: &entities::libraries::Model) -> Result<()> {
        LibraryRepository::update(&self.conn, library).await
    }

    /// Update last scanned timestamp
    pub async fn update_library_last_scanned(&self, id: Uuid) -> Result<()> {
        LibraryRepository::update_last_scanned(&self.conn, id).await
    }

    /// Delete a library
    pub async fn delete_library(&self, id: Uuid) -> Result<()> {
        LibraryRepository::delete(&self.conn, id).await
    }

    // ============================================================================
    // Series Repository Methods
    // ============================================================================

    /// Create a new series
    pub async fn create_series(
        &self,
        library_id: Uuid,
        name: &str,
    ) -> Result<entities::series::Model> {
        SeriesRepository::create(&self.conn, library_id, name, None).await
    }

    /// Get a series by ID
    pub async fn get_series_by_id(&self, id: Uuid) -> Result<Option<entities::series::Model>> {
        SeriesRepository::get_by_id(&self.conn, id).await
    }

    /// Get all series in a library
    pub async fn list_series_by_library(
        &self,
        library_id: Uuid,
    ) -> Result<Vec<entities::series::Model>> {
        SeriesRepository::list_by_library(&self.conn, library_id).await
    }

    /// Search series by name
    pub async fn search_series(&self, query: &str) -> Result<Vec<entities::series::Model>> {
        SeriesRepository::search_by_name(&self.conn, query).await
    }

    /// Update series
    pub async fn update_series(&self, series: &entities::series::Model) -> Result<()> {
        SeriesRepository::update(&self.conn, series, None).await
    }

    /// Increment book count for a series
    pub async fn increment_series_book_count(&self, id: Uuid) -> Result<()> {
        SeriesRepository::increment_book_count(&self.conn, id).await
    }

    /// Delete a series
    pub async fn delete_series(&self, id: Uuid) -> Result<()> {
        SeriesRepository::delete(&self.conn, id).await
    }

    // ============================================================================
    // Book Repository Methods
    // ============================================================================

    /// Create a new book
    pub async fn create_book(
        &self,
        book: &entities::books::Model,
    ) -> Result<entities::books::Model> {
        BookRepository::create(&self.conn, book, None).await
    }

    /// Get a book by ID
    pub async fn get_book_by_id(&self, id: Uuid) -> Result<Option<entities::books::Model>> {
        BookRepository::get_by_id(&self.conn, id).await
    }

    /// Get a book by file hash
    pub async fn get_book_by_hash(&self, hash: &str) -> Result<Option<entities::books::Model>> {
        BookRepository::get_by_hash(&self.conn, hash).await
    }

    /// Get a book by file path and library ID
    pub async fn get_book_by_path(
        &self,
        library_id: Uuid,
        path: &str,
    ) -> Result<Option<entities::books::Model>> {
        BookRepository::get_by_path(&self.conn, library_id, path).await
    }

    /// Get all books in a series
    pub async fn list_books_by_series(
        &self,
        series_id: Uuid,
    ) -> Result<Vec<entities::books::Model>> {
        BookRepository::list_by_series(&self.conn, series_id, false).await
    }

    /// Update book
    pub async fn update_book(&self, book: &entities::books::Model) -> Result<()> {
        BookRepository::update(&self.conn, book, None).await
    }

    /// Delete a book
    pub async fn delete_book(&self, id: Uuid) -> Result<()> {
        BookRepository::delete(&self.conn, id).await
    }

    // ============================================================================
    // Page Repository Methods
    // ============================================================================

    /// Create a new page
    pub async fn create_page(
        &self,
        page: &entities::pages::Model,
    ) -> Result<entities::pages::Model> {
        PageRepository::create(&self.conn, page).await
    }

    /// Create multiple pages in a batch
    pub async fn create_pages_batch(&self, pages: &[entities::pages::Model]) -> Result<()> {
        PageRepository::create_batch(&self.conn, pages).await
    }

    /// Get a page by ID
    pub async fn get_page_by_id(&self, id: Uuid) -> Result<Option<entities::pages::Model>> {
        PageRepository::get_by_id(&self.conn, id).await
    }

    /// Get all pages for a book
    pub async fn list_pages_by_book(&self, book_id: Uuid) -> Result<Vec<entities::pages::Model>> {
        PageRepository::list_by_book(&self.conn, book_id).await
    }

    /// Get a page by book ID and page number
    pub async fn get_page_by_book_and_number(
        &self,
        book_id: Uuid,
        page_number: i32,
    ) -> Result<Option<entities::pages::Model>> {
        PageRepository::get_by_book_and_number(&self.conn, book_id, page_number).await
    }

    /// Delete all pages for a book
    pub async fn delete_pages_by_book(&self, book_id: Uuid) -> Result<()> {
        PageRepository::delete_by_book(&self.conn, book_id).await
    }

    // ============================================================================
    // Book Metadata Repository Methods
    // ============================================================================

    /// Create or update book metadata
    pub async fn upsert_book_metadata(
        &self,
        metadata: &entities::book_metadata_records::Model,
    ) -> Result<entities::book_metadata_records::Model> {
        BookMetadataRepository::upsert(&self.conn, metadata).await
    }

    /// Get metadata by book ID
    pub async fn get_book_metadata(
        &self,
        book_id: Uuid,
    ) -> Result<Option<entities::book_metadata_records::Model>> {
        BookMetadataRepository::get_by_book_id(&self.conn, book_id).await
    }

    /// Update book metadata
    pub async fn update_book_metadata(
        &self,
        metadata: &entities::book_metadata_records::Model,
    ) -> Result<()> {
        BookMetadataRepository::update(&self.conn, metadata).await
    }

    /// Delete metadata by book ID
    pub async fn delete_book_metadata(&self, book_id: Uuid) -> Result<()> {
        BookMetadataRepository::delete_by_book_id(&self.conn, book_id).await
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

        // Verify SeaORM connection is available
        assert!(db.sea_orm_connection().ping().await.is_ok());

        db.close().await;
    }

    #[tokio::test]
    #[ignore] // Requires PostgreSQL server
    async fn test_database_new_postgres() {
        let config = DatabaseConfig {
            db_type: DatabaseType::Postgres,
            postgres: Some(crate::config::PostgresConfig {
                host: std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string()),
                port: std::env::var("POSTGRES_PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(5432),
                username: std::env::var("POSTGRES_USER")
                    .unwrap_or_else(|_| "codex_test".to_string()),
                password: std::env::var("POSTGRES_PASSWORD")
                    .unwrap_or_else(|_| "codex_test".to_string()),
                database_name: std::env::var("POSTGRES_DB")
                    .unwrap_or_else(|_| "codex_test".to_string()),
            }),
            sqlite: None,
        };

        let db = Database::new(&config).await.unwrap();

        // Health check
        assert!(db.health_check().await.is_ok());

        // Verify SeaORM connection is available
        assert!(db.sea_orm_connection().ping().await.is_ok());

        db.close().await;
    }
}
