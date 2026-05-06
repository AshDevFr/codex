/**
 * Plugin manifest types - describes plugin capabilities and requirements
 */

import type { MetadataContentType } from "./capabilities.js";

/**
 * Credential field configuration for admin UI
 */
export interface CredentialField {
  /** Unique key for this credential (used in environment variables) */
  key: string;
  /** Human-readable label for the UI */
  label: string;
  /** Help text explaining where to get this credential */
  description?: string;
  /** Whether this credential is required */
  required: boolean;
  /** Whether to mask this value in the UI (for passwords/API keys) */
  sensitive: boolean;
  /** Input type for the UI */
  type: "text" | "password" | "url";
  /** Placeholder text */
  placeholder?: string;
}

/**
 * Source kinds a release-source plugin can expose.
 *
 * - `rss-uploader`: Per-uploader feed (e.g., a Nyaa user RSS feed).
 * - `rss-series`: Per-series feed (e.g., MangaUpdates RSS for a single series).
 * - `api-feed`: Generic API-driven feed.
 * - `metadata-feed`: Metadata-derived signal (informational; usually doesn't
 *   write the ledger).
 *
 * Mirrors the Rust `ReleaseSourceKind` enum (kebab-case on the wire).
 */
export type ReleaseSourceKind = "rss-uploader" | "rss-series" | "api-feed" | "metadata-feed";

/**
 * Release-source capability declaration.
 *
 * Declares both *what* the plugin can announce (chapters/volumes) and *what*
 * it needs from the host (aliases, external IDs). The host uses these fields
 * to scope `releases/list_tracked` responses so plugins only see data they
 * asked for.
 */
export interface ReleaseSourceCapability {
  /** Source kinds this plugin exposes. */
  kinds: ReleaseSourceKind[];
  /**
   * Whether the plugin needs title aliases (set when the plugin matches by
   * title rather than by external ID, e.g. Nyaa).
   */
  requiresAliases?: boolean;
  /**
   * External-ID sources the plugin needs, e.g. `["mangaupdates"]` or
   * `["mangadex"]`. The host filters `series_external_ids` to these sources
   * when responding to `releases/list_tracked`.
   */
  requiresExternalIds?: string[];
  /** Whether the plugin announces chapter-level releases. */
  canAnnounceChapters?: boolean;
  /** Whether the plugin announces volume-level releases. */
  canAnnounceVolumes?: boolean;
}

/**
 * Plugin capabilities
 */
export interface PluginCapabilities {
  /**
   * Content types this plugin can provide metadata for.
   * E.g., ["series"] or ["series", "book"]
   */
  metadataProvider?: MetadataContentType[];
  /** Can sync reading progress with external service (per-user) */
  userReadSync?: boolean;
  /**
   * External ID source used to match sync entries to Codex series.
   * When set, pulled sync entries are matched to series via the
   * `series_external_ids` table using this source string.
   *
   * Should use the `api:<service>` convention, e.g. "api:anilist".
   * Only meaningful when `userReadSync` is true.
   */
  externalIdSource?: string;
  /** Can provide recommendations */
  userRecommendationProvider?: boolean;
  /**
   * Release-source plugin capability. Set when this plugin announces new
   * chapter/volume releases for tracked series via `releases/poll`.
   */
  releaseSource?: ReleaseSourceCapability;
}

/**
 * Any value that can round-trip through JSON. Used for config field defaults
 * and examples, which the host carries as opaque `serde_json::Value` and
 * forwards verbatim to plugins.
 */
export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

/**
 * Configuration field definition for documenting plugin config options.
 *
 * `type` is a free-form hint, not a wire constraint. The host never validates
 * stored config against this schema — it forwards the raw JSON to the plugin,
 * which parses whatever shape it expects. Common values are `"number"`,
 * `"string"`, `"boolean"`, `"string-array"`, and `"object"`; renderers fall
 * back to a generic JSON editor for unrecognized types.
 */
export interface ConfigField {
  /** Field name (key in JSON config) */
  key: string;
  /** Human-readable label */
  label: string;
  /** Description of what this field does */
  description?: string;
  /** Field type — free-form hint; see interface docs for common values. */
  type: string;
  /** Whether this field is required */
  required?: boolean;
  /** Default value if not provided */
  default?: JsonValue;
  /** Example value for documentation */
  example?: JsonValue;
}

