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
    // These MUST match the TypeScript SDK (@ashdev/codex-plugin-sdk) error codes in types/rpc.ts
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

    // Book metadata methods
    /// Search for book metadata (supports ISBN or title/author query)
    pub const METADATA_BOOK_SEARCH: &str = "metadata/book/search";
    /// Get full book metadata by external ID
    pub const METADATA_BOOK_GET: &str = "metadata/book/get";
    /// Find best match for a book (ISBN first, then title fallback)
    pub const METADATA_BOOK_MATCH: &str = "metadata/book/match";

    // Storage methods (user plugin data)
    /// Get a value by key from plugin storage
    pub const STORAGE_GET: &str = "storage/get";
    /// Set a value by key in plugin storage (upsert)
    pub const STORAGE_SET: &str = "storage/set";
    /// Delete a value by key from plugin storage
    pub const STORAGE_DELETE: &str = "storage/delete";
    /// List all keys in plugin storage
    pub const STORAGE_LIST: &str = "storage/list";
    /// Clear all data from plugin storage
    pub const STORAGE_CLEAR: &str = "storage/clear";
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

    /// Plugin type: "system" (admin-only metadata) or "user" (per-user integrations)
    #[serde(default)]
    pub plugin_type: PluginManifestType,

    /// OAuth 2.0 configuration for user plugins that require external service authentication
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth: Option<OAuthConfig>,

    /// User-facing description shown when enabling the plugin
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_description: Option<String>,

    /// Setup instructions/help text for the OAuth flow
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_instructions: Option<String>,
}

/// Content types that a metadata provider can support
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum MetadataContentType {
    /// Series metadata (manga, comics, etc.)
    Series,
    /// Book metadata (individual books, ebooks, novels)
    Book,
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
    /// Can provide personalized recommendations (v2)
    #[serde(default)]
    pub recommendation_provider: bool,
}

impl PluginCapabilities {
    /// Check if the plugin can provide series metadata
    pub fn can_provide_series_metadata(&self) -> bool {
        self.metadata_provider
            .contains(&MetadataContentType::Series)
    }

    /// Check if the plugin can provide book metadata
    pub fn can_provide_book_metadata(&self) -> bool {
        self.metadata_provider.contains(&MetadataContentType::Book)
    }
}

/// Plugin manifest type (declared by the plugin in its manifest)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginManifestType {
    /// System plugin: admin-configured, operates on shared library metadata
    #[default]
    System,
    /// User plugin: per-user integrations (sync, recommendations)
    User,
}

/// OAuth 2.0 configuration for user plugins
///
/// Plugins declare their OAuth requirements in the manifest. Codex handles
/// the OAuth flow (authorization URL generation, code exchange, token storage)
/// so plugins never directly interact with the OAuth provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfig {
    /// OAuth 2.0 authorization endpoint URL
    pub authorization_url: String,
    /// OAuth 2.0 token endpoint URL
    pub token_url: String,
    /// Required OAuth scopes
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Whether to use PKCE (Proof Key for Code Exchange)
    /// Recommended for public clients; defaults to true
    #[serde(default = "default_true")]
    pub pkce: bool,
    /// Optional user info endpoint URL (to fetch external identity after auth)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_info_url: Option<String>,
    /// OAuth client ID (can be overridden by admin in plugin config)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
}

fn default_true() -> bool {
    true
}

impl OAuthConfig {
    /// Validate that the OAuth config has all required fields
    pub fn validate(&self) -> Result<(), String> {
        if self.authorization_url.is_empty() {
            return Err("OAuth authorization_url is required".to_string());
        }
        if self.token_url.is_empty() {
            return Err("OAuth token_url is required".to_string());
        }
        // Validate URLs start with https:// (or http:// for local dev)
        if !self.authorization_url.starts_with("https://")
            && !self.authorization_url.starts_with("http://")
        {
            return Err(format!(
                "Invalid OAuth authorization_url (must start with http:// or https://): {}",
                self.authorization_url
            ));
        }
        if !self.token_url.starts_with("https://") && !self.token_url.starts_with("http://") {
            return Err(format!(
                "Invalid OAuth token_url (must start with http:// or https://): {}",
                self.token_url
            ));
        }
        if let Some(ref user_info_url) = self.user_info_url
            && !user_info_url.starts_with("https://")
            && !user_info_url.starts_with("http://")
        {
            return Err(format!(
                "Invalid OAuth user_info_url (must start with http:// or https://): {}",
                user_info_url
            ));
        }
        Ok(())
    }
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
    // =========================================================================
    // Series Scopes
    // =========================================================================
    /// Series detail page dropdown (search + auto-match)
    #[serde(rename = "series:detail")]
    SeriesDetail,
    /// Series list bulk actions (auto-match only)
    #[serde(rename = "series:bulk")]
    SeriesBulk,

