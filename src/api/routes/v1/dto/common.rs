use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

// =============================================================================
// Pagination Constants
// =============================================================================

/// Default page size for list endpoints
pub const DEFAULT_PAGE_SIZE: u64 = 50;

/// Maximum allowed page size
pub const MAX_PAGE_SIZE: u64 = 500;

/// Default page number (1-indexed)
pub const DEFAULT_PAGE: u64 = 1;

fn default_page_size() -> u64 {
    DEFAULT_PAGE_SIZE
}

fn default_page() -> u64 {
    DEFAULT_PAGE
}

// =============================================================================
// Pagination Parameters
// =============================================================================

/// Pagination parameters for list endpoints
#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Public API - fields read by serde deserialization
pub struct PaginationParams {
    /// Page number (1-indexed, minimum 1)
    #[serde(default = "default_page")]
    pub page: u64,

    /// Number of items per page (max 100, default 50)
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            page_size: DEFAULT_PAGE_SIZE,
        }
    }
}

#[allow(dead_code)] // Public API - used for pagination in list endpoints
impl PaginationParams {
    /// Validate and clamp pagination parameters
    /// - If page is 0, treats it as page 1 (backward compatibility)
    /// - Clamps page_size to max_page_size
    pub fn validate(mut self, max_page_size: u64) -> Self {
        // Treat page 0 as page 1 for backward compatibility
        if self.page == 0 {
            self.page = 1;
        }
        if self.page_size == 0 {
            self.page_size = DEFAULT_PAGE_SIZE;
        }
        if self.page_size > max_page_size {
            self.page_size = max_page_size;
        }
        self
    }

    /// Calculate offset for database queries (converts 1-indexed page to 0-indexed offset)
    pub fn offset(&self) -> u64 {
        self.page.saturating_sub(1) * self.page_size
    }

    /// Get limit for database queries
    pub fn limit(&self) -> u64 {
        self.page_size
    }
}

// =============================================================================
// HATEOAS Pagination Links
// =============================================================================

/// HATEOAS navigation links for paginated responses (RFC 8288)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PaginationLinks {
    /// Link to the current page
    #[serde(rename = "self")]
    pub self_link: String,

    /// Link to the first page
    pub first: String,

    /// Link to the previous page (null if on first page)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev: Option<String>,

    /// Link to the next page (null if on last page)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,

    /// Link to the last page
    pub last: String,
}

// =============================================================================
// Paginated Response
// =============================================================================

/// Generic paginated response wrapper with HATEOAS links
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    /// The data items for this page
    pub data: Vec<T>,

    /// Current page number (1-indexed)
    #[schema(example = 1)]
    pub page: u64,

    /// Number of items per page
    #[schema(example = 50)]
    pub page_size: u64,

    /// Total number of items across all pages
    #[schema(example = 150)]
    pub total: u64,

    /// Total number of pages
    #[schema(example = 3)]
    pub total_pages: u64,

    /// HATEOAS navigation links
    pub links: PaginationLinks,
}

impl<T> PaginatedResponse<T> {
    /// Create a new paginated response (backward compatible)
    ///
    /// This constructor maintains backward compatibility with existing code.
    /// Links will be empty placeholders - use `with_path` or `with_builder` for proper HATEOAS links.
    ///
    /// # Arguments
    /// * `data` - The items for this page
    /// * `page` - Current page number (1-indexed)
    /// * `page_size` - Items per page
    /// * `total` - Total number of items across all pages
    #[deprecated(
        since = "0.2.0",
        note = "Use `with_path` or `with_builder` for proper HATEOAS links"
    )]
    #[allow(dead_code)] // Used in tests
    pub fn new(data: Vec<T>, page: u64, page_size: u64, total: u64) -> Self {
        let total_pages = if page_size == 0 {
            0
        } else {
            total.div_ceil(page_size)
        };

        // Create placeholder links for backward compatibility
        let links = PaginationLinks {
            self_link: String::new(),
            first: String::new(),
            prev: None,
            next: None,
            last: String::new(),
        };

        Self {
            data,
            page,
            page_size,
            total,
            total_pages,
            links,
        }
    }

    /// Create a new paginated response with HATEOAS links
    ///
    /// # Arguments
    /// * `data` - The items for this page
    /// * `page` - Current page number (1-indexed)
    /// * `page_size` - Items per page
    /// * `total` - Total number of items across all pages
    /// * `base_path` - Base URL path for generating links (e.g., "/api/v1/books")
    #[allow(dead_code)] // Used in tests
    pub fn with_path(data: Vec<T>, page: u64, page_size: u64, total: u64, base_path: &str) -> Self {
        let total_pages = if page_size == 0 {
            0
        } else {
            total.div_ceil(page_size)
        };

        let links = PaginationLinkBuilder::new(base_path, page, page_size, total_pages).build();

        Self {
            data,
            page,
            page_size,
            total,
            total_pages,
            links,
        }
    }

    /// Create a new paginated response with custom links builder
    ///
    /// Use this when you need to add additional query parameters to the links
    pub fn with_builder(
        data: Vec<T>,
        page: u64,
        page_size: u64,
        total: u64,
        builder: &PaginationLinkBuilder,
    ) -> Self {
        let total_pages = if page_size == 0 {
            0
        } else {
            total.div_ceil(page_size)
        };

        Self {
            data,
            page,
            page_size,
            total,
            total_pages,
            links: builder.build(),
        }
    }
}

