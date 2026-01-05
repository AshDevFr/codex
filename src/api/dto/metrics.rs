use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Application metrics response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MetricsDto {
    /// Total number of libraries in the system
    pub library_count: i64,
    /// Total number of series across all libraries
    pub series_count: i64,
    /// Total number of books across all libraries
    pub book_count: i64,
    /// Total size of all books in bytes
    pub total_book_size: i64,
    /// Number of registered users
    pub user_count: i64,
    /// Database size in bytes (approximate)
    pub database_size: i64,
    /// Number of pages across all books
    pub page_count: i64,
    /// Breakdown by library
    pub libraries: Vec<LibraryMetricsDto>,
}

/// Metrics for a single library
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LibraryMetricsDto {
    /// Library ID
    pub id: uuid::Uuid,
    /// Library name
    pub name: String,
    /// Number of series in this library
    pub series_count: i64,
    /// Number of books in this library
    pub book_count: i64,
    /// Total size of books in bytes
    pub total_size: i64,
}
