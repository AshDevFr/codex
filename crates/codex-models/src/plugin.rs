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

impl PluginManifest {
    /// Whether this plugin requires *per-user* authentication.
    ///
    /// True when the plugin declares an OAuth flow or per-user required
    /// credentials, i.e. the user must connect an account / supply a secret
    /// before the plugin can act for them. False for credential-less plugins
    /// and for plugins that rely solely on an admin-configured shared key
    /// (which authenticates the plugin but does not identify the user). The
    /// host treats a no-per-user-auth plugin as "connected" once enabled, since
    /// there is nothing for the user to connect.
    pub fn requires_authentication(&self) -> bool {
        self.oauth.is_some() || !self.required_credentials.is_empty()
    }
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
    /// Whether this plugin consumes enriched series data (tags, genres,
    /// bibliographic metadata, custom metadata) on the entries it receives.
    /// When set, the host exposes the per-field `_codex.send*` toggles and only
    /// then attaches the opted-in data to sync/recommendation entries. Plugins
    /// that don't declare this never pay the assembly or payload cost.
    #[serde(default)]
    pub wants_full_metadata: bool,
    /// Whether this plugin consumes the per-book reading-progress breakdown
    /// (`SyncProgress.readBooks`) on the sync entries it receives. When set, the
    /// host fetches per-book volume/chapter/page detail and attaches it to push
    /// entries; plugins that don't declare it never pay the extra fetch or
    /// payload cost. Only meaningful when `user_read_sync` is true. The accurate
    /// `maxVolume`/`maxChapter` fields are always sent and are not gated by this.
    #[serde(default)]
    pub wants_detailed_progress: bool,
    /// Can announce new releases (chapters/volumes) for tracked series.
    /// When present, the plugin may invoke the `releases/*` reverse-RPC
    /// methods. The capability struct declares the data the plugin needs
    /// (aliases, external IDs) so the host can scope its responses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_source: Option<ReleaseSourceCapability>,
    /// Fronts a website users can jump to from a Codex series page.
    /// A pure manifest declaration: the host resolves `{config.<field>}`
    /// placeholders from the plugin's admin config and exposes the templates
    /// to the frontend; the plugin runtime is never involved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_links: Option<WebLinksCapability>,
}

/// Web-links capability declaration.
///
/// Templates carry two placeholder kinds: `{config.<field>}`, resolved
/// server-side from the plugin's stored admin config, and runtime
/// placeholders (`{title}` on the search template, `{externalId}` on series
/// links), resolved client-side per series with URL encoding.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebLinksCapability {
    /// Search page template, e.g. `{config.baseUrl}/search?q={title}`.
    pub search_url_template: String,
    /// Ordered direct-link templates; the first entry whose `source` the
    /// series has an external ID for wins. When none match, the frontend
    /// falls back to `search_url_template`.
    #[serde(default)]
    pub series_links: Vec<SeriesLinkTemplate>,
}