// =============================================================================
// Pagination Link Builder
// =============================================================================

/// Builder for creating HATEOAS pagination links and RFC 8288 Link headers
#[derive(Debug, Clone)]
pub struct PaginationLinkBuilder {
    base_path: String,
    current_page: u64,
    page_size: u64,
    total_pages: u64,
    additional_params: Vec<(String, String)>,
}

impl PaginationLinkBuilder {
    /// Create a new pagination link builder
    ///
    /// # Arguments
    /// * `base_path` - Base URL path (e.g., "/api/v1/books")
    /// * `page` - Current page number (1-indexed)
    /// * `page_size` - Items per page
    /// * `total_pages` - Total number of pages
    pub fn new(base_path: &str, page: u64, page_size: u64, total_pages: u64) -> Self {
        Self {
            base_path: base_path.to_string(),
            current_page: page.max(1),
            page_size,
            total_pages: total_pages.max(1),
            additional_params: Vec::new(),
        }
    }

    /// Add an additional query parameter to all generated links
    pub fn with_param(mut self, key: &str, value: &str) -> Self {
        self.additional_params
            .push((key.to_string(), value.to_string()));
        self
    }

    /// Add an optional parameter (only adds if value is Some)
    #[allow(dead_code)] // Useful helper for future use
    pub fn with_optional_param(self, key: &str, value: Option<&str>) -> Self {
        match value {
            Some(v) => self.with_param(key, v),
            None => self,
        }
    }

    /// Build a URL for a specific page
    fn build_url(&self, page: u64) -> String {
        let mut params = vec![
            format!("page={}", page),
            format!("page_size={}", self.page_size),
        ];

        for (key, value) in &self.additional_params {
            // URL-encode the value for safety
            params.push(format!("{}={}", key, urlencoding::encode(value)));
        }

        format!("{}?{}", self.base_path, params.join("&"))
    }

    /// Build the HATEOAS pagination links
    pub fn build(&self) -> PaginationLinks {
        PaginationLinks {
            self_link: self.build_url(self.current_page),
            first: self.build_url(1),
            prev: if self.current_page > 1 {
                Some(self.build_url(self.current_page - 1))
            } else {
                None
            },
            next: if self.current_page < self.total_pages {
                Some(self.build_url(self.current_page + 1))
            } else {
                None
            },
            last: self.build_url(self.total_pages),
        }
    }

    /// Build an RFC 8288 Link header value
    ///
    /// Format: `<url>; rel="relation", <url>; rel="relation", ...`
    pub fn build_link_header(&self) -> String {
        let mut links = vec![format!("<{}>; rel=\"first\"", self.build_url(1))];

        if self.current_page > 1 {
            links.push(format!(
                "<{}>; rel=\"prev\"",
                self.build_url(self.current_page - 1)
            ));
        }

        links.push(format!(
            "<{}>; rel=\"self\"",
            self.build_url(self.current_page)
        ));

        if self.current_page < self.total_pages {
            links.push(format!(
                "<{}>; rel=\"next\"",
                self.build_url(self.current_page + 1)
            ));
        }

        links.push(format!(
            "<{}>; rel=\"last\"",
            self.build_url(self.total_pages)
        ));

        links.join(", ")
    }

