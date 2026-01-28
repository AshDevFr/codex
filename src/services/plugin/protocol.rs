//! JSON-RPC Protocol Types for Plugin Communication
//!
//! This module defines the JSON-RPC 2.0 protocol types for communication with plugins,
//! including request/response structures, manifest schema, and metadata types.
//!
//! Note: Many types in this module are part of the plugin protocol specification and
//! are designed for serialization/deserialization. They may not all be used internally
//! yet, but form the complete API contract for plugin communication.

// Allow dead code for protocol types that are part of the API contract but not yet used internally.
// These types are essential for the complete plugin protocol specification.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC protocol version
pub const JSONRPC_VERSION: &str = "2.0";

/// Plugin protocol version
pub const PROTOCOL_VERSION: &str = "1.0";

// =============================================================================
// JSON-RPC Base Types
// =============================================================================

/// JSON-RPC request identifier
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    String(String),
}

impl From<i64> for RequestId {
    fn from(id: i64) -> Self {
        RequestId::Number(id)
    }
}

impl From<String> for RequestId {
    fn from(id: String) -> Self {
        RequestId::String(id)
    }
}

impl From<&str> for RequestId {
    fn from(id: &str) -> Self {
        RequestId::String(id.to_string())
    }
}

/// JSON-RPC request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    /// Create a new JSON-RPC request
    pub fn new(id: impl Into<RequestId>, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: id.into(),
            method: method.into(),
            params,
        }
    }

    /// Create a request without parameters
    pub fn without_params(id: impl Into<RequestId>, method: impl Into<String>) -> Self {
        Self::new(id, method, None)
    }
}

/// JSON-RPC response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RequestId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// Create a successful response
    pub fn success(id: RequestId, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(id),
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: Option<RequestId>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }

    /// Check if the response is an error
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

/// JSON-RPC error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(code: i32, message: impl Into<String>, data: Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }
}

// =============================================================================
// Standard JSON-RPC Error Codes
// =============================================================================

/// Standard JSON-RPC error codes
pub mod error_codes {
    /// Invalid JSON was received
    pub const PARSE_ERROR: i32 = -32700;
    /// The JSON sent is not a valid Request object
    pub const INVALID_REQUEST: i32 = -32600;
    /// The method does not exist / is not available
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// Invalid method parameters
    pub const INVALID_PARAMS: i32 = -32602;
    /// Internal JSON-RPC error
    pub const INTERNAL_ERROR: i32 = -32603;

    // Plugin-specific error codes (-32000 to -32099)
    // These MUST match the TypeScript SDK (@codex/plugin-sdk) error codes in types/rpc.ts
    /// Rate limited by external provider
    pub const RATE_LIMITED: i32 = -32001;
    /// Resource not found
    pub const NOT_FOUND: i32 = -32002;
    /// Authentication failed with external provider
    pub const AUTH_FAILED: i32 = -32003;
    /// External API error (e.g., 400, 500 from upstream provider)
    pub const API_ERROR: i32 = -32004;
    /// Plugin configuration error
    pub const CONFIG_ERROR: i32 = -32005;
}

// =============================================================================
// Standard Method Names
// =============================================================================

/// Standard method names
pub mod methods {
    /// Initialize the plugin and get manifest
    pub const INITIALIZE: &str = "initialize";
    /// Graceful shutdown request
    pub const SHUTDOWN: &str = "shutdown";
    /// Health check ping
    pub const PING: &str = "ping";

    // Series metadata methods (scoped by content type)
    /// Search for series metadata
    pub const METADATA_SERIES_SEARCH: &str = "metadata/series/search";
    /// Get full series metadata by external ID
    pub const METADATA_SERIES_GET: &str = "metadata/series/get";
    /// Find best match for a series
    pub const METADATA_SERIES_MATCH: &str = "metadata/series/match";

    // Book metadata methods (future)
    // pub const METADATA_BOOK_SEARCH: &str = "metadata/book/search";
    // pub const METADATA_BOOK_GET: &str = "metadata/book/get";
    // pub const METADATA_BOOK_MATCH: &str = "metadata/book/match";
}

// =============================================================================
// Plugin Manifest Types
// =============================================================================

/// Plugin manifest returned on initialize
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    /// Unique identifier (e.g., "mangaupdates")
    pub name: String,
    /// Display name for UI (e.g., "MangaUpdates")
    pub display_name: String,
    /// Semantic version (e.g., "1.0.0")
    pub version: String,
    /// Description of the plugin
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Plugin author
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Plugin homepage URL
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,

    /// Protocol version this plugin implements
    pub protocol_version: String,

    /// Plugin capabilities
    pub capabilities: PluginCapabilities,

    /// Required credentials for this plugin
    #[serde(default)]
    pub required_credentials: Vec<CredentialField>,

    /// JSON Schema for plugin-specific configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_schema: Option<Value>,
}

