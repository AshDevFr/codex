//! JSON-RPC Protocol Types for Plugin Communication
//!
//! This module defines the JSON-RPC 2.0 protocol types for communication with plugins,
//! including request/response structures, manifest schema, and metadata types.
//!
//! Note: Many types in this module are part of the plugin protocol specification and
//! are designed for serialization/deserialization. They may not all be used internally
//! yet, but form the complete API contract for plugin communication.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC protocol version
pub const JSONRPC_VERSION: &str = "2.0";

/// Plugin protocol version
///
/// - 1.1 (additive minor): added `total_volume_count` + `total_chapter_count` and the
///   matching `MetadataWriteTotalVolumeCount` / `MetadataWriteTotalChapterCount`
///   permissions; legacy `total_book_count` still accepted on the wire.
/// - 1.2 (breaking minor): legacy `total_book_count` field and
///   `MetadataWriteTotalBookCount` permission removed; plugins must populate the split
///   counts directly.
#[allow(dead_code)] // Protocol contract: sent to plugins during initialize
pub const PROTOCOL_VERSION: &str = "1.2";

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
    #[allow(dead_code)] // Protocol contract: used by plugins and tests
    pub fn new(id: impl Into<RequestId>, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: id.into(),
            method: method.into(),
            params,
        }
    }

    /// Create a request without parameters
    #[allow(dead_code)] // Protocol contract: convenience constructor
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
    #[allow(dead_code)] // Protocol contract: response inspection utility
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

    #[allow(dead_code)] // Protocol contract: error constructor with payload
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
    #[allow(dead_code)] // Standard JSON-RPC error code
    pub const PARSE_ERROR: i32 = -32700;
    /// The JSON sent is not a valid Request object
    #[allow(dead_code)] // Standard JSON-RPC error code
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

    // Sync methods (user plugin sync providers)
    /// Get user info from external service
    pub const SYNC_GET_USER_INFO: &str = "sync/getUserInfo";
    /// Push reading progress to external service
    pub const SYNC_PUSH_PROGRESS: &str = "sync/pushProgress";
    /// Pull reading progress from external service
    pub const SYNC_PULL_PROGRESS: &str = "sync/pullProgress";
    /// Get sync status/diff between Codex and external service
    pub const SYNC_STATUS: &str = "sync/status";

    // Recommendation methods (user plugin recommendation providers)
    /// Get personalized recommendations
    pub const RECOMMENDATIONS_GET: &str = "recommendations/get";
    /// Update taste profile from new user activity
    #[allow(dead_code)] // Protocol contract: method available for future use
    pub const RECOMMENDATIONS_UPDATE_PROFILE: &str = "recommendations/updateProfile";
    /// Clear cached recommendations
    pub const RECOMMENDATIONS_CLEAR: &str = "recommendations/clear";
    /// Dismiss a recommendation (user not interested)
    pub const RECOMMENDATIONS_DISMISS: &str = "recommendations/dismiss";

    // Release-source methods (host -> plugin)
    /// Ask the plugin to poll its source for new releases.
    pub const RELEASES_POLL: &str = "releases/poll";

    // Release-source reverse-RPC methods (plugin -> host)
    /// List tracked series scoped to what the source needs.
    pub const RELEASES_LIST_TRACKED: &str = "releases/list_tracked";
    /// Record a release candidate in the ledger.
    pub const RELEASES_RECORD: &str = "releases/record";
    /// Get the persisted state for a release source (etag, cursor, etc.).
    pub const RELEASES_SOURCE_STATE_GET: &str = "releases/source_state/get";
    /// Set persisted state for a release source.
    pub const RELEASES_SOURCE_STATE_SET: &str = "releases/source_state/set";
    /// Replace the set of release-source rows owned by this plugin.
    /// The host upserts each entry by `(plugin_id, source_key)` and prunes
    /// rows whose `source_key` is no longer in the input list.
    pub const RELEASES_REGISTER_SOURCES: &str = "releases/register_sources";
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

    /// JSON Schema for plugin-specific configuration (admin-facing)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_schema: Option<Value>,

    /// Configuration schema for per-user settings (user-facing)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_config_schema: Option<Value>,

    /// Plugin type: "system" (admin-only metadata) or "user" (per-user integrations)
    #[serde(default)]
    pub plugin_type: PluginManifestType,

    /// OAuth 2.0 configuration for user plugins that require external service authentication
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth: Option<OAuthConfig>,

    /// User-facing description shown when enabling the plugin
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_description: Option<String>,

    /// Admin-facing setup instructions (e.g., how to create OAuth app, set client ID)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_setup_instructions: Option<String>,

    /// User-facing setup instructions (e.g., how to connect or get a personal token)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_setup_instructions: Option<String>,

    /// URI template for searching on the plugin's website.
    /// Use `<title>` as placeholder for the URL-encoded search query.
    /// Example: `https://mangabaka.org/search?sort_by=popularity_asc&q=<title>`
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "searchURITemplate"
    )]
    pub search_uri_template: Option<String>,
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
    pub user_read_sync: bool,
    /// External ID source used to match sync entries to Codex series.
    /// When set, pulled sync entries are matched to series via the
    /// `series_external_ids` table using this source string.
    /// Uses the `api:<service>` convention, e.g. "api:anilist".
    /// Only meaningful when `user_read_sync` is true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id_source: Option<String>,
    /// Can provide personalized recommendations (v2)
    #[serde(default)]
    pub user_recommendation_provider: bool,
    /// Can announce new releases (chapters/volumes) for tracked series.
    /// When present, the plugin may invoke the `releases/*` reverse-RPC
    /// methods. The capability struct declares the data the plugin needs
    /// (aliases, external IDs) so the host can scope its responses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_source: Option<ReleaseSourceCapability>,
}