    // =========================================================================
    // Book Scopes
    // =========================================================================
    /// Book detail page dropdown (search + auto-match)
    #[serde(rename = "book:detail")]
    BookDetail,
    /// Book list bulk actions (auto-match only)
    #[serde(rename = "book:bulk")]
    BookBulk,

    // =========================================================================
    // Library Scopes
    // =========================================================================
    /// Library dropdown action (auto-match all series/books)
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

    /// Get scopes available for book metadata providers
    pub fn book_scopes() -> Vec<Self> {
        vec![
            Self::BookDetail,
            Self::BookBulk,
            Self::LibraryDetail,
            Self::LibraryScan,
        ]
    }

    /// Get all scopes (series + book + library)
    pub fn all_scopes() -> Vec<Self> {
        vec![
            Self::SeriesDetail,
            Self::SeriesBulk,
            Self::BookDetail,
            Self::BookBulk,
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
    /// Number of books in the series (if known by the provider)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_count: Option<i32>,
    /// Author names (for book search results)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
}

/// Parameters for metadata/get
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataGetParams {
    /// External ID to fetch
    pub external_id: String,
}

/// Parameters for metadata/match (series)
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

// =============================================================================
// Book Metadata Types
// =============================================================================

/// Parameters for metadata/book/search
///
/// Supports both ISBN lookup and title/author search:
/// - If `isbn` is provided, direct ISBN lookup is attempted first (faster, more accurate)
/// - If only `query` is provided, title/author search is used
/// - If both are provided, ISBN is tried first with query as fallback
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookSearchParams {
    /// ISBN-10 or ISBN-13 (if provided, takes priority over query)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isbn: Option<String>,
    /// Search query (title, author, or combined) - used if no ISBN
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Optional: filter by author name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Optional: filter by publication year
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    /// Maximum number of results
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Pagination cursor
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

impl BookSearchParams {
    /// Check if this is an ISBN search
    pub fn is_isbn_search(&self) -> bool {
        self.isbn.is_some()
    }

    /// Check if this is a query-based search
    pub fn is_query_search(&self) -> bool {
        self.query.is_some()
    }

    /// Check if the params are valid (at least one of isbn or query must be present)
    pub fn is_valid(&self) -> bool {
        self.isbn.is_some() || self.query.is_some()
    }
}

/// Parameters for metadata/book/match (auto-matching)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookMatchParams {
    /// Book title
    pub title: String,
    /// Authors (if known)
    #[serde(default)]
    pub authors: Vec<String>,
    /// ISBN (if available - will be tried first)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isbn: Option<String>,
    /// Publication year (if known)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    /// Publisher (if known)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
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

/// Full book metadata from a provider
///
/// This structure contains all metadata fields that plugins can provide for books.
/// It supports both traditional books (novels, ebooks) and comics/manga volumes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginBookMetadata {
    /// External ID from the provider
    pub external_id: String,
    /// URL to the book on the provider's website
    pub external_url: String,

    // =========================================================================
    // Core Fields (all optional)
    // =========================================================================
    /// Primary title
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Subtitle (e.g., "A Novel")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    /// Alternative titles with language info
    #[serde(default)]
    pub alternate_titles: Vec<AlternateTitle>,
    /// Full description/summary
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Book type (comic, manga, novel, etc.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_type: Option<String>,

    // =========================================================================
    // Book-Specific Fields
    // =========================================================================
    /// Volume number in series
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
    /// Chapter number (for single-chapter releases)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapter: Option<f64>,
    /// Page count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_count: Option<i32>,
    /// Release date (ISO 8601 format)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    /// Publication year
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,

    // =========================================================================
    // ISBN and Identifiers
    // =========================================================================
    /// Primary ISBN (ISBN-13 preferred)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isbn: Option<String>,
    /// All ISBNs (ISBN-10 and ISBN-13)
    #[serde(default)]
    pub isbns: Vec<String>,

    // =========================================================================
    // Translation/Edition Info
    // =========================================================================
    /// Edition information (e.g., "First Edition", "Revised")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edition: Option<String>,
    /// Original title (for translations)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_title: Option<String>,
    /// Original publication year
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_year: Option<i32>,
    /// Translator name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translator: Option<String>,
    /// BCP47 language code (e.g., "en", "ja", "ko")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,

    // =========================================================================
    // Series Position
    // =========================================================================
    /// Position in series (e.g., 1.0, 1.5 for specials)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_position: Option<f64>,
    /// Total number of books in series (if known)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_total: Option<i32>,

    // =========================================================================
    // Taxonomy
    // =========================================================================
    /// Genres (e.g., "Science Fiction", "Romance")
    #[serde(default)]
    pub genres: Vec<String>,
    /// Tags/themes (e.g., "Time Travel", "Space Exploration")
    #[serde(default)]
    pub tags: Vec<String>,
    /// Subjects/topics (library classification)
    #[serde(default)]
    pub subjects: Vec<String>,

    // =========================================================================
    // Credits
    // =========================================================================
    /// Structured authors with roles
    #[serde(default)]
    pub authors: Vec<BookAuthor>,
    /// Artists (for comics/manga)
    #[serde(default)]
    pub artists: Vec<String>,
    /// Publisher name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,

    // =========================================================================
    // Media
    // =========================================================================
    /// Primary cover URL (for backwards compatibility)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    /// Multiple covers with different sizes/sources
    #[serde(default)]
    pub covers: Vec<BookCover>,

    // =========================================================================
    // Rating
    // =========================================================================
    /// Primary external rating
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<ExternalRating>,
    /// Multiple external ratings from different sources
    #[serde(default)]
    pub external_ratings: Vec<ExternalRating>,

    // =========================================================================
    // Awards
    // =========================================================================
    /// Awards received
    #[serde(default)]
    pub awards: Vec<BookAward>,

    // =========================================================================
    // External Links
    // =========================================================================
    /// Links to other sites
    #[serde(default)]
    pub external_links: Vec<ExternalLink>,
}

/// Structured author with role information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookAuthor {
    /// Author's display name
    pub name: String,
    /// Author's role
    #[serde(default)]
    pub role: BookAuthorRole,
    /// Author's name in sort order (e.g., "Doe, Jane")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_name: Option<String>,
}

/// Author role in a book
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BookAuthorRole {
    #[default]
    Author,
    CoAuthor,
    Editor,
    Translator,
    Illustrator,
    Contributor,
}

/// Book cover with size and source information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookCover {
    /// URL to download the cover image
    pub url: String,
    /// Image width in pixels (if known)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<i32>,
    /// Image height in pixels (if known)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<i32>,
    /// Size hint for cover
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<BookCoverSize>,
}

/// Cover size hint
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BookCoverSize {
    Small,
    Medium,
    Large,
}

/// Book award information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookAward {
    /// Award name (e.g., "Hugo Award")
    pub name: String,
    /// Year the award was given
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    /// Award category (e.g., "Best Novel")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Whether the book won (true) or was nominated (false)
    #[serde(default)]
    pub won: bool,
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

