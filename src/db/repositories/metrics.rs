use anyhow::{Context, Result};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QuerySelect, Statement,
};
use uuid::Uuid;

use crate::db::entities::prelude::*;

/// Repository for gathering application metrics
pub struct MetricsRepository;

/// Library-specific metrics
#[derive(Debug, Clone)]
pub struct LibraryMetrics {
    pub id: Uuid,
    pub name: String,
    pub series_count: i64,
    pub book_count: i64,
    pub total_size: i64,
}

impl MetricsRepository {
    /// Get total count of libraries
    pub async fn count_libraries(db: &DatabaseConnection) -> Result<i64> {
        let count = Libraries::find()
            .count(db)
            .await
            .context("Failed to count libraries")?;
        Ok(count as i64)
    }

    /// Get total count of series across all libraries
    pub async fn count_series(db: &DatabaseConnection) -> Result<i64> {
        let count = Series::find()
            .count(db)
            .await
            .context("Failed to count series")?;
        Ok(count as i64)
    }

    /// Get total count of books across all libraries
    pub async fn count_books(db: &DatabaseConnection) -> Result<i64> {
        let count = Books::find()
            .count(db)
            .await
            .context("Failed to count books")?;
        Ok(count as i64)
    }

    /// Get total count of pages across all books
    pub async fn count_pages(db: &DatabaseConnection) -> Result<i64> {
        let count = Pages::find()
            .count(db)
            .await
            .context("Failed to count pages")?;
        Ok(count as i64)
    }

    /// Get total count of users
    pub async fn count_users(db: &DatabaseConnection) -> Result<i64> {
        let count = Users::find()
            .count(db)
            .await
            .context("Failed to count users")?;
        Ok(count as i64)
    }

    /// Get total size of all books in bytes
    pub async fn total_book_size(db: &DatabaseConnection) -> Result<i64> {
        use sea_orm::sea_query::Expr;

        let result = Books::find()
            .select_only()
            .column_as(Expr::cust("COALESCE(SUM(file_size), 0)"), "total_size")
            .into_tuple::<i64>()
            .one(db)
            .await
            .context("Failed to calculate total book size")?;

        Ok(result.unwrap_or(0))
    }

    /// Get database size (approximate, platform-dependent)
    /// For SQLite, returns the actual file size
    /// For PostgreSQL, returns the size of the current database
    pub async fn database_size(db: &DatabaseConnection) -> Result<i64> {
        // Get the database backend type
        let backend = db.get_database_backend();

        use sea_orm::DbBackend;
        match backend {
            DbBackend::Sqlite => {
                // For SQLite, we could query PRAGMA page_count * page_size
                // But we need raw SQL for this
                let page_count_result = db
                    .query_one(Statement::from_string(
                        backend,
                        "PRAGMA page_count".to_string(),
                    ))
                    .await
                    .context("Failed to get page count")?;

                let page_size_result = db
                    .query_one(Statement::from_string(
                        backend,
                        "PRAGMA page_size".to_string(),
                    ))
                    .await
                    .context("Failed to get page size")?;

                if let (Some(pc_row), Some(ps_row)) = (page_count_result, page_size_result) {
                    let page_count: i64 = pc_row.try_get("", "page_count").unwrap_or(0);
                    let page_size: i64 = ps_row.try_get("", "page_size").unwrap_or(0);
                    Ok(page_count * page_size)
                } else {
                    Ok(0)
                }
            }
            DbBackend::Postgres => {
                // For PostgreSQL, use pg_database_size
                let result = db
                    .query_one(Statement::from_string(
                        backend,
                        "SELECT pg_database_size(current_database()) as size".to_string(),
                    ))
                    .await
                    .context("Failed to get database size")?;

                if let Some(row) = result {
                    Ok(row.try_get("", "size").unwrap_or(0))
                } else {
                    Ok(0)
                }
            }
            _ => {
                // Unknown backend, return 0
                Ok(0)
            }
        }
    }

