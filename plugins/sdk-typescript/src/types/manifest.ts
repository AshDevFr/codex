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
}

/**
 * Configuration field definition for documenting plugin config options
 */
export interface ConfigField {
  /** Field name (key in JSON config) */
  key: string;
  /** Human-readable label */
  label: string;
  /** Description of what this field does */
  description?: string;
  /** Field type */
  type: "number" | "string" | "boolean";
  /** Whether this field is required */
  required?: boolean;
  /** Default value if not provided */
  default?: number | string | boolean;
  /** Example value for documentation */
  example?: number | string | boolean;
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
  protocolVersion: "1.0";

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