    #[test]
    fn test_plugin_manifest_with_multiple_content_types() {
        let json = json!({
            "name": "multi-provider",
            "displayName": "Multi Provider",
            "version": "1.0.0",
            "protocolVersion": "1.0",
            "capabilities": {
                "metadataProvider": ["series", "book"]
            }
        });

        let manifest: PluginManifest = serde_json::from_value(json).unwrap();
        assert!(manifest.capabilities.can_provide_series_metadata());
        assert!(manifest.capabilities.can_provide_book_metadata());
    }

    #[test]
    fn test_plugin_manifest_book_only() {
        let json = json!({
            "name": "book-provider",
            "displayName": "Book Provider",
            "version": "1.0.0",
            "protocolVersion": "1.0",
            "capabilities": {
                "metadataProvider": ["book"]
            }
        });

        let manifest: PluginManifest = serde_json::from_value(json).unwrap();
        assert!(!manifest.capabilities.can_provide_series_metadata());
        assert!(manifest.capabilities.can_provide_book_metadata());
    }

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

    // =========================================================================
    // Book Metadata Tests
    // =========================================================================

    #[test]
    fn test_book_search_params_isbn() {
        let params = BookSearchParams {
            isbn: Some("978-0-306-40615-7".to_string()),
            query: None,
            author: None,
            year: None,
            limit: Some(10),
            cursor: None,
        };
        assert!(params.is_isbn_search());
        assert!(!params.is_query_search());
        assert!(params.is_valid());
    }

