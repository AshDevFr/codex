use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Application metrics response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MetricsDto {
    /// Total number of libraries in the system
    #[schema(example = 5)]
    pub library_count: i64,

    /// Total number of series across all libraries
    #[schema(example = 150)]
    pub series_count: i64,

    /// Total number of books across all libraries
    #[schema(example = 3500)]
    pub book_count: i64,

    /// Total size of all books in bytes (approx. 50GB)
    #[schema(example = "52428800000")]
    pub total_book_size: i64,

    /// Number of registered users
    #[schema(example = 12)]
    pub user_count: i64,

    /// Database size in bytes (approximate)
    #[schema(example = 10485760)]
    pub database_size: i64,

    /// Number of pages across all books
    #[schema(example = 175000)]
    pub page_count: i64,

    /// Breakdown by library
    pub libraries: Vec<LibraryMetricsDto>,
}

/// Metrics for a single library
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LibraryMetricsDto {
    /// Library ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: uuid::Uuid,

    /// Library name
    #[schema(example = "Comics")]
    pub name: String,

    /// Number of series in this library
    #[schema(example = 45)]
    pub series_count: i64,

    /// Number of books in this library
    #[schema(example = 1200)]
    pub book_count: i64,

    /// Total size of books in bytes (approx. 15GB)
    #[schema(example = "15728640000")]
    pub total_size: i64,
}