/// Release-source capability declaration.
///
/// Plugins that want to announce releases declare this capability in their
/// manifest. The struct describes both *what* the plugin can announce and
/// *what* it needs from the host. The host uses these fields when filling
/// `releases/list_tracked` responses so plugins only see data they asked for.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseSourceCapability {
    /// Source kinds this plugin exposes (e.g. `["rss-uploader"]`).
    #[serde(default)]
    pub kinds: Vec<ReleaseSourceKind>,
    /// Whether the plugin needs title aliases (set when the plugin matches
    /// by title rather than by external ID, e.g. Nyaa).
    #[serde(default)]
    pub requires_aliases: bool,
    /// External-ID sources the plugin needs, e.g. `["mangaupdates"]` or
    /// `["mangadex"]`. The host filters `series_external_ids` to these
    /// sources when responding to `releases/list_tracked`.
    #[serde(default)]
    pub requires_external_ids: Vec<String>,
    /// Whether the plugin announces chapter-level releases.
    #[serde(default)]
    pub can_announce_chapters: bool,
    /// Whether the plugin announces volume-level releases.
    #[serde(default)]
    pub can_announce_volumes: bool,
    /// Default poll interval in seconds. Used when a `release_sources` row
    /// for this plugin doesn't override it. Server settings can also set a
    /// global default that takes precedence at schedule resolution time.
    #[serde(default)]
    pub default_poll_interval_s: u32,
}

impl Default for ReleaseSourceCapability {
    fn default() -> Self {
        Self {
            kinds: Vec::new(),
            requires_aliases: false,
            requires_external_ids: Vec::new(),
            can_announce_chapters: true,
            can_announce_volumes: true,
            default_poll_interval_s: 86_400,
        }
    }
}

/// Kind of release source. Mirrors the `release_sources.kind` column on the
/// host side, but lives here so plugins can declare it without depending on
/// the database schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReleaseSourceKind {
    /// Per-uploader feed (e.g., a Nyaa user RSS feed).
    RssUploader,
    /// Per-series feed (e.g., MangaUpdates RSS for a single series).
    RssSeries,
    /// Generic API-driven feed.
    ApiFeed,
    /// Metadata-derived signal (informational; usually doesn't write the
    /// ledger - see Phase 5).
    MetadataFeed,
}

impl ReleaseSourceKind {
    /// Canonical kebab-case string matching `release_sources.kind` and the
    /// serde representation. Used when comparing against string-typed
    /// `kind` fields parsed from RPC requests.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RssUploader => "rss-uploader",
            Self::RssSeries => "rss-series",
            Self::ApiFeed => "api-feed",
            Self::MetadataFeed => "metadata-feed",
        }
    }
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

    /// Whether this plugin declares the `release_source` capability.
    pub fn is_release_source(&self) -> bool {
        self.release_source.is_some()
    }

    /// Infer the plugin type from capabilities.
    ///
    /// User-facing capabilities (`user_read_sync`, `user_recommendation_provider`)
    /// indicate a "user" plugin. Metadata-provider and release-source
    /// capabilities indicate a "system" plugin. Returns `None` when
    /// capabilities are empty.
    pub fn inferred_plugin_type(&self) -> Option<PluginManifestType> {
        if self.user_read_sync || self.user_recommendation_provider {
            Some(PluginManifestType::User)
        } else if !self.metadata_provider.is_empty() || self.release_source.is_some() {
            Some(PluginManifestType::System)
        } else {
            None
        }
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

impl std::fmt::Display for PluginManifestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::System => write!(f, "system"),
            Self::User => write!(f, "user"),
        }
    }
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
    #[allow(dead_code)] // Protocol contract: validation for plugin registration
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
    #[allow(dead_code)] // Protocol contract: scope helpers for book metadata plugins
    pub fn book_scopes() -> Vec<Self> {
        vec![
            Self::BookDetail,
            Self::BookBulk,
            Self::LibraryDetail,
            Self::LibraryScan,
        ]
    }

    /// Get all scopes (series + book + library)
    #[allow(dead_code)] // Protocol contract: scope helpers for multi-content plugins
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
    /// Content format discriminator (e.g. `manga`, `novel`, `light_novel`,
    /// `manhwa`, `manhua`, `comic`, `webtoon`, `one_shot`, `doujin`,
    /// `artbook`).
    ///
    /// Free-form at the protocol level so plugins are not locked into an
    /// enum that requires Codex core changes when new formats appear.
    /// Plugin authors are encouraged to emit lowercase snake_case values
    /// from the recommended vocabulary above so the UI can render
    /// consistent badges; unknown values still render as a neutral badge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
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
    #[allow(dead_code)] // Protocol contract: query type inspection
    pub fn is_isbn_search(&self) -> bool {
        self.isbn.is_some()
    }

    /// Check if this is a query-based search
    #[allow(dead_code)] // Protocol contract: query type inspection
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
    /// Expected total number of volumes in the series, when known.
    /// Use this for volume-organized libraries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_volume_count: Option<i32>,
    /// Expected total number of chapters in the series, when known. May be fractional.
    /// Use this for chapter-organized libraries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_chapter_count: Option<f32>,
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
    #[serde(default, deserialize_with = "deserialize_series_authors")]
    pub authors: Vec<BookAuthor>,
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

    // External IDs (cross-references to other services)
    /// Cross-reference IDs from other services (e.g., AniList, MAL, MangaDex).
    /// These use the `api:` prefix convention (e.g., "api:anilist").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_ids: Vec<PluginExternalId>,
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

    // =========================================================================
    // External IDs (cross-references to other services)
    // =========================================================================
    /// Cross-reference IDs from other services (e.g., OpenLibrary, Goodreads).
    /// These use the `api:` prefix convention (e.g., "api:openlibrary").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_ids: Vec<PluginExternalId>,
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
#[serde(rename_all = "snake_case")]
pub enum BookAuthorRole {
    #[default]
    Author,
    CoAuthor,
    Editor,
    Translator,
    Illustrator,
    Contributor,
    // Comic-specific roles
    Writer,
    Penciller,
    Inker,
    Colorist,
    Letterer,
    CoverArtist,
}

