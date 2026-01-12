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
	"ui.language": string;
	"ui.sidebar_collapsed": boolean;

	// Reader preferences
	"reader.default_zoom": number;
	"reader.reading_direction": "auto" | "ltr" | "rtl";
	"reader.page_fit": "width" | "height" | "contain" | "cover";

	// Notification preferences
	"notifications.email_enabled": boolean;
	"notifications.new_books": boolean;

	// Library preferences
	"library.default_view": "grid" | "list";
	"library.default_page_size": number;
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
	"ui.language": "en",
	"ui.sidebar_collapsed": false,
	"reader.default_zoom": 100,
	"reader.reading_direction": "auto",
	"reader.page_fit": "width",
	"notifications.email_enabled": true,
	"notifications.new_books": true,
	"library.default_view": "grid",
	"library.default_page_size": 24,
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
