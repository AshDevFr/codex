import { useEffect, useState } from "react";
import { create } from "zustand";
import { devtools, persist } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";
import {
	type UserPreferenceDto,
	userPreferencesApi,
} from "@/api/userPreferences";
import {
	getPreferenceDefault,
	PREFERENCE_DEFAULTS,
	type PreferenceKey,
	type TypedPreferences,
} from "@/types/preferences";

// Debounce timeout for syncing to server (ms)
const SYNC_DEBOUNCE_MS = 500;

// Map to track pending sync operations
const pendingSyncs = new Map<string, NodeJS.Timeout>();

export interface UserPreferencesState {
	/**
	 * Cache of preference values from the server
	 */
	preferences: Partial<TypedPreferences>;

	/**
	 * Whether preferences have been loaded from the server
	 */
	isLoaded: boolean;

	/**
	 * Whether there's an error loading preferences
	 */
	loadError: string | null;

	/**
	 * Get a preference value with type safety
	 * Returns the cached value or default if not set
	 */
	getPreference: <K extends PreferenceKey>(key: K) => TypedPreferences[K];

	/**
	 * Set a preference value (updates local cache and syncs to server)
	 */
	setPreference: <K extends PreferenceKey>(
		key: K,
		value: TypedPreferences[K],
	) => void;

	/**
	 * Reset a preference to its default value
	 */
	resetPreference: (key: PreferenceKey) => void;

	/**
	 * Load all preferences from the server
	 */
	loadFromServer: () => Promise<void>;

	/**
	 * Clear all cached preferences (used on logout)
	 */
	clearCache: () => void;
}

/**
 * Parse a preference value from the API response into the correct type
 */
function parsePreferenceValue<K extends PreferenceKey>(
	_key: K,
	dto: UserPreferenceDto,
): TypedPreferences[K] {
	const value = dto.value;

	// Return value as-is if it matches the expected type
	// The server stores values in their native types
	return value as TypedPreferences[K];
}

/**
 * Sync a single preference to the server with debouncing
 */
function syncToServer<K extends PreferenceKey>(
	key: K,
	value: TypedPreferences[K],
): void {
	// Clear any pending sync for this key
	const pendingTimeout = pendingSyncs.get(key);
	if (pendingTimeout) {
		clearTimeout(pendingTimeout);
	}

	// Schedule a new sync
	const timeout = setTimeout(async () => {
		try {
			await userPreferencesApi.set(key, value);
			pendingSyncs.delete(key);
		} catch (error) {
			console.error(`Failed to sync preference ${key}:`, error);
			// Could add error state here if needed
		}
	}, SYNC_DEBOUNCE_MS);

	pendingSyncs.set(key, timeout);
}

export const useUserPreferencesStore = create<UserPreferencesState>()(
	devtools(
		persist(
			immer((set, get) => ({
				preferences: {},
				isLoaded: false,
				loadError: null,

				getPreference: <K extends PreferenceKey>(
					key: K,
				): TypedPreferences[K] => {
					const cached = get().preferences[key];
					if (cached !== undefined) {
						return cached as TypedPreferences[K];
					}
					return getPreferenceDefault(key);
				},

				setPreference: <K extends PreferenceKey>(
					key: K,
					value: TypedPreferences[K],
				) => {
					set((state) => {
						state.preferences[key] = value;
					});

					// Sync to server (debounced)
					syncToServer(key, value);
				},

				resetPreference: (key: PreferenceKey) => {
					set((state) => {
						delete state.preferences[key];
					});

					// Delete from server
					userPreferencesApi.delete(key).catch((error) => {
						console.error(`Failed to reset preference ${key}:`, error);
					});
				},

				loadFromServer: async () => {
					try {
						const dtos = await userPreferencesApi.getAll();

						set((state) => {
							// Clear existing preferences
							state.preferences = {};

							// Parse and set each preference
							for (const dto of dtos) {
								const key = dto.key as PreferenceKey;
								// Only process known preference keys
								if (key in PREFERENCE_DEFAULTS) {
									// Type assertion needed because TypeScript can't narrow the generic type
									(state.preferences as Record<string, unknown>)[key] =
										parsePreferenceValue(key, dto);
								}
							}

							state.isLoaded = true;
							state.loadError = null;
						});
					} catch (error) {
						const message =
							error instanceof Error
								? error.message
								: "Failed to load preferences";
						set((state) => {
							state.loadError = message;
						});
						console.error("Failed to load preferences from server:", error);
					}
				},

				clearCache: () => {
					// Cancel any pending syncs
					for (const timeout of pendingSyncs.values()) {
						clearTimeout(timeout);
					}
					pendingSyncs.clear();

					set((state) => {
						state.preferences = {};
						state.isLoaded = false;
						state.loadError = null;
					});
				},
			})),
			{
				name: "user-preferences-storage",
				partialize: (state) => ({
					preferences: state.preferences,
				}),
			},
		),
		{
			name: "UserPreferences",
			enabled: import.meta.env.DEV,
		},
	),
);

// =============================================================================
// Performance Selectors
// =============================================================================

/**
 * Select a specific preference value.
 * Components using this will only re-render when THIS preference changes.
 */
export const selectPreference =
	<K extends PreferenceKey>(key: K) =>
	(state: UserPreferencesState): TypedPreferences[K] => {
		const cached = state.preferences[key];
		if (cached !== undefined) {
			return cached as TypedPreferences[K];
		}
		return getPreferenceDefault(key);
	};

/**
 * Select whether preferences have been loaded from the server.
 */
export const selectIsLoaded = (state: UserPreferencesState): boolean =>
	state.isLoaded;

/**
 * Select the load error, if any.
 */
export const selectLoadError = (state: UserPreferencesState): string | null =>
	state.loadError;

// =============================================================================
// Hydration Hook
// =============================================================================

/**
 * Hook that returns true once the store has finished hydrating from localStorage.
 * Use this to prevent flash of default values before persisted state loads.
 */
export function useUserPreferencesHydrated(): boolean {
	const [hasHydrated, setHasHydrated] = useState(
		useUserPreferencesStore.persist.hasHydrated(),
	);

	useEffect(() => {
		const unsub = useUserPreferencesStore.persist.onFinishHydration(() => {
			setHasHydrated(true);
		});
		return unsub;
	}, []);

	return hasHydrated;
}