/// Custom deserializer for series authors that accepts both:
/// - Legacy format: `["Author Name", "Another Author"]` (Vec<String>)
/// - New format: `[{"name": "Author Name", "role": "author"}]` (Vec<BookAuthor>)
fn deserialize_series_authors<'de, D>(deserializer: D) -> Result<Vec<BookAuthor>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum AuthorItem {
        Structured(BookAuthor),
        Plain(String),
    }

    let items: Vec<AuthorItem> = Vec::deserialize(deserializer)?;
    Ok(items
        .into_iter()
        .filter_map(|item| match item {
            AuthorItem::Structured(author) => Some(author),
            AuthorItem::Plain(name) => {
                let name = name.trim().to_string();
                if name.is_empty() {
                    None
                } else {
                    Some(BookAuthor {
                        name,
                        role: BookAuthorRole::Author,
                        sort_name: None,
                    })
                }
            }
        })
        .collect())
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

/// External ID cross-reference from a metadata provider
///
/// Allows metadata plugins to return IDs for the same series on other services.
/// For example, a MangaBaka plugin can return the AniList and MAL IDs it knows about.
///
/// ## Source Naming Convention
///
/// - `api:<service>` - External API service ID (e.g., "api:anilist", "api:myanimelist")
/// - `plugin:<name>` - Plugin match provenance (managed by Codex, not returned by plugins)
/// - No prefix - File/user sources (e.g., "comicinfo", "epub", "manual")
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginExternalId {
    /// Source identifier (e.g., "api:anilist", "api:myanimelist", "api:mangadex")
    pub source: String,
    /// ID on the external service
    pub external_id: String,
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
// User Library Data Contract (Sync Providers)
// =============================================================================

/// A user's library entry sent to sync plugins
///
/// Contains series info, reading progress, and the user's personal data
/// (rating, notes) needed for sync providers to push/pull state with
/// external services.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserLibraryEntry {
    /// Codex series ID
    pub series_id: String,
    /// Primary title
    pub title: String,
    /// Alternative titles (native, romaji, etc.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternate_titles: Vec<String>,
    /// Publication year
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    /// Series status
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<SeriesStatus>,
    /// Genres
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genres: Vec<String>,
    /// Tags
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Expected total number of volumes in the series, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_volume_count: Option<i32>,
    /// Expected total number of chapters in the series, when known. May be fractional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_chapter_count: Option<f32>,

    /// Known external IDs (source → external_id mapping)
    /// e.g., {"anilist": "12345", "myanimelist": "67890"}
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_ids: Vec<UserLibraryExternalId>,

    /// User's reading status (derived from progress across books)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reading_status: Option<UserReadingStatus>,
    /// Number of books the user has completed in this series
    #[serde(default)]
    pub books_read: i32,
    /// Total number of books in the user's library for this series
    #[serde(default)]
    pub books_owned: i32,
    /// User's personal rating (1-100 scale)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_rating: Option<i32>,
    /// User's personal notes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_notes: Option<String>,
    /// When the user started reading (ISO 8601)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    /// When the user last read (ISO 8601)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_read_at: Option<String>,
    /// When the user completed the series (ISO 8601)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

/// External ID mapping for a library entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserLibraryExternalId {
    /// Source name (e.g., "anilist", "myanimelist", "mangadex")
    pub source: String,
    /// External ID on that service
    pub external_id: String,
    /// URL to the entry on the external service
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_url: Option<String>,
}

/// User's reading status for a series (derived from book progress)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserReadingStatus {
    /// User has not started reading
    Unread,
    /// User is currently reading (some books have progress)
    Reading,
    /// User has completed all available books
    Completed,
}

// =============================================================================
// Initialize Response
// =============================================================================

/// Parameters for initialize (usually empty or with config)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    /// Plugin configuration from Codex (merged admin + user, deprecated)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
    /// Admin-level plugin configuration (from plugin settings)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_config: Option<Value>,
    /// Per-user plugin configuration (from user plugin settings)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_config: Option<Value>,
    /// Credentials passed via init message (alternative to env vars)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<Value>,
    /// Scoped data directory for this plugin's file storage.
    /// Created by the host at `{plugins_dir}/{plugin_name}/`.
    /// Plugins can use this for larger file-based storage (SQLite DBs, caches, etc.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_dir: Option<String>,
}

// =============================================================================
// Releases Poll (host -> plugin)
// =============================================================================

/// Parameters for `releases/poll` (host → plugin call).
///
/// The host invokes this once per scheduled poll for a single
/// `release_sources` row. The plugin uses the `source_id` to scope its work
/// (which feed/uploader/series to query) and may consult the supplied
/// `etag` for conditional GETs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePollRequest {
    /// Source row the plugin should poll. The plugin can call back into
    /// `releases/source_state/get` for richer state (etag, last_polled_at)
    /// or `releases/list_tracked` to harvest the tracked-series scope.
    pub source_id: uuid::Uuid,
    /// Plugin-defined stable key for this source row (the same value the
    /// plugin originally passed to `releases/register_sources`). Carried in
    /// the poll request so the plugin can dispatch directly without a
    /// reverse-RPC roundtrip — useful when one plugin process owns multiple
    /// source rows (e.g., one per Nyaa uploader).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_key: Option<String>,
    /// Snapshot of `release_sources.config` at poll time, if any. Plugins
    /// that store per-source config on register can read it back here to
    /// avoid keeping their own `(sourceKey, config)` map in memory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    /// Etag value from the previous successful poll, if any. Plugins doing
    /// HTTP conditional GETs (`If-None-Match`) can use it directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
}