/**
 * Plugin configuration schema - documents available config options
 */
export interface ConfigSchema {
  /** Human-readable description of the configuration */
  description?: string;
  /** List of configuration fields */
  fields: ConfigField[];
}

/**
 * OAuth 2.0 configuration for user plugins requiring external service authentication.
 *
 * Codex handles the full OAuth flow (authorization URL, code exchange, token storage).
 * Plugins only need to declare their OAuth requirements here.
 */
export interface OAuthConfig {
  /** OAuth 2.0 authorization endpoint URL */
  authorizationUrl: string;
  /** OAuth 2.0 token endpoint URL */
  tokenUrl: string;
  /** Required OAuth scopes */
  scopes?: string[];
  /**
   * Whether to use PKCE (Proof Key for Code Exchange).
   * Recommended for public clients. Defaults to true.
   */
  pkce?: boolean;
  /** Optional user info endpoint URL (to fetch external identity after auth) */
  userInfoUrl?: string;
  /** Optional default OAuth client ID (can be overridden by admin in plugin config) */
  clientId?: string;
}

/**
 * Plugin manifest returned by the `initialize` method
 */
export interface PluginManifest {
  /** Unique plugin identifier (lowercase, alphanumeric, hyphens) */
  name: string;
  /** Human-readable name for UI display */
  displayName: string;
  /** Plugin version (semver) */
  version: string;
  /** Short description of what the plugin does */
  description: string;
  /** Author name or organization */
  author: string;
  /** Homepage URL (documentation, source code) */
  homepage?: string;
  /** Icon URL (optional, for UI display) */
  icon?: string;

  /** Protocol version this plugin implements */
  protocolVersion: "1.0" | "1.1";

  /** What this plugin can do */
  capabilities: PluginCapabilities;

  /** Credentials required from admin */
  requiredCredentials?: CredentialField[];

  /**
   * Configuration schema documenting available config options.
   * This is displayed in the admin UI to help administrators configure the plugin.
   */
  configSchema?: ConfigSchema;

  /**
   * Configuration schema for per-user settings.
   * Displayed in the user-facing Integrations settings modal.
   * Users can customize these fields per-account (stored in user_plugins.config).
   */
  userConfigSchema?: ConfigSchema;

  /**
   * OAuth 2.0 configuration for user plugins that require external service authentication.
   * When present, the Integrations UI shows "Connect with {name}" instead of "Enable".
   */
  oauth?: OAuthConfig;

  /** User-facing description shown when enabling the plugin */
  userDescription?: string;

  /** Admin-facing setup instructions (e.g., how to create OAuth app, configure client ID) */
  adminSetupInstructions?: string;

  /** User-facing setup instructions (e.g., how to connect or get a personal token) */
  userSetupInstructions?: string;

  /**
   * URI template for searching on the plugin's website.
   * Use `<title>` as a placeholder for the URL-encoded search query.
   * When present, the metadata search modal shows a "Search on {displayName}" button.
   * @example "https://mangabaka.org/search?sort_by=popularity_asc&q=<title>"
   */
  searchURITemplate?: string;
}

// =============================================================================
// Type Guards for Manifest Validation
// =============================================================================

/**
 * Type guard to check if manifest declares series metadata provider capability
 */
export function hasSeriesMetadataProvider(manifest: PluginManifest): manifest is PluginManifest & {
  capabilities: { metadataProvider: MetadataContentType[] };
} {
  return (
    Array.isArray(manifest.capabilities.metadataProvider) &&
    manifest.capabilities.metadataProvider.includes("series")
  );
}

/**
 * Type guard to check if manifest declares book metadata provider capability
 */
export function hasBookMetadataProvider(manifest: PluginManifest): manifest is PluginManifest & {
  capabilities: { metadataProvider: MetadataContentType[] };
} {
  return (
    Array.isArray(manifest.capabilities.metadataProvider) &&
    manifest.capabilities.metadataProvider.includes("book")
  );
}

/**
 * Type guard to check if manifest declares the release-source capability.
 */
export function hasReleaseSource(manifest: PluginManifest): manifest is PluginManifest & {
  capabilities: { releaseSource: ReleaseSourceCapability };
} {
  return (
    manifest.capabilities.releaseSource !== undefined &&
    manifest.capabilities.releaseSource !== null
  );
}
