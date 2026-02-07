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
  userSyncProvider?: boolean;
  /**
   * External ID source used to match sync entries to Codex series.
   * When set, pulled sync entries are matched to series via the
   * `series_external_ids` table using this source string.
   *
   * Should use the `api:<service>` convention, e.g. "api:anilist".
   * Only meaningful when `userSyncProvider` is true.
   */
  externalIdSource?: string;
  /** Can provide recommendations */
  recommendationProvider?: boolean;
  /**
   * @deprecated Use userSyncProvider instead
   * Kept for backwards compatibility
   */
  syncProvider?: boolean;
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
   * This is displayed in the admin UI to help users configure the plugin.
   */
  configSchema?: ConfigSchema;
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

// =============================================================================
// Backwards Compatibility (deprecated)
// =============================================================================

/**
 * @deprecated Use PluginCapabilities with metadataProvider array instead
 */
export interface LegacyPluginCapabilities {
  /** @deprecated Use metadataProvider: ["series"] instead */
  seriesMetadataProvider?: boolean;
  syncProvider?: boolean;
  recommendationProvider?: boolean;
}