    /// Get the total pages (useful for response construction)
    #[allow(dead_code)] // Useful helper for future use
    pub fn total_pages(&self) -> u64 {
        self.total_pages
    }
}

// =============================================================================
// Cursor-Based Pagination
// =============================================================================

/// A cursor for cursor-based pagination
///
/// Cursors encode the sort keys and ID of the last item on the current page,
/// allowing efficient pagination without using OFFSET (which becomes slow on large datasets).
///
/// The cursor is base64-encoded JSON containing:
/// - `sort_values`: The values of the sort columns for the last item
/// - `id`: The UUID of the last item (used as tiebreaker)
#[allow(dead_code)] // Prepared for future cursor-based pagination endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationCursor {
    /// Values of the sort columns (in order) for the cursor position
    pub sort_values: Vec<serde_json::Value>,
    /// The ID of the item at cursor position (tiebreaker for stable pagination)
    pub id: uuid::Uuid,
}

#[allow(dead_code)] // Prepared for future cursor-based pagination endpoints
impl PaginationCursor {
    /// Create a new cursor from sort values and ID
    pub fn new(sort_values: Vec<serde_json::Value>, id: uuid::Uuid) -> Self {
        Self { sort_values, id }
    }

    /// Encode the cursor as a base64 string for use in URLs
    pub fn encode(&self) -> String {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        let json = serde_json::to_string(self).unwrap_or_default();
        URL_SAFE_NO_PAD.encode(json.as_bytes())
    }

    /// Decode a cursor from a base64 string
    ///
    /// Returns `None` if the string is invalid base64 or invalid JSON
    pub fn decode(s: &str) -> Option<Self> {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        let bytes = URL_SAFE_NO_PAD.decode(s).ok()?;
        let json = String::from_utf8(bytes).ok()?;
        serde_json::from_str(&json).ok()
    }

    /// Create a cursor from a single string sort value and ID
    pub fn from_string_value(value: &str, id: uuid::Uuid) -> Self {
        Self {
            sort_values: vec![serde_json::Value::String(value.to_string())],
            id,
        }
    }

    /// Create a cursor from a single integer sort value and ID
    pub fn from_i64_value(value: i64, id: uuid::Uuid) -> Self {
        Self {
            sort_values: vec![serde_json::Value::Number(value.into())],
            id,
        }
    }

    /// Create a cursor from a single optional integer sort value and ID
    pub fn from_optional_i64_value(value: Option<i64>, id: uuid::Uuid) -> Self {
        Self {
            sort_values: vec![match value {
                Some(v) => serde_json::Value::Number(v.into()),
                None => serde_json::Value::Null,
            }],
            id,
        }
    }

    /// Get the first sort value as a string (if present)
    pub fn first_string_value(&self) -> Option<&str> {
        self.sort_values.first().and_then(|v| v.as_str())
    }

    /// Get the first sort value as an i64 (if present)
    pub fn first_i64_value(&self) -> Option<i64> {
        self.sort_values.first().and_then(|v| v.as_i64())
    }
}

