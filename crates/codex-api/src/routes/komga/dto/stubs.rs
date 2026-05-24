//! Stub DTOs for unimplemented Komga endpoints
//!
//! These DTOs are used for endpoints that Komic expects but Codex doesn't fully support.
//! They return empty results to prevent 404 errors in the client.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Minimal collection DTO (stub)
///
/// Komga collections are user-created groupings of series.
/// Codex doesn't support this feature, so we return empty results.
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KomgaCollectionDto {
    /// Collection unique identifier
    pub id: String,
    /// Collection name
    pub name: String,
    /// Whether the collection is ordered
    pub ordered: bool,
    /// Series IDs in the collection
    pub series_ids: Vec<String>,
    /// Created timestamp (ISO 8601)
    pub created_date: String,
    /// Last modified timestamp (ISO 8601)
    pub last_modified_date: String,
    /// Whether this collection is filtered from the user's view
    pub filtered: bool,
}

/// Minimal read list DTO (stub)
///
/// Komga read lists are user-created lists of books to read.
/// Codex doesn't support this feature, so we return empty results.
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KomgaReadListDto {
    /// Read list unique identifier
    pub id: String,
    /// Read list name
    pub name: String,
    /// Read list summary/description
    pub summary: String,
    /// Whether the read list is ordered
    pub ordered: bool,
    /// Book IDs in the read list
    pub book_ids: Vec<String>,
    /// Created timestamp (ISO 8601)
    pub created_date: String,
    /// Last modified timestamp (ISO 8601)
    pub last_modified_date: String,
    /// Whether this read list is filtered from the user's view
    pub filtered: bool,
}

/// Query parameters for stub pagination endpoints
#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
pub struct StubPaginationQuery {
    /// Page number (0-indexed)
    #[serde(default)]
    pub page: i32,
    /// Page size
    #[serde(default = "default_stub_page_size")]
    pub size: i32,
    /// Search term (ignored - accepted for Komic compatibility)
    #[serde(default)]
    pub search: Option<String>,
    /// Library ID filter (ignored - accepted for Komic compatibility)
    #[serde(default)]
    pub library_id: Option<String>,
    /// Unpaged flag (ignored - accepted for Komic compatibility)
    #[serde(default)]
    pub unpaged: Option<bool>,
}

fn default_stub_page_size() -> i32 {
    20
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_dto_serialization() {
        let collection = KomgaCollectionDto {
            id: "col-1".to_string(),
            name: "My Collection".to_string(),
            ordered: true,
            series_ids: vec!["s1".to_string(), "s2".to_string()],
            created_date: "2024-01-01T00:00:00Z".to_string(),
            last_modified_date: "2024-01-15T00:00:00Z".to_string(),
            filtered: false,
        };

        let json = serde_json::to_string(&collection).unwrap();
        assert!(json.contains("\"id\":\"col-1\""));
        assert!(json.contains("\"name\":\"My Collection\""));
        assert!(json.contains("\"seriesIds\""));
        assert!(json.contains("\"createdDate\""));
        assert!(json.contains("\"lastModifiedDate\""));
    }

    #[test]
    fn test_read_list_dto_serialization() {
        let readlist = KomgaReadListDto {
            id: "rl-1".to_string(),
            name: "To Read".to_string(),
            summary: "Books to read later".to_string(),
            ordered: false,
            book_ids: vec!["b1".to_string()],
            created_date: "2024-01-01T00:00:00Z".to_string(),
            last_modified_date: "2024-01-15T00:00:00Z".to_string(),
            filtered: false,
        };

        let json = serde_json::to_string(&readlist).unwrap();
        assert!(json.contains("\"id\":\"rl-1\""));
        assert!(json.contains("\"name\":\"To Read\""));
        assert!(json.contains("\"bookIds\""));
    }

    #[test]
    fn test_stub_pagination_query_defaults() {
        let query: StubPaginationQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(query.page, 0);
        assert_eq!(query.size, 20);
        assert!(query.search.is_none());
        assert!(query.library_id.is_none());
    }
}