/// Content types that a metadata provider can support
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum MetadataContentType {
    /// Series metadata (manga, comics, etc.)
    Series,
    // TODO: Add Book variant when book metadata is implemented
    // /// Book metadata (individual books, ebooks)
    // Book,
}

/// Plugin capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCapabilities {
    /// Content types this plugin can provide metadata for
    /// e.g., ["series"] or ["series", "book"]
    #[serde(default)]
    pub metadata_provider: Vec<MetadataContentType>,
    /// Can sync user reading progress (v2)
    #[serde(default)]
    pub user_sync_provider: bool,
}

impl PluginCapabilities {
    /// Check if the plugin can provide series metadata
    pub fn can_provide_series_metadata(&self) -> bool {
        self.metadata_provider
            .contains(&MetadataContentType::Series)
    }

    // TODO: Uncomment when book metadata is implemented
    // /// Check if the plugin can provide book metadata
    // pub fn can_provide_book_metadata(&self) -> bool {
    //     self.metadata_provider.contains(&MetadataContentType::Book)
    // }
}

/// Credential field definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialField {
    /// Credential key (e.g., "api_key")
    pub key: String,
    /// Display label (e.g., "API Key")
    pub label: String,
    /// Description for the user
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether this credential is required
    #[serde(default)]
    pub required: bool,
    /// Whether to mask the value in UI
    #[serde(default)]
    pub sensitive: bool,
    /// Input type for UI
    #[serde(default)]
    pub credential_type: CredentialType,
}

/// Credential input type
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CredentialType {
    #[default]
    String,
    Password,
    OAuth,
}

// =============================================================================
// Plugin Scopes (Server-Side)
// =============================================================================

/// Plugin scope defining where it can be invoked (server-side only).
///
/// Note: Scopes are determined by the server based on plugin capabilities,
/// not declared in the plugin manifest. This enum is used internally by Codex
/// to control where plugins can be invoked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginScope {
    /// Series detail page dropdown (search + auto-match)
    #[serde(rename = "series:detail")]
    SeriesDetail,
    /// Series list bulk actions (auto-match only)
    #[serde(rename = "series:bulk")]
    SeriesBulk,
    /// Library dropdown action (auto-match all series)
    #[serde(rename = "library:detail")]
    LibraryDetail,
    /// Post-analysis hook (auto-match if forced/changed)
    #[serde(rename = "library:scan")]
    LibraryScan,
}

impl PluginScope {
    /// Get scopes available for series metadata providers
    pub fn series_scopes() -> Vec<Self> {
        vec![
            Self::SeriesDetail,
            Self::SeriesBulk,
            Self::LibraryDetail,
            Self::LibraryScan,
        ]
    }
}

// =============================================================================
// Metadata Types
// =============================================================================

/// Parameters for metadata/search
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSearchParams {
    /// Search query
    pub query: String,
    /// Maximum number of results
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Pagination cursor
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Response from metadata/search
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSearchResponse {
    /// Search results
    pub results: Vec<SearchResult>,
    /// Cursor for next page
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Individual search result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    /// External ID from the provider
    pub external_id: String,
    /// Primary title
    pub title: String,
    /// Alternative titles
    #[serde(default)]
    pub alternate_titles: Vec<String>,
    /// Year of publication/release
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    /// Cover image URL
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    /// Relevance score (0.0-1.0). Optional - if not provided, result order is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relevance_score: Option<f64>,
    /// Preview data for displaying in results list
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<SearchResultPreview>,
}

/// Preview data shown in search results
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultPreview {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Parameters for metadata/get
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataGetParams {
    /// External ID to fetch
    pub external_id: String,
}

/// Parameters for metadata/match
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataMatchParams {
    /// Title to match
    pub title: String,
    /// Year hint for matching
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    /// Author hint for matching
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
}

/// Full series metadata from a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSeriesMetadata {
    /// External ID from the provider
    pub external_id: String,
    /// URL to the series on the provider's website
    pub external_url: String,

    // Core fields (all optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub alternate_titles: Vec<AlternateTitle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<SeriesStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,

    // Extended metadata
    /// Expected total number of books in the series
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_book_count: Option<i32>,
    /// BCP47 language code (e.g., "en", "ja", "ko")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Age rating (e.g., 0, 13, 16, 18)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_rating: Option<i32>,
    /// Reading direction: "ltr", "rtl", or "ttb"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reading_direction: Option<String>,

    // Taxonomy
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,

    // Credits
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub artists: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,

    // Media
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner_url: Option<String>,

    // Rating
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<ExternalRating>,
    /// Multiple external ratings from different sources (e.g., AniList, MAL)
    #[serde(default)]
    pub external_ratings: Vec<ExternalRating>,

    // External links
    #[serde(default)]
    pub external_links: Vec<ExternalLink>,
}

/// Full book metadata from a provider (for future use)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginBookMetadata {
    /// External ID from the provider
    pub external_id: String,
    /// URL to the book on the provider's website
    pub external_url: String,

    // Core fields (all optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub alternate_titles: Vec<AlternateTitle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,

    // Book-specific
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapter: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_count: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isbn: Option<String>,

    // Taxonomy
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,

    // Credits
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub artists: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,

    // Media
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,

    // Rating
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<ExternalRating>,
    /// Multiple external ratings from different sources
    #[serde(default)]
    pub external_ratings: Vec<ExternalRating>,

    // External links
    #[serde(default)]
    pub external_links: Vec<ExternalLink>,
}