    #[test]
    fn test_book_search_params_query() {
        let params = BookSearchParams {
            isbn: None,
            query: Some("The Hobbit".to_string()),
            author: Some("Tolkien".to_string()),
            year: Some(1937),
            limit: None,
            cursor: None,
        };
        assert!(!params.is_isbn_search());
        assert!(params.is_query_search());
        assert!(params.is_valid());
    }

    #[test]
    fn test_book_search_params_invalid() {
        let params = BookSearchParams {
            isbn: None,
            query: None,
            author: None,
            year: None,
            limit: None,
            cursor: None,
        };
        assert!(!params.is_valid());
    }

    #[test]
    fn test_book_match_params() {
        let params = BookMatchParams {
            title: "The Hobbit".to_string(),
            authors: vec!["J.R.R. Tolkien".to_string()],
            isbn: Some("978-0-547-92822-7".to_string()),
            year: Some(1937),
            publisher: Some("Houghton Mifflin".to_string()),
        };

        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["title"], "The Hobbit");
        assert_eq!(json["authors"][0], "J.R.R. Tolkien");
        assert_eq!(json["isbn"], "978-0-547-92822-7");
    }

    #[test]
    fn test_book_author_serialization() {
        let author = BookAuthor {
            name: "Jane Doe".to_string(),
            role: BookAuthorRole::Author,
            sort_name: Some("Doe, Jane".to_string()),
        };

        let json = serde_json::to_value(&author).unwrap();
        assert_eq!(json["name"], "Jane Doe");
        assert_eq!(json["role"], "author");
        assert_eq!(json["sortName"], "Doe, Jane");
    }

    #[test]
    fn test_book_author_role_default() {
        let author: BookAuthor = serde_json::from_value(json!({
            "name": "John Smith"
        }))
        .unwrap();

        assert_eq!(author.name, "John Smith");
        assert_eq!(author.role, BookAuthorRole::Author);
        assert!(author.sort_name.is_none());
    }

    #[test]
    fn test_book_cover_serialization() {
        let cover = BookCover {
            url: "https://example.com/cover.jpg".to_string(),
            width: Some(300),
            height: Some(450),
            size: Some(BookCoverSize::Medium),
        };

        let json = serde_json::to_value(&cover).unwrap();
        assert_eq!(json["url"], "https://example.com/cover.jpg");
        assert_eq!(json["width"], 300);
        assert_eq!(json["height"], 450);
        assert_eq!(json["size"], "medium");
    }

    #[test]
    fn test_book_award_serialization() {
        let award = BookAward {
            name: "Hugo Award".to_string(),
            year: Some(2024),
            category: Some("Best Novel".to_string()),
            won: true,
        };

        let json = serde_json::to_value(&award).unwrap();
        assert_eq!(json["name"], "Hugo Award");
        assert_eq!(json["year"], 2024);
        assert_eq!(json["category"], "Best Novel");
        assert!(json["won"].as_bool().unwrap());
    }

    #[test]
    fn test_book_metadata_full() {
        let metadata = PluginBookMetadata {
            external_id: "12345".to_string(),
            external_url: "https://example.com/book/12345".to_string(),
            title: Some("The Hobbit".to_string()),
            subtitle: Some("or There and Back Again".to_string()),
            alternate_titles: vec![],
            summary: Some("A fantasy novel about a hobbit's journey".to_string()),
            book_type: Some("novel".to_string()),
            volume: None,
            chapter: None,
            page_count: Some(310),
            release_date: Some("1937-09-21".to_string()),
            year: Some(1937),
            isbn: Some("978-0-547-92822-7".to_string()),
            isbns: vec!["978-0-547-92822-7".to_string()],
            edition: Some("75th Anniversary Edition".to_string()),
            original_title: None,
            original_year: None,
            translator: None,
            language: Some("en".to_string()),
            series_position: Some(0.0),
            series_total: Some(4),
            genres: vec!["Fantasy".to_string()],
            tags: vec!["adventure".to_string(), "dragons".to_string()],
            subjects: vec!["Middle-earth (Imaginary place)".to_string()],
            authors: vec![BookAuthor {
                name: "J.R.R. Tolkien".to_string(),
                role: BookAuthorRole::Author,
                sort_name: Some("Tolkien, J.R.R.".to_string()),
            }],
            artists: vec![],
            publisher: Some("Houghton Mifflin Harcourt".to_string()),
            cover_url: Some("https://example.com/cover.jpg".to_string()),
            covers: vec![],
            rating: Some(ExternalRating {
                score: 92.0,
                vote_count: Some(100000),
                source: "goodreads".to_string(),
            }),
            external_ratings: vec![],
            awards: vec![],
            external_links: vec![],
        };

        let json = serde_json::to_value(&metadata).unwrap();
        assert_eq!(json["externalId"], "12345");
        assert_eq!(json["title"], "The Hobbit");
        assert_eq!(json["subtitle"], "or There and Back Again");
        assert_eq!(json["bookType"], "novel");
        assert_eq!(json["year"], 1937);
        assert_eq!(json["isbn"], "978-0-547-92822-7");
        assert_eq!(json["authors"][0]["name"], "J.R.R. Tolkien");
    }

    #[test]
    fn test_book_scope_serialization() {
        let scope = PluginScope::BookDetail;
        let json = serde_json::to_value(&scope).unwrap();
        assert_eq!(json, "book:detail");

        let scope: PluginScope = serde_json::from_value(json!("book:bulk")).unwrap();
        assert_eq!(scope, PluginScope::BookBulk);
    }

    #[test]
    fn test_book_scopes() {
        let scopes = PluginScope::book_scopes();
        assert!(scopes.contains(&PluginScope::BookDetail));
        assert!(scopes.contains(&PluginScope::BookBulk));
        assert!(scopes.contains(&PluginScope::LibraryDetail));
        assert!(scopes.contains(&PluginScope::LibraryScan));
        assert!(!scopes.contains(&PluginScope::SeriesDetail));
        assert_eq!(scopes.len(), 4);
    }

    #[test]
    fn test_all_scopes() {
        let scopes = PluginScope::all_scopes();
        assert!(scopes.contains(&PluginScope::SeriesDetail));
        assert!(scopes.contains(&PluginScope::BookDetail));
        assert_eq!(scopes.len(), 6);
    }

    // =========================================================================
    // OAuth Config & User Plugin Tests
    // =========================================================================

    #[test]
    fn test_plugin_manifest_type_default() {
        let manifest_type: PluginManifestType = Default::default();
        assert_eq!(manifest_type, PluginManifestType::System);
    }

    #[test]
    fn test_plugin_manifest_type_serialization() {
        let system = PluginManifestType::System;
        let user = PluginManifestType::User;
        assert_eq!(serde_json::to_value(&system).unwrap(), json!("system"));
        assert_eq!(serde_json::to_value(&user).unwrap(), json!("user"));
    }

    #[test]
    fn test_plugin_manifest_type_deserialization() {
        let system: PluginManifestType = serde_json::from_value(json!("system")).unwrap();
        let user: PluginManifestType = serde_json::from_value(json!("user")).unwrap();
        assert_eq!(system, PluginManifestType::System);
        assert_eq!(user, PluginManifestType::User);
    }

    #[test]
    fn test_oauth_config_serialization() {
        let config = OAuthConfig {
            authorization_url: "https://anilist.co/api/v2/oauth/authorize".to_string(),
            token_url: "https://anilist.co/api/v2/oauth/token".to_string(),
            scopes: vec!["read".to_string(), "write".to_string()],
            pkce: true,
            user_info_url: Some("https://graphql.anilist.co".to_string()),
            client_id: None,
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(
            json["authorizationUrl"],
            "https://anilist.co/api/v2/oauth/authorize"
        );
        assert_eq!(json["tokenUrl"], "https://anilist.co/api/v2/oauth/token");
        assert_eq!(json["scopes"], json!(["read", "write"]));
        assert!(json["pkce"].as_bool().unwrap());
        assert_eq!(json["userInfoUrl"], "https://graphql.anilist.co");
    }

    #[test]
    fn test_oauth_config_deserialization() {
        let json = json!({
            "authorizationUrl": "https://myanimelist.net/v1/oauth2/authorize",
            "tokenUrl": "https://myanimelist.net/v1/oauth2/token",
            "scopes": ["read"],
            "pkce": true
        });

        let config: OAuthConfig = serde_json::from_value(json).unwrap();
        assert_eq!(
            config.authorization_url,
            "https://myanimelist.net/v1/oauth2/authorize"
        );
        assert_eq!(config.token_url, "https://myanimelist.net/v1/oauth2/token");
        assert_eq!(config.scopes, vec!["read"]);
        assert!(config.pkce);
        assert!(config.user_info_url.is_none());
    }

    #[test]
    fn test_oauth_config_pkce_defaults_to_true() {
        let json = json!({
            "authorizationUrl": "https://example.com/auth",
            "tokenUrl": "https://example.com/token"
        });

        let config: OAuthConfig = serde_json::from_value(json).unwrap();
        assert!(config.pkce);
    }

    #[test]
    fn test_oauth_config_validate_valid() {
        let config = OAuthConfig {
            authorization_url: "https://example.com/auth".to_string(),
            token_url: "https://example.com/token".to_string(),
            scopes: vec![],
            pkce: true,
            user_info_url: None,
            client_id: None,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_oauth_config_validate_empty_auth_url() {
        let config = OAuthConfig {
            authorization_url: "".to_string(),
            token_url: "https://example.com/token".to_string(),
            scopes: vec![],
            pkce: true,
            user_info_url: None,
            client_id: None,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_oauth_config_validate_invalid_url() {
        let config = OAuthConfig {
            authorization_url: "not-a-url".to_string(),
            token_url: "https://example.com/token".to_string(),
            scopes: vec![],
            pkce: true,
            user_info_url: None,
            client_id: None,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_oauth_config_validate_with_user_info_url() {
        let config = OAuthConfig {
            authorization_url: "https://example.com/auth".to_string(),
            token_url: "https://example.com/token".to_string(),
            scopes: vec![],
            pkce: true,
            user_info_url: Some("https://example.com/userinfo".to_string()),
            client_id: None,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_oauth_config_validate_invalid_user_info_url() {
        let config = OAuthConfig {
            authorization_url: "https://example.com/auth".to_string(),
            token_url: "https://example.com/token".to_string(),
            scopes: vec![],
            pkce: true,
            user_info_url: Some("not-a-url".to_string()),
            client_id: None,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_plugin_manifest_with_oauth_config() {
        let json = json!({
            "name": "anilist-sync",
            "displayName": "AniList Sync",
            "version": "1.0.0",
            "protocolVersion": "1.0",
            "pluginType": "user",
            "capabilities": {
                "userSyncProvider": true
            },
            "oauth": {
                "authorizationUrl": "https://anilist.co/api/v2/oauth/authorize",
                "tokenUrl": "https://anilist.co/api/v2/oauth/token",
                "scopes": [],
                "pkce": false
            },
            "userDescription": "Sync reading progress with AniList",
            "setupInstructions": "Click Connect to link your AniList account"
        });

        let manifest: PluginManifest = serde_json::from_value(json).unwrap();
        assert_eq!(manifest.name, "anilist-sync");
        assert_eq!(manifest.plugin_type, PluginManifestType::User);
        assert!(manifest.capabilities.user_sync_provider);
        assert!(!manifest.capabilities.recommendation_provider);

        let oauth = manifest.oauth.unwrap();
        assert_eq!(
            oauth.authorization_url,
            "https://anilist.co/api/v2/oauth/authorize"
        );
        assert!(!oauth.pkce);

        assert_eq!(
            manifest.user_description.unwrap(),
            "Sync reading progress with AniList"
        );
        assert!(manifest.setup_instructions.is_some());
    }

    #[test]
    fn test_plugin_manifest_defaults_to_system_type() {
        let json = json!({
            "name": "metadata-plugin",
            "displayName": "Metadata Plugin",
            "version": "1.0.0",
            "protocolVersion": "1.0",
            "capabilities": {
                "metadataProvider": ["series"]
            }
        });

        let manifest: PluginManifest = serde_json::from_value(json).unwrap();
        assert_eq!(manifest.plugin_type, PluginManifestType::System);
        assert!(manifest.oauth.is_none());
        assert!(manifest.user_description.is_none());
    }

    #[test]
    fn test_plugin_capabilities_recommendation_provider() {
        let json = json!({
            "name": "rec-engine",
            "displayName": "Recommendation Engine",
            "version": "1.0.0",
            "protocolVersion": "1.0",
            "pluginType": "user",
            "capabilities": {
                "recommendationProvider": true
            }
        });

        let manifest: PluginManifest = serde_json::from_value(json).unwrap();
        assert!(manifest.capabilities.recommendation_provider);
        assert!(!manifest.capabilities.user_sync_provider);
        assert!(manifest.capabilities.metadata_provider.is_empty());
    }
}
