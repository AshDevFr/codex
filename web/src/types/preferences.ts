/**
 * Type-safe user preference keys and values.
 *
 * This module provides TypeScript types for type-safe access to user preferences.
 * The API types (UserPreferenceDto, etc.) are auto-generated in api.generated.ts.
 */

/**
 * Map of all known preference keys to their value types.
 * Use this for type-safe preference access.
 */
export interface TypedPreferences {
  // UI preferences
  "ui.theme": "light" | "dark" | "system";

  // Library preferences
  "library.show_deleted_books": boolean;

  // Release-tracking preferences
  /**
   * Series IDs whose `release_announced` events should NOT bump the badge or
   * surface a toast for this user. The series detail page exposes a per-series
   * mute toggle that writes here; the Release Tracking settings page exposes
   * a "Clear all mutes" action that deletes the preference.
   */
  "release_tracking.muted_series_ids": string[];
}

/**
 * All valid preference keys
 */
export type PreferenceKey = keyof TypedPreferences;

/**
 * Default values for each preference key
 */
export const PREFERENCE_DEFAULTS: TypedPreferences = {
  "ui.theme": "system",
  "library.show_deleted_books": false,
  "release_tracking.muted_series_ids": [],
};

/**
 * Type guard to check if a key is a valid preference key
 */
export function isPreferenceKey(key: string): key is PreferenceKey {
  return key in PREFERENCE_DEFAULTS;
}

/**
 * Get the default value for a preference key
 */
export function getPreferenceDefault<K extends PreferenceKey>(
  key: K,
): TypedPreferences[K] {
  return PREFERENCE_DEFAULTS[key];
}