/// Response from `releases/poll`.
///
/// Plugins MAY also call `releases/record` directly during polling (the
/// reverse-RPC channel is open). The `candidates` field is convenience for
/// plugins that prefer to return everything at once; both styles are
/// supported and the host treats them identically.
///
/// Plugins that stream via `releases/record` should also populate the
/// counter fields (`parsed`, `matched`, `recorded`, `deduped`) so the host
/// can build an accurate `last_summary` for the source. Without those, the
/// host can only see what came back in `candidates` and a streaming
/// plugin's status badge will read "Fetched 0 items" no matter what.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePollResponse {
    /// Optional batch of candidates the host should evaluate and ledger
    /// (in addition to anything the plugin already streamed via
    /// `releases/record`).
    #[serde(default)]
    pub candidates: Vec<crate::services::release::candidate::ReleaseCandidate>,
    /// New etag observed by the plugin (e.g. from the upstream feed's
    /// `ETag` header). The host stores this on the source row for the
    /// next poll's conditional-GET.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// Whether the upstream returned `304 Not Modified` (or equivalent
    /// "no work" signal). Purely informational; the host doesn't act on it
    /// beyond logging.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_modified: Option<bool>,
    /// HTTP status code observed from the upstream feed, if any. Used by
    /// the host's per-host backoff layer to detect 429 / 503.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_status: Option<u16>,
    /// Items the plugin parsed from the upstream feed before any matching
    /// or threshold filtering. Streaming plugins should populate this so
    /// the host's `last_summary` reflects upstream activity, not just the
    /// shape of the response payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parsed: Option<u32>,
    /// Of those parsed, the count that matched a tracked series alias
    /// (i.e. that became candidates the plugin then evaluated/streamed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched: Option<u32>,
    /// Of those matched, the count actually inserted into the ledger
    /// (excludes dedupes). For plugins that stream via `releases/record`,
    /// this is the count of non-deduped record outcomes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded: Option<u32>,
    /// Of those matched, the count that the host deduped onto an existing
    /// ledger row. Optional; when omitted the host infers `matched -
    /// recorded`. Provided explicitly by streaming plugins that already
    /// know.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deduped: Option<u32>,
}

// =============================================================================
// Rate Limit Error Data
// =============================================================================

