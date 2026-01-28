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
  /** Can sync reading progress with external service */
  syncProvider?: boolean;
  /** Can provide recommendations */
  recommendationProvider?: boolean;
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
