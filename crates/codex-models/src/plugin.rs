//! Plugin manifest and scope value types shared between the db and
//! services layers.
//!
//! The JSON-RPC wire format and the search/match DTOs live next to the plugin
//! manager in `codex::services::plugin::protocol`. Only the types that both
//! a repository and a service need to speak (manifest descriptors, capability
//! declarations, scope enums) live here so `db` can reference them without
//! taking a hard dependency on `services`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Plugin manifest declared by a plugin in its `manifest.json` and cached on
/// the plugin row.
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
}

impl Default for ReleaseSourceCapability {
    fn default() -> Self {
        Self {
            kinds: Vec::new(),
            requires_aliases: false,
            requires_external_ids: Vec::new(),
            can_announce_chapters: true,
            can_announce_volumes: true,
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
    /// ledger).
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