/// Data included with rate limit errors
#[allow(dead_code)] // Protocol contract: rate limit error payload schema
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
        assert_eq!(result.preview.as_ref().unwrap().format, None);
    }

    #[test]
    fn test_search_result_preview_with_format_round_trip() {
        let preview = SearchResultPreview {
            status: Some("ongoing".to_string()),
            genres: vec!["Action".to_string()],
            rating: None,
            description: None,
            book_count: None,
            authors: vec![],
            format: Some("manga".to_string()),
        };

        let json = serde_json::to_value(&preview).unwrap();
        assert_eq!(json["format"], "manga");

        let parsed: SearchResultPreview = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.format.as_deref(), Some("manga"));
    }

    #[test]
    fn test_search_result_preview_format_optional_old_shape() {
        // Old plugin output (no `format` field) must still deserialize.
        let json = json!({
            "status": "ongoing",
            "genres": ["Action", "Adventure"],
            "bookCount": 14,
        });

        let preview: SearchResultPreview = serde_json::from_value(json).unwrap();
        assert_eq!(preview.format, None);
        assert_eq!(preview.book_count, Some(14));
    }

    #[test]
    fn test_search_result_preview_format_skipped_when_none() {
        let preview = SearchResultPreview {
            status: None,
            genres: vec![],
            rating: None,
            description: None,
            book_count: None,
            authors: vec![],
            format: None,
        };

        let json = serde_json::to_value(&preview).unwrap();
        assert!(!json.as_object().unwrap().contains_key("format"));
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
            total_volume_count: Some(100),
            total_chapter_count: Some(1086.0),
            language: Some("ja".to_string()),
            age_rating: Some(13),
            reading_direction: Some("rtl".to_string()),
            genres: vec!["Action".to_string(), "Adventure".to_string()],
            tags: vec!["pirates".to_string()],
            authors: vec![BookAuthor {
                name: "Oda, Eiichiro".to_string(),
                role: BookAuthorRole::Author,
                sort_name: None,
            }],
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
            external_ids: vec![
                PluginExternalId {
                    source: "api:anilist".to_string(),
                    external_id: "21".to_string(),
                },
                PluginExternalId {
                    source: "api:myanimelist".to_string(),
                    external_id: "13".to_string(),
                },
            ],
        };

        let json = serde_json::to_value(&metadata).unwrap();
        assert_eq!(json["externalId"], "12345");
        assert_eq!(json["status"], "ongoing");
        let ext_ids = json["externalIds"].as_array().unwrap();
        assert_eq!(ext_ids.len(), 2);
        assert_eq!(ext_ids[0]["source"], "api:anilist");
        assert_eq!(ext_ids[0]["externalId"], "21");
        assert_eq!(ext_ids[1]["source"], "api:myanimelist");
        assert_eq!(ext_ids[1]["externalId"], "13");
    }

    #[test]
    fn test_plugin_external_id_serialization() {
        let ext_id = PluginExternalId {
            source: "api:anilist".to_string(),
            external_id: "97".to_string(),
        };
        let json = serde_json::to_value(&ext_id).unwrap();
        assert_eq!(json["source"], "api:anilist");
        assert_eq!(json["externalId"], "97");
    }

    #[test]
    fn test_plugin_external_id_deserialization() {
        let json = serde_json::json!({
            "source": "api:mangadex",
            "externalId": "abc-def-123"
        });
        let ext_id: PluginExternalId = serde_json::from_value(json).unwrap();
        assert_eq!(ext_id.source, "api:mangadex");
        assert_eq!(ext_id.external_id, "abc-def-123");
    }

    #[test]
    fn test_plugin_series_metadata_empty_external_ids_skipped() {
        let metadata = PluginSeriesMetadata {
            external_id: "1".to_string(),
            external_url: "https://example.com/1".to_string(),
            title: None,
            alternate_titles: vec![],
            summary: None,
            status: None,
            year: None,
            total_volume_count: None,
            total_chapter_count: None,
            language: None,
            age_rating: None,
            reading_direction: None,
            genres: vec![],
            tags: vec![],
            authors: vec![],
            artists: vec![],
            publisher: None,
            cover_url: None,
            banner_url: None,
            rating: None,
            external_ratings: vec![],
            external_links: vec![],
            external_ids: vec![],
        };
        let json = serde_json::to_value(&metadata).unwrap();
        // externalIds should be omitted when empty
        assert!(!json.as_object().unwrap().contains_key("externalIds"));
    }

    #[test]
    fn test_plugin_series_metadata_split_counts_round_trip() {
        // Both volume and chapter counts populate cleanly.
        let json = json!({
            "externalId": "abc",
            "externalUrl": "https://example.com/series/abc",
            "title": "One Piece",
            "totalVolumeCount": 14,
            "totalChapterCount": 109.5,
        });
        let parsed: PluginSeriesMetadata = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.total_volume_count, Some(14));
        assert_eq!(parsed.total_chapter_count, Some(109.5));

        // Round-trip back to JSON preserves both fields.
        let serialized = serde_json::to_value(&parsed).unwrap();
        assert_eq!(serialized["totalVolumeCount"], 14);
        assert_eq!(serialized["totalChapterCount"], 109.5);
    }

    #[test]
    fn test_plugin_series_metadata_legacy_total_book_count_is_ignored() {
        // Protocol 1.2 dropped the legacy field. Older plugins that still emit it
        // must still parse (serde ignores unknown fields by default), but the
        // value is silently discarded - there is no longer a routing path for it.
        let json = json!({
            "externalId": "old-1",
            "externalUrl": "https://example.com/old-1",
            "totalBookCount": 14,
        });
        let parsed: PluginSeriesMetadata = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.total_volume_count, None);
        assert_eq!(parsed.total_chapter_count, None);
    }

    #[test]
    fn test_plugin_series_metadata_skips_unset_count_fields() {
        // When all three count fields are None, none should be present on the wire.
        let metadata = PluginSeriesMetadata {
            external_id: "1".to_string(),
            external_url: "https://example.com/1".to_string(),
            title: None,
            alternate_titles: vec![],
            summary: None,
            status: None,
            year: None,
            total_volume_count: None,
            total_chapter_count: None,
            language: None,
            age_rating: None,
            reading_direction: None,
            genres: vec![],
            tags: vec![],
            authors: vec![],
            artists: vec![],
            publisher: None,
            cover_url: None,
            banner_url: None,
            rating: None,
            external_ratings: vec![],
            external_links: vec![],
            external_ids: vec![],
        };
        let json = serde_json::to_value(&metadata).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("totalBookCount"));
        assert!(!obj.contains_key("totalVolumeCount"));
        assert!(!obj.contains_key("totalChapterCount"));
    }

    #[test]
    fn test_user_library_entry_split_counts_round_trip() {
        let json = json!({
            "seriesId": "uuid-1",
            "title": "One Piece",
            "totalVolumeCount": 107,
            "totalChapterCount": 1086.5,
            "booksRead": 0,
            "booksOwned": 0,
        });
        let parsed: UserLibraryEntry = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.total_volume_count, Some(107));
        assert_eq!(parsed.total_chapter_count, Some(1086.5));

        let serialized = serde_json::to_value(&parsed).unwrap();
        assert_eq!(serialized["totalVolumeCount"], 107);
        assert_eq!(serialized["totalChapterCount"], 1086.5);
    }

    #[test]
    fn test_protocol_version_is_minor_bumped() {
        // Phase 9 of metadata-count-split bumps the protocol from 1.1 to 1.2:
        // legacy `totalBookCount` field and `metadata:write:total_book_count`
        // permission are removed. Plugins that still emit the legacy field
        // round-trip through serde silently (the field is dropped on decode).
        assert_eq!(PROTOCOL_VERSION, "1.2");
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
            external_ids: vec![PluginExternalId {
                source: "api:openlibrary".to_string(),
                external_id: "OL12345M".to_string(),
            }],
        };

        let json = serde_json::to_value(&metadata).unwrap();
        assert_eq!(json["externalId"], "12345");
        assert_eq!(json["title"], "The Hobbit");
        assert_eq!(json["subtitle"], "or There and Back Again");
        assert_eq!(json["bookType"], "novel");
        assert_eq!(json["year"], 1937);
        assert_eq!(json["isbn"], "978-0-547-92822-7");
        assert_eq!(json["authors"][0]["name"], "J.R.R. Tolkien");
        assert_eq!(json["externalIds"][0]["source"], "api:openlibrary");
        assert_eq!(json["externalIds"][0]["externalId"], "OL12345M");
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
    fn test_plugin_manifest_type_display() {
        assert_eq!(PluginManifestType::System.to_string(), "system");
        assert_eq!(PluginManifestType::User.to_string(), "user");
    }

    #[test]
    fn test_inferred_plugin_type_from_user_read_sync() {
        let caps = PluginCapabilities {
            user_read_sync: true,
            ..Default::default()
        };
        assert_eq!(caps.inferred_plugin_type(), Some(PluginManifestType::User));
    }

    #[test]
    fn test_inferred_plugin_type_from_recommendation_provider() {
        let caps = PluginCapabilities {
            user_recommendation_provider: true,
            ..Default::default()
        };
        assert_eq!(caps.inferred_plugin_type(), Some(PluginManifestType::User));
    }

    #[test]
    fn test_inferred_plugin_type_from_metadata_provider() {
        let caps = PluginCapabilities {
            metadata_provider: vec![MetadataContentType::Series],
            ..Default::default()
        };
        assert_eq!(
            caps.inferred_plugin_type(),
            Some(PluginManifestType::System)
        );
    }

    #[test]
    fn test_inferred_plugin_type_empty_capabilities() {
        let caps = PluginCapabilities::default();
        assert_eq!(caps.inferred_plugin_type(), None);
    }

    #[test]
    fn test_release_source_capability_serializes_camel_case() {
        let cap = ReleaseSourceCapability {
            kinds: vec![ReleaseSourceKind::RssUploader],
            requires_aliases: true,
            requires_external_ids: vec!["mangaupdates".to_string()],
            can_announce_chapters: true,
            can_announce_volumes: false,
            default_poll_interval_s: 3600,
        };
        let json = serde_json::to_value(&cap).unwrap();
        assert_eq!(json["kinds"], json!(["rss-uploader"]));
        assert!(json["requiresAliases"].as_bool().unwrap());
        assert_eq!(json["requiresExternalIds"], json!(["mangaupdates"]));
        assert!(json["canAnnounceChapters"].as_bool().unwrap());
        assert!(!json["canAnnounceVolumes"].as_bool().unwrap());
        assert_eq!(json["defaultPollIntervalS"], 3600);
    }

    #[test]
    fn test_release_source_capability_kind_round_trip() {
        for kind in [
            ReleaseSourceKind::RssUploader,
            ReleaseSourceKind::RssSeries,
            ReleaseSourceKind::ApiFeed,
            ReleaseSourceKind::MetadataFeed,
        ] {
            let json = serde_json::to_value(kind).unwrap();
            let back: ReleaseSourceKind = serde_json::from_value(json).unwrap();
            assert_eq!(kind, back);
        }
    }

    #[test]
    fn test_plugin_capabilities_release_source_inferred_type() {
        let caps = PluginCapabilities {
            release_source: Some(ReleaseSourceCapability::default()),
            ..Default::default()
        };
        assert!(caps.is_release_source());
        assert_eq!(
            caps.inferred_plugin_type(),
            Some(PluginManifestType::System)
        );
    }

    #[test]
    fn test_plugin_capabilities_manifest_parses_release_source() {
        let manifest_json = json!({
            "name": "release-nyaa",
            "displayName": "Nyaa Releases",
            "version": "0.1.0",
            "protocolVersion": "1.2",
            "capabilities": {
                "releaseSource": {
                    "kinds": ["rss-uploader"],
                    "requiresAliases": true,
                    "requiresExternalIds": [],
                    "canAnnounceChapters": true,
                    "canAnnounceVolumes": true,
                    "defaultPollIntervalS": 3600
                }
            }
        });
        let manifest: PluginManifest = serde_json::from_value(manifest_json).unwrap();
        assert!(manifest.capabilities.is_release_source());
        let cap = manifest.capabilities.release_source.unwrap();
        assert_eq!(cap.kinds, vec![ReleaseSourceKind::RssUploader]);
        assert!(cap.requires_aliases);
        assert_eq!(cap.default_poll_interval_s, 3600);
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
            "name": "sync-anilist",
            "displayName": "AniList Sync",
            "version": "1.0.0",
            "protocolVersion": "1.0",
            "pluginType": "user",
            "capabilities": {
                "userReadSync": true
            },
            "oauth": {
                "authorizationUrl": "https://anilist.co/api/v2/oauth/authorize",
                "tokenUrl": "https://anilist.co/api/v2/oauth/token",
                "scopes": [],
                "pkce": false
            },
            "userDescription": "Sync reading progress with AniList",
            "adminSetupInstructions": "Create an AniList app at ...",
            "userSetupInstructions": "Click Connect to link your AniList account"
        });

        let manifest: PluginManifest = serde_json::from_value(json).unwrap();
        assert_eq!(manifest.name, "sync-anilist");
        assert_eq!(manifest.plugin_type, PluginManifestType::User);
        assert!(manifest.capabilities.user_read_sync);
        assert!(!manifest.capabilities.user_recommendation_provider);

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
        assert!(manifest.admin_setup_instructions.is_some());
        assert!(manifest.user_setup_instructions.is_some());
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
                "userRecommendationProvider": true
            }
        });

        let manifest: PluginManifest = serde_json::from_value(json).unwrap();
        assert!(manifest.capabilities.user_recommendation_provider);
        assert!(!manifest.capabilities.user_read_sync);
        assert!(manifest.capabilities.metadata_provider.is_empty());
    }

    // =========================================================================
    // User Library Data Contract Tests
    // =========================================================================

    #[test]
    fn test_user_library_entry_full_serialization() {
        let entry = UserLibraryEntry {
            series_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            title: "One Piece".to_string(),
            alternate_titles: vec!["ワンピース".to_string()],
            year: Some(1997),
            status: Some(SeriesStatus::Ongoing),
            genres: vec!["Action".to_string(), "Adventure".to_string()],
            tags: vec!["pirates".to_string()],
            total_volume_count: Some(107),
            total_chapter_count: Some(1086.5),
            external_ids: vec![UserLibraryExternalId {
                source: "anilist".to_string(),
                external_id: "21".to_string(),
                external_url: Some("https://anilist.co/manga/21".to_string()),
            }],
            reading_status: Some(UserReadingStatus::Reading),
            books_read: 95,
            books_owned: 100,
            user_rating: Some(95),
            user_notes: Some("Masterpiece".to_string()),
            started_at: Some("2024-01-01T00:00:00Z".to_string()),
            last_read_at: Some("2026-02-06T00:00:00Z".to_string()),
            completed_at: None,
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["seriesId"], "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(json["title"], "One Piece");
        assert_eq!(json["alternateTitles"][0], "ワンピース");
        assert_eq!(json["year"], 1997);
        assert_eq!(json["status"], "ongoing");
        assert_eq!(json["genres"].as_array().unwrap().len(), 2);
        assert_eq!(json["totalVolumeCount"], 107);
        assert_eq!(json["totalChapterCount"], 1086.5);
        assert_eq!(json["externalIds"][0]["source"], "anilist");
        assert_eq!(json["externalIds"][0]["externalId"], "21");
        assert_eq!(json["readingStatus"], "reading");
        assert_eq!(json["booksRead"], 95);
        assert_eq!(json["booksOwned"], 100);
        assert_eq!(json["userRating"], 95);
        assert_eq!(json["userNotes"], "Masterpiece");
        assert!(!json.as_object().unwrap().contains_key("completedAt"));
    }

    #[test]
    fn test_user_library_entry_minimal() {
        let entry = UserLibraryEntry {
            series_id: "abc".to_string(),
            title: "Test".to_string(),
            alternate_titles: vec![],
            year: None,
            status: None,
            genres: vec![],
            tags: vec![],
            total_volume_count: None,
            total_chapter_count: None,
            external_ids: vec![],
            reading_status: None,
            books_read: 0,
            books_owned: 3,
            user_rating: None,
            user_notes: None,
            started_at: None,
            last_read_at: None,
            completed_at: None,
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["seriesId"], "abc");
        assert_eq!(json["title"], "Test");
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("alternateTitles"));
        assert!(!obj.contains_key("year"));
        assert!(!obj.contains_key("status"));
        assert!(!obj.contains_key("genres"));
        assert!(!obj.contains_key("externalIds"));
        assert!(!obj.contains_key("readingStatus"));
        assert!(!obj.contains_key("userRating"));
    }

    #[test]
    fn test_user_library_entry_deserialization() {
        let json = json!({
            "seriesId": "123",
            "title": "Berserk",
            "readingStatus": "completed",
            "booksRead": 42,
            "booksOwned": 42,
            "userRating": 100,
            "completedAt": "2025-12-01T00:00:00Z"
        });
        let entry: UserLibraryEntry = serde_json::from_value(json).unwrap();
        assert_eq!(entry.series_id, "123");
        assert_eq!(entry.title, "Berserk");
        assert_eq!(entry.reading_status, Some(UserReadingStatus::Completed));
        assert_eq!(entry.books_read, 42);
        assert_eq!(entry.user_rating, Some(100));
        assert_eq!(entry.completed_at.unwrap(), "2025-12-01T00:00:00Z");
    }

    #[test]
    fn test_user_library_external_id_serialization() {
        let ext_id = UserLibraryExternalId {
            source: "myanimelist".to_string(),
            external_id: "99999".to_string(),
            external_url: Some("https://myanimelist.net/manga/99999".to_string()),
        };
        let json = serde_json::to_value(&ext_id).unwrap();
        assert_eq!(json["source"], "myanimelist");
        assert_eq!(json["externalId"], "99999");
        assert_eq!(json["externalUrl"], "https://myanimelist.net/manga/99999");
    }

    #[test]
    fn test_user_library_external_id_without_url() {
        let ext_id = UserLibraryExternalId {
            source: "comicinfo".to_string(),
            external_id: "abc".to_string(),
            external_url: None,
        };
        let json = serde_json::to_value(&ext_id).unwrap();
        assert!(!json.as_object().unwrap().contains_key("externalUrl"));
    }

    #[test]
    fn test_user_reading_status_serialization() {
        assert_eq!(
            serde_json::to_value(UserReadingStatus::Unread).unwrap(),
            json!("unread")
        );
        assert_eq!(
            serde_json::to_value(UserReadingStatus::Reading).unwrap(),
            json!("reading")
        );
        assert_eq!(
            serde_json::to_value(UserReadingStatus::Completed).unwrap(),
            json!("completed")
        );
    }

    #[test]
    fn test_user_reading_status_deserialization() {
        let unread: UserReadingStatus = serde_json::from_value(json!("unread")).unwrap();
        assert_eq!(unread, UserReadingStatus::Unread);
        let reading: UserReadingStatus = serde_json::from_value(json!("reading")).unwrap();
        assert_eq!(reading, UserReadingStatus::Reading);
        let completed: UserReadingStatus = serde_json::from_value(json!("completed")).unwrap();
        assert_eq!(completed, UserReadingStatus::Completed);
    }

    #[test]
    fn test_user_library_entry_multiple_external_ids() {
        let entry = UserLibraryEntry {
            series_id: "s1".to_string(),
            title: "Test Series".to_string(),
            alternate_titles: vec![],
            year: None,
            status: None,
            genres: vec![],
            tags: vec![],
            total_volume_count: None,
            total_chapter_count: None,
            external_ids: vec![
                UserLibraryExternalId {
                    source: "anilist".to_string(),
                    external_id: "21".to_string(),
                    external_url: None,
                },
                UserLibraryExternalId {
                    source: "myanimelist".to_string(),
                    external_id: "13".to_string(),
                    external_url: None,
                },
            ],
            reading_status: None,
            books_read: 0,
            books_owned: 0,
            user_rating: None,
            user_notes: None,
            started_at: None,
            last_read_at: None,
            completed_at: None,
        };
        let json = serde_json::to_value(&entry).unwrap();
        let ids = json["externalIds"].as_array().unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0]["source"], "anilist");
        assert_eq!(ids[1]["source"], "myanimelist");
    }

    // =========================================================================
    // InitializeParams Tests
    // =========================================================================

    #[test]
    fn test_initialize_params_with_split_config() {
        let params = InitializeParams {
            config: None,
            admin_config: Some(json!({"clientId": "abc"})),
            user_config: Some(json!({"progressUnit": "chapters"})),
            credentials: Some(json!({"access_token": "secret"})),
            data_dir: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["adminConfig"]["clientId"], "abc");
        assert_eq!(json["userConfig"]["progressUnit"], "chapters");
        assert_eq!(json["credentials"]["access_token"], "secret");
        assert!(!json.as_object().unwrap().contains_key("config"));
    }

    #[test]
    fn test_initialize_params_with_legacy_config() {
        let params = InitializeParams {
            config: Some(json!({"merged": true})),
            admin_config: None,
            user_config: None,
            credentials: None,
            data_dir: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["config"]["merged"], true);
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("adminConfig"));
        assert!(!obj.contains_key("userConfig"));
        assert!(!obj.contains_key("credentials"));
    }

    #[test]
    fn test_initialize_params_deserialization_with_split_config() {
        let json = json!({
            "adminConfig": {"clientId": "abc"},
            "userConfig": {"progressUnit": "chapters"},
            "credentials": {"access_token": "secret"}
        });
        let params: InitializeParams = serde_json::from_value(json).unwrap();
        assert!(params.config.is_none());
        assert_eq!(params.admin_config.unwrap()["clientId"], "abc");
        assert_eq!(params.user_config.unwrap()["progressUnit"], "chapters");
        assert_eq!(params.credentials.unwrap()["access_token"], "secret");
    }

    #[test]
    fn test_initialize_params_deserialization_backward_compat() {
        // Old format: only config field (no adminConfig/userConfig)
        let json = json!({
            "config": {"clientId": "abc", "progressUnit": "chapters"},
            "credentials": {"access_token": "secret"}
        });
        let params: InitializeParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.config.unwrap()["clientId"], "abc");
        assert!(params.admin_config.is_none());
        assert!(params.user_config.is_none());
    }

    #[test]
    fn test_initialize_params_empty() {
        let params = InitializeParams::default();
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json, json!({}));
    }

    #[test]
    fn test_initialize_params_with_data_dir() {
        let params = InitializeParams {
            data_dir: Some("/data/plugins/my-plugin".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["dataDir"], "/data/plugins/my-plugin");
        // Other fields should be omitted when None
        assert!(!json.as_object().unwrap().contains_key("config"));
        assert!(!json.as_object().unwrap().contains_key("credentials"));
    }

    #[test]
    fn test_initialize_params_data_dir_deserialization() {
        let json = json!({
            "adminConfig": {"key": "val"},
            "dataDir": "/data/plugins/test-plugin"
        });
        let params: InitializeParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.data_dir.unwrap(), "/data/plugins/test-plugin");
        assert!(params.admin_config.is_some());
    }

    #[test]
    fn test_plugin_manifest_with_search_uri_template() {
        let json = json!({
            "name": "test-plugin",
            "displayName": "Test Plugin",
            "version": "1.0.0",
            "protocolVersion": "1.0",
            "capabilities": {
                "metadataProvider": ["series"]
            },
            "searchURITemplate": "https://example.com/search?q=<title>"
        });

        let manifest: PluginManifest = serde_json::from_value(json).unwrap();
        assert_eq!(
            manifest.search_uri_template.as_deref(),
            Some("https://example.com/search?q=<title>")
        );
    }

    #[test]
    fn test_plugin_manifest_without_search_uri_template() {
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
        assert!(manifest.search_uri_template.is_none());
    }

    #[test]
    fn test_plugin_manifest_search_uri_template_round_trip() {
        let json = json!({
            "name": "test-plugin",
            "displayName": "Test Plugin",
            "version": "1.0.0",
            "protocolVersion": "1.0",
            "capabilities": {
                "metadataProvider": ["series"]
            },
            "searchURITemplate": "https://mangabaka.org/search?sort_by=popularity_asc&q=<title>"
        });

        let manifest: PluginManifest = serde_json::from_value(json).unwrap();
        let serialized = serde_json::to_value(&manifest).unwrap();
        // The field is renamed explicitly to "searchURITemplate" to match plugin protocol
        assert_eq!(
            serialized["searchURITemplate"],
            "https://mangabaka.org/search?sort_by=popularity_asc&q=<title>"
        );
    }

    #[test]
    fn test_plugin_manifest_search_uri_template_omitted_when_none() {
        let json = json!({
            "name": "test-plugin",
            "displayName": "Test Plugin",
            "version": "1.0.0",
            "protocolVersion": "1.0",
            "capabilities": {}
        });

        let manifest: PluginManifest = serde_json::from_value(json).unwrap();
        let serialized = serde_json::to_value(&manifest).unwrap();
        assert!(
            !serialized
                .as_object()
                .unwrap()
                .contains_key("searchURITemplate")
        );
    }
}