    /// Get metrics broken down by library
    pub async fn library_metrics(db: &DatabaseConnection) -> Result<Vec<LibraryMetrics>> {
        use crate::db::entities::series;
        use sea_orm::{ColumnTrait, QueryFilter};

        // Get all libraries
        let libraries_list = Libraries::find()
            .all(db)
            .await
            .context("Failed to fetch libraries")?;

        let mut metrics = Vec::new();

        for library in libraries_list {
            // Count series in this library
            let series_count = Series::find()
                .filter(series::Column::LibraryId.eq(library.id))
                .count(db)
                .await
                .context("Failed to count series for library")?
                as i64;

            // Count books and total size for this library
            // We need to join books with series to filter by library
            use sea_orm::sea_query::Expr;

            let book_stats = Books::find()
                .inner_join(Series)
                .filter(series::Column::LibraryId.eq(library.id))
                .select_only()
                .column_as(Expr::cust("COUNT(*)"), "book_count")
                .column_as(Expr::cust("COALESCE(SUM(file_size), 0)"), "total_size")
                .into_tuple::<(i64, i64)>()
                .one(db)
                .await
                .context("Failed to get book stats for library")?;

            let (book_count, total_size) = book_stats.unwrap_or((0, 0));

            metrics.push(LibraryMetrics {
                id: library.id,
                name: library.name,
                series_count,
                book_count,
                total_size,
            });
        }

        Ok(metrics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::books;
    use crate::db::repositories::{BookRepository, LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;
    use crate::db::ScanningStrategy;
    use chrono::Utc;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_count_libraries() {
        let (db, _temp_dir) = create_test_db().await;

        // Initially should be 0
        let count = MetricsRepository::count_libraries(db.sea_orm_connection())
            .await
            .unwrap();
        assert_eq!(count, 0);

        // Create a library
        LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Should now be 1
        let count = MetricsRepository::count_libraries(db.sea_orm_connection())
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_count_series_and_books() {
        let (db, _temp_dir) = create_test_db().await;

        // Create library
        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Create series
        let series = SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series")
            .await
            .unwrap();

        // Create book
        let now = Utc::now();
        let book_model = books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            title: None,
            number: None,
            file_path: "/test/path/series/book.cbz".to_string(),
            file_name: "book.cbz".to_string(),
            file_size: 1000000,
            file_hash: "abc123".to_string(),
            format: "cbz".to_string(),
            page_count: 10,
            deleted: false,
            analyzed: false,
            modified_at: now,
            created_at: now,
            updated_at: now,
        };

        BookRepository::create(db.sea_orm_connection(), &book_model)
            .await
            .unwrap();

        // Check counts
        let series_count = MetricsRepository::count_series(db.sea_orm_connection())
            .await
            .unwrap();
        assert_eq!(series_count, 1);

        let book_count = MetricsRepository::count_books(db.sea_orm_connection())
            .await
            .unwrap();
        assert_eq!(book_count, 1);

        let total_size = MetricsRepository::total_book_size(db.sea_orm_connection())
            .await
            .unwrap();
        assert_eq!(total_size, 1000000);
    }

    #[tokio::test]
    async fn test_library_metrics() {
        let (db, _temp_dir) = create_test_db().await;

        // Create library
        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Create series
        let series = SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series")
            .await
            .unwrap();

        // Create book
        let now = Utc::now();
        let book_model = books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            title: None,
            number: None,
            file_path: "/test/path/series/book.cbz".to_string(),
            file_name: "book.cbz".to_string(),
            file_size: 1000000,
            file_hash: "abc123".to_string(),
            format: "cbz".to_string(),
            page_count: 10,
            deleted: false,
            analyzed: false,
            modified_at: now,
            created_at: now,
            updated_at: now,
        };

        BookRepository::create(db.sea_orm_connection(), &book_model)
            .await
            .unwrap();

        // Get library metrics
        let metrics = MetricsRepository::library_metrics(db.sea_orm_connection())
            .await
            .unwrap();

        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "Test Library");
        assert_eq!(metrics[0].series_count, 1);
        assert_eq!(metrics[0].book_count, 1);
        assert_eq!(metrics[0].total_size, 1000000);
    }
}