/// HATEOAS navigation links for cursor-paginated responses
#[allow(dead_code)] // Prepared for future cursor-based pagination endpoints
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CursorPaginationLinks {
    /// Link to the current page
    #[serde(rename = "self")]
    pub self_link: String,

    /// Link to the first page (no cursor)
    pub first: String,

    /// Link to the next page (null if no more items)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

/// Generic cursor-paginated response wrapper
///
/// This is an alternative to offset-based pagination that performs better
/// on large datasets. Instead of `page` and `total_pages`, it uses opaque
/// cursors for navigation.
#[allow(dead_code)] // Prepared for future cursor-based pagination endpoints
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CursorPaginatedResponse<T> {
    /// The data items for this page
    pub data: Vec<T>,

    /// Number of items per page
    #[schema(example = 50)]
    pub page_size: u64,

    /// Cursor for the next page (null if no more items)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,

    /// Whether there are more items after this page
    #[schema(example = true)]
    pub has_more: bool,

    /// HATEOAS navigation links
    pub links: CursorPaginationLinks,
}

#[allow(dead_code)] // Prepared for future cursor-based pagination endpoints
impl<T> CursorPaginatedResponse<T> {
    /// Create a new cursor-paginated response
    ///
    /// # Arguments
    /// * `data` - The items for this page
    /// * `page_size` - Items per page
    /// * `next_cursor` - Cursor for the next page (if there are more items)
    /// * `base_path` - Base URL path for generating links
    /// * `current_cursor` - The cursor used for the current request (if any)
    pub fn new(
        data: Vec<T>,
        page_size: u64,
        next_cursor: Option<String>,
        base_path: &str,
        current_cursor: Option<&str>,
    ) -> Self {
        let has_more = next_cursor.is_some();

        // Build self link
        let self_link = match current_cursor {
            Some(cursor) => format!("{}?page_size={}&cursor={}", base_path, page_size, cursor),
            None => format!("{}?page_size={}", base_path, page_size),
        };

        // Build first link (no cursor)
        let first = format!("{}?page_size={}", base_path, page_size);

        // Build next link
        let next = next_cursor
            .as_ref()
            .map(|cursor| format!("{}?page_size={}&cursor={}", base_path, page_size, cursor));

        Self {
            data,
            page_size,
            next_cursor,
            has_more,
            links: CursorPaginationLinks {
                self_link,
                first,
                next,
            },
        }
    }

    /// Create a response with additional query parameters in links
    pub fn with_params(
        data: Vec<T>,
        page_size: u64,
        next_cursor: Option<String>,
        base_path: &str,
        current_cursor: Option<&str>,
        additional_params: &[(String, String)],
    ) -> Self {
        let has_more = next_cursor.is_some();

        // Build query string for additional params
        let extra_params: String = additional_params
            .iter()
            .map(|(k, v)| format!("&{}={}", k, urlencoding::encode(v)))
            .collect();

        // Build self link
        let self_link = match current_cursor {
            Some(cursor) => format!(
                "{}?page_size={}&cursor={}{}",
                base_path, page_size, cursor, extra_params
            ),
            None => format!("{}?page_size={}{}", base_path, page_size, extra_params),
        };

        // Build first link (no cursor)
        let first = format!("{}?page_size={}{}", base_path, page_size, extra_params);

        // Build next link
        let next = next_cursor.as_ref().map(|cursor| {
            format!(
                "{}?page_size={}&cursor={}{}",
                base_path, page_size, cursor, extra_params
            )
        });

        Self {
            data,
            page_size,
            next_cursor,
            has_more,
            links: CursorPaginationLinks {
                self_link,
                first,
                next,
            },
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_params_defaults() {
        let params = PaginationParams::default();
        assert_eq!(params.page, 1);
        assert_eq!(params.page_size, 50);
    }

    #[test]
    fn test_pagination_params_offset_calculation() {
        // Page 1 should have offset 0
        let params = PaginationParams {
            page: 1,
            page_size: 50,
        };
        assert_eq!(params.offset(), 0);

        // Page 2 should have offset 50
        let params = PaginationParams {
            page: 2,
            page_size: 50,
        };
        assert_eq!(params.offset(), 50);

        // Page 3 with page_size 20 should have offset 40
        let params = PaginationParams {
            page: 3,
            page_size: 20,
        };
        assert_eq!(params.offset(), 40);
    }

    #[test]
    fn test_pagination_params_validate_page_zero() {
        // Page 0 should be treated as page 1
        let params = PaginationParams {
            page: 0,
            page_size: 50,
        };
        let validated = params.validate(100);
        assert_eq!(validated.page, 1);
    }

    #[test]
    fn test_pagination_params_validate_page_size_zero() {
        let params = PaginationParams {
            page: 1,
            page_size: 0,
        };
        let validated = params.validate(100);
        assert_eq!(validated.page_size, DEFAULT_PAGE_SIZE);
    }

    #[test]
    fn test_pagination_params_validate_max_page_size() {
        let params = PaginationParams {
            page: 1,
            page_size: 500,
        };
        let validated = params.validate(100);
        assert_eq!(validated.page_size, 100);
    }

    #[test]
    fn test_pagination_link_builder_first_page() {
        let builder = PaginationLinkBuilder::new("/api/v1/books", 1, 50, 10);
        let links = builder.build();

        assert_eq!(links.self_link, "/api/v1/books?page=1&page_size=50");
        assert_eq!(links.first, "/api/v1/books?page=1&page_size=50");
        assert!(links.prev.is_none());
        assert_eq!(
            links.next,
            Some("/api/v1/books?page=2&page_size=50".to_string())
        );
        assert_eq!(links.last, "/api/v1/books?page=10&page_size=50");
    }

    #[test]
    fn test_pagination_link_builder_middle_page() {
        let builder = PaginationLinkBuilder::new("/api/v1/series", 5, 20, 10);
        let links = builder.build();

        assert_eq!(links.self_link, "/api/v1/series?page=5&page_size=20");
        assert_eq!(links.first, "/api/v1/series?page=1&page_size=20");
        assert_eq!(
            links.prev,
            Some("/api/v1/series?page=4&page_size=20".to_string())
        );
        assert_eq!(
            links.next,
            Some("/api/v1/series?page=6&page_size=20".to_string())
        );
        assert_eq!(links.last, "/api/v1/series?page=10&page_size=20");
    }

    #[test]
    fn test_pagination_link_builder_last_page() {
        let builder = PaginationLinkBuilder::new("/api/v1/books", 10, 50, 10);
        let links = builder.build();

        assert_eq!(links.self_link, "/api/v1/books?page=10&page_size=50");
        assert_eq!(
            links.prev,
            Some("/api/v1/books?page=9&page_size=50".to_string())
        );
        assert!(links.next.is_none());
        assert_eq!(links.last, "/api/v1/books?page=10&page_size=50");
    }

    #[test]
    fn test_pagination_link_builder_single_page() {
        let builder = PaginationLinkBuilder::new("/api/v1/books", 1, 50, 1);
        let links = builder.build();

        assert!(links.prev.is_none());
        assert!(links.next.is_none());
        assert_eq!(links.first, links.last);
    }

    #[test]
    fn test_pagination_link_builder_with_params() {
        let builder = PaginationLinkBuilder::new("/api/v1/books", 1, 50, 10)
            .with_param("library_id", "abc-123")
            .with_param("sort", "name,asc");
        let links = builder.build();

        assert!(links.self_link.contains("library_id=abc-123"));
        assert!(links.self_link.contains("sort=name%2Casc")); // URL encoded
    }

    #[test]
    fn test_pagination_link_builder_link_header() {
        let builder = PaginationLinkBuilder::new("/api/v1/books", 2, 50, 5);
        let header = builder.build_link_header();

        assert!(header.contains("rel=\"first\""));
        assert!(header.contains("rel=\"prev\""));
        assert!(header.contains("rel=\"self\""));
        assert!(header.contains("rel=\"next\""));
        assert!(header.contains("rel=\"last\""));
        assert!(header.contains("page=1"));
        assert!(header.contains("page=2")); // self
        assert!(header.contains("page=3")); // next
        assert!(header.contains("page=5")); // last
    }

    #[test]
    fn test_paginated_response_with_path() {
        let data = vec!["a", "b", "c"];
        let response = PaginatedResponse::with_path(data, 1, 50, 100, "/api/v1/items");

        assert_eq!(response.page, 1);
        assert_eq!(response.page_size, 50);
        assert_eq!(response.total, 100);
        assert_eq!(response.total_pages, 2);
        assert_eq!(response.data.len(), 3);
        assert!(response.links.next.is_some());
    }

    #[test]
    #[allow(deprecated)]
    fn test_paginated_response_new_backward_compat() {
        // Test that the deprecated `new` still works for backward compatibility
        let data = vec!["a", "b", "c"];
        let response = PaginatedResponse::new(data, 1, 50, 100);

        assert_eq!(response.page, 1);
        assert_eq!(response.page_size, 50);
        assert_eq!(response.total, 100);
        assert_eq!(response.total_pages, 2);
        assert_eq!(response.data.len(), 3);
        // Links should be empty placeholders
        assert!(response.links.self_link.is_empty());
    }

    #[test]
    fn test_paginated_response_total_pages_calculation() {
        // 100 items with page_size 50 = 2 pages
        let response: PaginatedResponse<()> =
            PaginatedResponse::with_path(vec![], 1, 50, 100, "/test");
        assert_eq!(response.total_pages, 2);

        // 101 items with page_size 50 = 3 pages
        let response: PaginatedResponse<()> =
            PaginatedResponse::with_path(vec![], 1, 50, 101, "/test");
        assert_eq!(response.total_pages, 3);

        // 0 items = 0 pages
        let response: PaginatedResponse<()> =
            PaginatedResponse::with_path(vec![], 1, 50, 0, "/test");
        assert_eq!(response.total_pages, 0);
    }

    // =========================================================================
    // Cursor-Based Pagination Tests
    // =========================================================================

    #[test]
    fn test_pagination_cursor_encode_decode() {
        let id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let cursor = PaginationCursor::from_string_value("test_value", id);

        let encoded = cursor.encode();
        let decoded = PaginationCursor::decode(&encoded).unwrap();

        assert_eq!(decoded.id, id);
        assert_eq!(decoded.first_string_value(), Some("test_value"));
    }

    #[test]
    fn test_pagination_cursor_decode_invalid() {
        // Invalid base64
        assert!(PaginationCursor::decode("not-valid-base64!@#").is_none());

        // Valid base64 but invalid JSON
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        let invalid_json = URL_SAFE_NO_PAD.encode(b"not valid json");
        assert!(PaginationCursor::decode(&invalid_json).is_none());
    }

    #[test]
    fn test_pagination_cursor_from_i64_value() {
        let id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let cursor = PaginationCursor::from_i64_value(12345, id);

        assert_eq!(cursor.first_i64_value(), Some(12345));
        assert_eq!(cursor.id, id);

        // Round-trip test
        let encoded = cursor.encode();
        let decoded = PaginationCursor::decode(&encoded).unwrap();
        assert_eq!(decoded.first_i64_value(), Some(12345));
    }

    #[test]
    fn test_pagination_cursor_from_optional_i64_value() {
        let id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();

        // With Some value
        let cursor = PaginationCursor::from_optional_i64_value(Some(42), id);
        assert_eq!(cursor.first_i64_value(), Some(42));

        // With None value
        let cursor = PaginationCursor::from_optional_i64_value(None, id);
        assert!(cursor.first_i64_value().is_none());
        assert!(cursor.sort_values[0].is_null());
    }

    #[test]
    fn test_pagination_cursor_multiple_sort_values() {
        let id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let cursor = PaginationCursor::new(
            vec![
                serde_json::Value::String("alpha".to_string()),
                serde_json::Value::Number(100.into()),
            ],
            id,
        );

        let encoded = cursor.encode();
        let decoded = PaginationCursor::decode(&encoded).unwrap();

        assert_eq!(decoded.sort_values.len(), 2);
        assert_eq!(decoded.sort_values[0].as_str(), Some("alpha"));
        assert_eq!(decoded.sort_values[1].as_i64(), Some(100));
    }

    #[test]
    fn test_cursor_paginated_response_with_next() {
        let data = vec!["a", "b", "c"];
        let next_cursor = Some("abc123".to_string());
        let response = CursorPaginatedResponse::new(data, 50, next_cursor, "/api/v1/books", None);

        assert_eq!(response.page_size, 50);
        assert!(response.has_more);
        assert_eq!(response.next_cursor, Some("abc123".to_string()));
        assert_eq!(response.links.first, "/api/v1/books?page_size=50");
        assert_eq!(
            response.links.next,
            Some("/api/v1/books?page_size=50&cursor=abc123".to_string())
        );
    }

    #[test]
    fn test_cursor_paginated_response_without_next() {
        let data = vec!["a", "b", "c"];
        let response: CursorPaginatedResponse<&str> =
            CursorPaginatedResponse::new(data, 50, None, "/api/v1/books", Some("current_cursor"));

        assert!(!response.has_more);
        assert!(response.next_cursor.is_none());
        assert!(response.links.next.is_none());
        assert!(response.links.self_link.contains("cursor=current_cursor"));
    }

    #[test]
    fn test_cursor_paginated_response_with_params() {
        let data = vec!["a", "b", "c"];
        let additional_params = vec![
            ("library_id".to_string(), "lib-123".to_string()),
            ("sort".to_string(), "name,asc".to_string()),
        ];
        let response = CursorPaginatedResponse::with_params(
            data,
            50,
            Some("next123".to_string()),
            "/api/v1/series",
            None,
            &additional_params,
        );

        assert!(response.links.self_link.contains("library_id=lib-123"));
        assert!(response.links.self_link.contains("sort=name%2Casc"));
        assert!(response.links.first.contains("library_id=lib-123"));
        assert!(response
            .links
            .next
            .as_ref()
            .unwrap()
            .contains("cursor=next123"));
    }
}
