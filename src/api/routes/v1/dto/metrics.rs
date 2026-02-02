use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;
use uuid::Uuid;

/// Application metrics response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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

// ============================================================
// Plugin Metrics DTOs
// ============================================================

/// Plugin metrics response - current performance statistics for all plugins
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginMetricsResponse {
    /// When the metrics were last updated
    #[schema(example = "2026-01-30T12:00:00Z")]
    pub updated_at: DateTime<Utc>,

    /// Overall summary statistics
    pub summary: PluginMetricsSummaryDto,

    /// Per-plugin breakdown
    pub plugins: Vec<PluginMetricsDto>,
}

/// Summary metrics across all plugins
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginMetricsSummaryDto {
    /// Total number of registered plugins
    #[schema(example = 3)]
    pub total_plugins: u64,

    /// Number of healthy plugins
    #[schema(example = 2)]
    pub healthy_plugins: u64,

    /// Number of degraded plugins
    #[schema(example = 1)]
    pub degraded_plugins: u64,

    /// Number of unhealthy plugins
    #[schema(example = 0)]
    pub unhealthy_plugins: u64,

    /// Total requests made across all plugins
    #[schema(example = 1500)]
    pub total_requests: u64,

    /// Total successful requests
    #[schema(example = 1400)]
    pub total_success: u64,

    /// Total failed requests
    #[schema(example = 100)]
    pub total_failed: u64,

    /// Total rate limit rejections
    #[schema(example = 5)]
    pub total_rate_limit_rejections: u64,
}

/// Metrics for a single plugin
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginMetricsDto {
    /// Plugin ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub plugin_id: Uuid,

    /// Plugin name
    #[schema(example = "AniList Provider")]
    pub plugin_name: String,

    /// Total requests made
    #[schema(example = 500)]
    pub requests_total: u64,

    /// Successful requests
    #[schema(example = 480)]
    pub requests_success: u64,

    /// Failed requests
    #[schema(example = 20)]
    pub requests_failed: u64,

    /// Average request duration in milliseconds
    #[schema(example = 250.5)]
    pub avg_duration_ms: f64,

    /// Number of rate limit rejections
    #[schema(example = 2)]
    pub rate_limit_rejections: u64,

    /// Error rate as percentage
    #[schema(example = 4.0)]
    pub error_rate_pct: f64,

    /// Last successful request timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_success: Option<DateTime<Utc>>,

    /// Last failure timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_failure: Option<DateTime<Utc>>,

    /// Current health status
    #[schema(example = "healthy")]
    pub health_status: String,

    /// Per-method breakdown
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by_method: Option<HashMap<String, PluginMethodMetricsDto>>,

    /// Failure counts by error code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_counts: Option<HashMap<String, u64>>,
}

/// Metrics breakdown by method for a plugin
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginMethodMetricsDto {
    /// Method name
    #[schema(example = "search")]
    pub method: String,

    /// Total requests for this method
    #[schema(example = 200)]
    pub requests_total: u64,

    /// Successful requests
    #[schema(example = 195)]
    pub requests_success: u64,

    /// Failed requests
    #[schema(example = 5)]
    pub requests_failed: u64,

    /// Average duration in milliseconds
    #[schema(example = 180.5)]
    pub avg_duration_ms: f64,
}