/// A per-source direct-link template for the web-links capability.
///
/// `source` uses the *bare Codex* source name (no `api:`/`plugin:` prefix);
/// the template itself carries the target site's own provider notation as a
/// literal, so the host never needs to translate names.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesLinkTemplate {
    /// Bare Codex external-ID source name, e.g. `mangabaka`, `myanimelist`.
    pub source: String,
    /// Full URL template for the series page on the target site.
    pub url_template: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities_wants_detailed_progress_defaults_false() {
        let caps: PluginCapabilities = serde_json::from_value(serde_json::json!({
            "userReadSync": true
        }))
        .unwrap();
        assert!(!caps.wants_detailed_progress);
    }

    #[test]
    fn test_capabilities_wants_detailed_progress_parses_true() {
        let caps: PluginCapabilities = serde_json::from_value(serde_json::json!({
            "userReadSync": true,
            "wantsDetailedProgress": true
        }))
        .unwrap();
        assert!(caps.wants_detailed_progress);
    }

    #[test]
    fn test_capabilities_web_links_defaults_none() {
        let caps: PluginCapabilities = serde_json::from_value(serde_json::json!({
            "userReadSync": true
        }))
        .unwrap();
        assert!(caps.web_links.is_none());
    }

    #[test]
    fn test_capabilities_web_links_parses_full_declaration() {
        let caps: PluginCapabilities = serde_json::from_value(serde_json::json!({
            "webLinks": {
                "searchUrlTemplate": "{config.baseUrl}/search?q={title}",
                "seriesLinks": [
                    {
                        "source": "mangabaka",
                        "urlTemplate": "{config.baseUrl}/series/lookup?source=mangabaka&id={externalId}"
                    },
                    {
                        "source": "myanimelist",
                        "urlTemplate": "{config.baseUrl}/series/lookup?source=mal&id={externalId}"
                    }
                ]
            }
        }))
        .unwrap();
        let web_links = caps.web_links.expect("webLinks should parse");
        assert_eq!(
            web_links.search_url_template,
            "{config.baseUrl}/search?q={title}"
        );
        // Order is significant: it doubles as match priority.
        assert_eq!(web_links.series_links.len(), 2);
        assert_eq!(web_links.series_links[0].source, "mangabaka");
        assert_eq!(web_links.series_links[1].source, "myanimelist");
        assert_eq!(
            web_links.series_links[1].url_template,
            "{config.baseUrl}/series/lookup?source=mal&id={externalId}"
        );
    }

    #[test]
    fn test_capabilities_web_links_series_links_optional() {
        let caps: PluginCapabilities = serde_json::from_value(serde_json::json!({
            "webLinks": { "searchUrlTemplate": "https://nyaa.si/?q={title}" }
        }))
        .unwrap();
        let web_links = caps.web_links.expect("webLinks should parse");
        assert!(web_links.series_links.is_empty());
    }

    #[test]
    fn test_capabilities_web_links_round_trips_camel_case() {
        let caps = PluginCapabilities {
            web_links: Some(WebLinksCapability {
                search_url_template: "{config.baseUrl}/search?q={title}".to_string(),
                series_links: vec![SeriesLinkTemplate {
                    source: "anilist".to_string(),
                    url_template: "{config.baseUrl}/series/lookup?source=anilist&id={externalId}"
                        .to_string(),
                }],
            }),
            ..PluginCapabilities::default()
        };
        let value = serde_json::to_value(&caps).unwrap();
        assert_eq!(
            value["webLinks"]["searchUrlTemplate"],
            "{config.baseUrl}/search?q={title}"
        );
        assert_eq!(value["webLinks"]["seriesLinks"][0]["source"], "anilist");
        assert_eq!(
            value["webLinks"]["seriesLinks"][0]["urlTemplate"],
            "{config.baseUrl}/series/lookup?source=anilist&id={externalId}"
        );

        let back: PluginCapabilities = serde_json::from_value(value).unwrap();
        assert_eq!(back.web_links.unwrap().series_links.len(), 1);
    }

    #[test]
    fn test_capabilities_without_web_links_skips_field_on_serialize() {
        let caps = PluginCapabilities::default();
        let value = serde_json::to_value(&caps).unwrap();
        assert!(value.get("webLinks").is_none());
    }

    fn manifest_from(extra: serde_json::Value) -> PluginManifest {
        let mut base = serde_json::json!({
            "name": "p",
            "displayName": "P",
            "version": "1.0.0",
            "protocolVersion": "1.0",
            "capabilities": { "userReadSync": true }
        });
        let obj = base.as_object_mut().unwrap();
        for (k, v) in extra.as_object().unwrap() {
            obj.insert(k.clone(), v.clone());
        }
        serde_json::from_value(base).unwrap()
    }

    #[test]
    fn test_requires_authentication_false_for_credentialless_plugin() {
        let manifest = manifest_from(serde_json::json!({}));
        assert!(!manifest.requires_authentication());
    }

    #[test]
    fn test_requires_authentication_true_with_required_credentials() {
        let manifest = manifest_from(serde_json::json!({
            "requiredCredentials": [
                { "key": "access_token", "label": "Token" }
            ]
        }));
        assert!(manifest.requires_authentication());
    }

    #[test]
    fn test_requires_authentication_true_with_oauth() {
        let manifest = manifest_from(serde_json::json!({
            "oauth": {
                "authorizationUrl": "https://example.com/auth",
                "tokenUrl": "https://example.com/token"
            }
        }));
        assert!(manifest.requires_authentication());
    }
}