/// Alternate title with language info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlternateTitle {
    pub title: String,
    /// ISO 639-1 language code (e.g., "en", "ja")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Title type (e.g., "romaji", "native", "english")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_type: Option<String>,
}

// Re-export SeriesStatus from db entities - this is the canonical source
pub use crate::db::entities::SeriesStatus;

/// External rating from provider
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalRating {
    /// Normalized score (0-100)
    pub score: f64,
    /// Number of votes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vote_count: Option<i32>,
    /// Source name (e.g., "mangaupdates")
    pub source: String,
}

/// External link to other sites
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalLink {
    pub url: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_type: Option<ExternalLinkType>,
}

/// Type of external link
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExternalLinkType {
    Provider,
    Official,
    Social,
    Purchase,
    Read,
    Other,
}

// =============================================================================
// Initialize Response
// =============================================================================

/// Parameters for initialize (usually empty or with config)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    /// Plugin configuration from Codex
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
    /// Credentials passed via init message (alternative to env vars)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<Value>,
}

// =============================================================================
// Rate Limit Error Data
// =============================================================================

/// Data included with rate limit errors
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitErrorData {
    pub retry_after_seconds: u64,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_jsonrpc_request_serialization() {
        let request = JsonRpcRequest::new(
            1i64,
            "metadata/series/search",
            Some(json!({"query": "test"})),
        );
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"metadata/series/search\""));
        assert!(json.contains("\"id\":1"));
    }

    #[test]
    fn test_jsonrpc_request_without_params() {
        let request = JsonRpcRequest::without_params(1i64, "ping");
        let json = serde_json::to_string(&request).unwrap();
        assert!(!json.contains("params"));
    }

    #[test]
    fn test_jsonrpc_response_success() {
        let response = JsonRpcResponse::success(RequestId::Number(1), json!({"status": "ok"}));
        assert!(!response.is_error());
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_jsonrpc_response_error() {
        let response = JsonRpcResponse::error(
            Some(RequestId::Number(1)),
            JsonRpcError::new(error_codes::NOT_FOUND, "Resource not found"),
        );
        assert!(response.is_error());
        assert!(response.result.is_none());
        assert!(response.error.is_some());
    }

    #[test]
    fn test_request_id_from_i64() {
        let id: RequestId = 42i64.into();
        assert_eq!(id, RequestId::Number(42));
    }

    #[test]
    fn test_request_id_from_string() {
        let id: RequestId = "abc-123".into();
        assert_eq!(id, RequestId::String("abc-123".to_string()));
    }

    #[test]
    fn test_plugin_manifest_deserialization() {
        let json = json!({
            "name": "test-plugin",
            "displayName": "Test Plugin",
            "version": "1.0.0",
            "protocolVersion": "1.0",
            "capabilities": {
                "metadataProvider": ["series"]
            }
        });

        let manifest: PluginManifest = serde_json::from_value(json).unwrap();
        assert_eq!(manifest.name, "test-plugin");
        assert_eq!(manifest.display_name, "Test Plugin");
        assert!(manifest.capabilities.can_provide_series_metadata());
    }

    // TODO: Re-enable when book metadata is implemented
    // #[test]
    // fn test_plugin_manifest_with_multiple_content_types() {
    //     let json = json!({
    //         "name": "multi-provider",
    //         "displayName": "Multi Provider",
    //         "version": "1.0.0",
    //         "protocolVersion": "1.0",
    //         "capabilities": {
    //             "metadataProvider": ["series", "book"]
    //         }
    //     });
    //
    //     let manifest: PluginManifest = serde_json::from_value(json).unwrap();
    //     assert!(manifest.capabilities.can_provide_series_metadata());
    //     assert!(manifest.capabilities.can_provide_book_metadata());
    // }

    #[test]
    fn test_plugin_manifest_empty_capabilities() {
        let json = json!({
            "name": "empty-plugin",
            "displayName": "Empty Plugin",
            "version": "1.0.0",
            "protocolVersion": "1.0",
            "capabilities": {}
        });

        let manifest: PluginManifest = serde_json::from_value(json).unwrap();
        assert!(!manifest.capabilities.can_provide_series_metadata());
    }

    #[test]
    fn test_metadata_search_params() {
        let params = MetadataSearchParams {
            query: "One Piece".to_string(),
            limit: Some(10),
            cursor: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["query"], "One Piece");
        assert_eq!(json["limit"], 10);
        assert!(!json.as_object().unwrap().contains_key("cursor"));
    }

    #[test]
    fn test_search_result_deserialization() {
        let json = json!({
            "externalId": "12345",
            "title": "One Piece",
            "alternateTitles": ["ワンピース"],
            "year": 1997,
            "relevanceScore": 0.98,
            "preview": {
                "status": "ongoing",
                "genres": ["Action", "Adventure"]
            }
        });

        let result: SearchResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.external_id, "12345");
        assert_eq!(result.title, "One Piece");
        assert_eq!(result.year, Some(1997));
        assert_eq!(result.relevance_score, Some(0.98));
        assert!(result.preview.is_some());
    }

    #[test]
    fn test_series_metadata_full() {
        let metadata = PluginSeriesMetadata {
            external_id: "12345".to_string(),
            external_url: "https://example.com/series/12345".to_string(),
            title: Some("One Piece".to_string()),
            alternate_titles: vec![AlternateTitle {
                title: "ワンピース".to_string(),
                language: Some("ja".to_string()),
                title_type: Some("native".to_string()),
            }],
            summary: Some("A pirate adventure".to_string()),
            status: Some(SeriesStatus::Ongoing),
            year: Some(1997),
            total_book_count: Some(100),
            language: Some("ja".to_string()),
            age_rating: Some(13),
            reading_direction: Some("rtl".to_string()),
            genres: vec!["Action".to_string(), "Adventure".to_string()],
            tags: vec!["pirates".to_string()],
            authors: vec!["Oda, Eiichiro".to_string()],
            artists: vec![],
            publisher: Some("Shueisha".to_string()),
            cover_url: Some("https://example.com/cover.jpg".to_string()),
            banner_url: None,
            rating: Some(ExternalRating {
                score: 91.0,
                vote_count: Some(50000),
                source: "example".to_string(),
            }),
            external_ratings: vec![],
            external_links: vec![],
        };

        let json = serde_json::to_value(&metadata).unwrap();
        assert_eq!(json["externalId"], "12345");
        assert_eq!(json["status"], "ongoing");
    }

    #[test]
    fn test_credential_field() {
        let field = CredentialField {
            key: "api_key".to_string(),
            label: "API Key".to_string(),
            description: Some("Get your API key from...".to_string()),
            required: true,
            sensitive: true,
            credential_type: CredentialType::Password,
        };

        let json = serde_json::to_value(&field).unwrap();
        assert_eq!(json["key"], "api_key");
        assert_eq!(json["credentialType"], "password");
        assert!(json["sensitive"].as_bool().unwrap());
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(error_codes::PARSE_ERROR, -32700);
        assert_eq!(error_codes::RATE_LIMITED, -32001);
        assert_eq!(error_codes::NOT_FOUND, -32002);
        assert_eq!(error_codes::AUTH_FAILED, -32003);
        assert_eq!(error_codes::API_ERROR, -32004);
        assert_eq!(error_codes::CONFIG_ERROR, -32005);
    }

    #[test]
    fn test_jsonrpc_error_with_data() {
        let error = JsonRpcError::with_data(
            error_codes::RATE_LIMITED,
            "Rate limited",
            json!({"retryAfterSeconds": 60}),
        );
        assert_eq!(error.code, -32001);
        assert_eq!(error.message, "Rate limited");
        assert!(error.data.is_some());
    }
}
