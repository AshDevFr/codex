use anyhow::{Context, Result};
use uuid::Uuid;
use tracing::info;

use crate::config::{DatabaseConfig, DatabaseType};
use crate::db::entities;
use super::ScanningStrategy;

use super::postgres::PostgresDatabase;
use super::repositories::{
    BookMetadataRepository, BookRepository, LibraryRepository, PageRepository,
    SeriesRepository,
};
use super::sqlite::SqliteDatabase;
use migration::{Migrator, MigratorTrait};

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
    ///
    /// This will apply all pending migrations. Migrator::up() is idempotent,
    /// so it's safe to call multiple times - it will only run pending migrations.
    pub async fn run_migrations(&self) -> Result<()> {
        // Check migration status for logging
        let status = Migrator::status(self.sea_orm_connection())
            .await
            .context("Failed to check migration status")?;

        // Log migration status
        info!("Migration status: {:?}", status);

        // Apply all pending migrations
        info!("Running database migrations...");
        Migrator::up(self.sea_orm_connection(), None)
            .await
            .context("Failed to run database migrations")?;
        info!("Database migrations completed successfully");

        Ok(())
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

    /// Get reference to SeaORM database connection
    pub fn sea_orm_connection(&self) -> &sea_orm::DatabaseConnection {
        match self {
            Database::Sqlite(db) => db.sea_orm_connection(),
            Database::Postgres(db) => db.sea_orm_connection(),
        }
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
        LibraryRepository::create(self.sea_orm_connection(), name, path, strategy).await
    }

    /// Get a library by ID
    pub async fn get_library_by_id(&self, id: Uuid) -> Result<Option<entities::libraries::Model>> {
        LibraryRepository::get_by_id(self.sea_orm_connection(), id).await
    }

    /// Get all libraries
    pub async fn list_libraries(&self) -> Result<Vec<entities::libraries::Model>> {
        LibraryRepository::list_all(self.sea_orm_connection()).await
    }

    /// Get library by path
    pub async fn get_library_by_path(&self, path: &str) -> Result<Option<entities::libraries::Model>> {
        LibraryRepository::get_by_path(self.sea_orm_connection(), path).await
    }

    /// Update library
    pub async fn update_library(&self, library: &entities::libraries::Model) -> Result<()> {
        LibraryRepository::update(self.sea_orm_connection(), library).await
    }

    /// Update last scanned timestamp
    pub async fn update_library_last_scanned(&self, id: Uuid) -> Result<()> {
        LibraryRepository::update_last_scanned(self.sea_orm_connection(), id).await
    }

    /// Delete a library
    pub async fn delete_library(&self, id: Uuid) -> Result<()> {
        LibraryRepository::delete(self.sea_orm_connection(), id).await
    }

    // ============================================================================
    // Series Repository Methods
    // ============================================================================

    /// Create a new series
    pub async fn create_series(&self, library_id: Uuid, name: &str) -> Result<entities::series::Model> {
        SeriesRepository::create(self.sea_orm_connection(), library_id, name).await
    }

    /// Get a series by ID
    pub async fn get_series_by_id(&self, id: Uuid) -> Result<Option<entities::series::Model>> {
        SeriesRepository::get_by_id(self.sea_orm_connection(), id).await
    }

    /// Get all series in a library
    pub async fn list_series_by_library(&self, library_id: Uuid) -> Result<Vec<entities::series::Model>> {
        SeriesRepository::list_by_library(self.sea_orm_connection(), library_id).await
    }

    /// Search series by name
    pub async fn search_series(&self, query: &str) -> Result<Vec<entities::series::Model>> {
        SeriesRepository::search_by_name(self.sea_orm_connection(), query).await
    }

    /// Update series
    pub async fn update_series(&self, series: &entities::series::Model) -> Result<()> {
        SeriesRepository::update(self.sea_orm_connection(), series).await
    }

    /// Increment book count for a series
    pub async fn increment_series_book_count(&self, id: Uuid) -> Result<()> {
        SeriesRepository::increment_book_count(self.sea_orm_connection(), id).await
    }

    /// Delete a series
    pub async fn delete_series(&self, id: Uuid) -> Result<()> {
        SeriesRepository::delete(self.sea_orm_connection(), id).await
    }

    // ============================================================================
    // Book Repository Methods
    // ============================================================================

    /// Create a new book
    pub async fn create_book(&self, book: &entities::books::Model) -> Result<entities::books::Model> {
        BookRepository::create(self.sea_orm_connection(), book).await
    }

    /// Get a book by ID
    pub async fn get_book_by_id(&self, id: Uuid) -> Result<Option<entities::books::Model>> {
        BookRepository::get_by_id(self.sea_orm_connection(), id).await
    }

    /// Get a book by file hash
    pub async fn get_book_by_hash(&self, hash: &str) -> Result<Option<entities::books::Model>> {
        BookRepository::get_by_hash(self.sea_orm_connection(), hash).await
    }

    /// Get a book by file path
    pub async fn get_book_by_path(&self, path: &str) -> Result<Option<entities::books::Model>> {
        BookRepository::get_by_path(self.sea_orm_connection(), path).await
    }

    /// Get all books in a series
    pub async fn list_books_by_series(&self, series_id: Uuid) -> Result<Vec<entities::books::Model>> {
        BookRepository::list_by_series(self.sea_orm_connection(), series_id).await
    }

    /// Update book
    pub async fn update_book(&self, book: &entities::books::Model) -> Result<()> {
        BookRepository::update(self.sea_orm_connection(), book).await
    }

    /// Delete a book
    pub async fn delete_book(&self, id: Uuid) -> Result<()> {
        BookRepository::delete(self.sea_orm_connection(), id).await
    }

    // ============================================================================
    // Page Repository Methods
    // ============================================================================

    /// Create a new page
    pub async fn create_page(&self, page: &entities::pages::Model) -> Result<entities::pages::Model> {
        PageRepository::create(self.sea_orm_connection(), page).await
    }

    /// Create multiple pages in a batch
    pub async fn create_pages_batch(&self, pages: &[entities::pages::Model]) -> Result<()> {
        PageRepository::create_batch(self.sea_orm_connection(), pages).await
    }

    /// Get a page by ID
    pub async fn get_page_by_id(&self, id: Uuid) -> Result<Option<entities::pages::Model>> {
        PageRepository::get_by_id(self.sea_orm_connection(), id).await
    }

    /// Get all pages for a book
    pub async fn list_pages_by_book(&self, book_id: Uuid) -> Result<Vec<entities::pages::Model>> {
        PageRepository::list_by_book(self.sea_orm_connection(), book_id).await
    }

    /// Get a page by book ID and page number
    pub async fn get_page_by_book_and_number(
        &self,
        book_id: Uuid,
        page_number: i32,
    ) -> Result<Option<entities::pages::Model>> {
        PageRepository::get_by_book_and_number(self.sea_orm_connection(), book_id, page_number).await
    }

    /// Delete all pages for a book
    pub async fn delete_pages_by_book(&self, book_id: Uuid) -> Result<()> {
        PageRepository::delete_by_book(self.sea_orm_connection(), book_id).await
    }

    // ============================================================================
    // Book Metadata Repository Methods
    // ============================================================================

    /// Create or update book metadata
    pub async fn upsert_book_metadata(
        &self,
        metadata: &entities::book_metadata_records::Model,
    ) -> Result<entities::book_metadata_records::Model> {
        BookMetadataRepository::upsert(self.sea_orm_connection(), metadata).await
    }

    /// Get metadata by book ID
    pub async fn get_book_metadata(&self, book_id: Uuid) -> Result<Option<entities::book_metadata_records::Model>> {
        BookMetadataRepository::get_by_book_id(self.sea_orm_connection(), book_id).await
    }

    /// Update book metadata
    pub async fn update_book_metadata(&self, metadata: &entities::book_metadata_records::Model) -> Result<()> {
        BookMetadataRepository::update(self.sea_orm_connection(), metadata).await
    }

    /// Delete metadata by book ID
    pub async fn delete_book_metadata(&self, book_id: Uuid) -> Result<()> {
        BookMetadataRepository::delete_by_book_id(self.sea_orm_connection(), book_id).await
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

        // Verify SeaORM connection is available
        assert!(db.sea_orm_connection().ping().await.is_ok());

        db.close().await;
    }
}
