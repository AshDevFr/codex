use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Pagination parameters for list endpoints
#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct PaginationParams {
    /// Page number (0-indexed)
    #[serde(default)]
    pub page: u64,

    /// Number of items per page (max 100)
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

fn default_page_size() -> u64 {
    20
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: 0,
            page_size: default_page_size(),
        }
    }
}

impl PaginationParams {
    /// Validate and clamp pagination parameters
    pub fn validate(mut self, max_page_size: u64) -> Self {
        if self.page_size == 0 {
            self.page_size = default_page_size();
        }
        if self.page_size > max_page_size {
            self.page_size = max_page_size;
        }
        self
    }

    /// Calculate offset for database queries
    pub fn offset(&self) -> u64 {
        self.page * self.page_size
    }

    /// Get limit for database queries
    pub fn limit(&self) -> u64 {
        self.page_size
    }
}

/// Generic paginated response wrapper
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    /// The data items for this page
    pub data: Vec<T>,

    /// Current page number (0-indexed)
    pub page: u64,

    /// Number of items per page
    pub page_size: u64,

    /// Total number of items across all pages
    pub total: u64,

    /// Total number of pages
    pub total_pages: u64,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, page: u64, page_size: u64, total: u64) -> Self {
        let total_pages = if page_size == 0 {
            0
        } else {
            (total + page_size - 1) / page_size
        };

        Self {
            data,
            page,
            page_size,
            total,
            total_pages,
        }
    }
}
