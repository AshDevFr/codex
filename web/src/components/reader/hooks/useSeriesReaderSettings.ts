import { useCallback, useEffect, useMemo, useState } from "react";
import { useAuthStore } from "@/store/authStore";
import {
	type ForkableReaderSettings,
	type SeriesReaderOverride,
	createSeriesOverride,
	extractForkableSettings,
	isSeriesReaderOverride,
	useReaderStore,
} from "@/store/readerStore";

// =============================================================================
// Constants
// =============================================================================

/** Prefix for all reader settings localStorage keys */
export const STORAGE_KEY_PREFIX = "codex-reader-";

/** Suffix used for series-specific keys */
export const SERIES_KEY_SUFFIX = "-series-";

/** Anonymous user fallback ID */
const ANONYMOUS_USER_ID = "anonymous";

/** Custom event name for series settings updates */
const SERIES_SETTINGS_UPDATE_EVENT = "codex-series-settings-update";

// =============================================================================
// Storage Key Utilities
// =============================================================================

/**
 * Generate localStorage key for series-specific settings.
 * @param userId - User ID (or anonymous fallback)
 * @param seriesId - Series UUID
 */
export function getSeriesStorageKey(userId: string, seriesId: string): string {
	return `${STORAGE_KEY_PREFIX}${userId}-series-${seriesId}`;
}

/**
 * Get the current user ID from auth store, or fallback to anonymous.
 */
function useUserId(): string {
	const user = useAuthStore((state) => state.user);
	return user?.id ?? ANONYMOUS_USER_ID;
}

// =============================================================================
// localStorage Utilities
// =============================================================================

/**
 * Read a series override from localStorage.
 * Returns null if not found or invalid.
 */
function readSeriesOverride(storageKey: string): SeriesReaderOverride | null {
	try {
		const stored = localStorage.getItem(storageKey);
		if (!stored) return null;

		const parsed: unknown = JSON.parse(stored);
		if (!isSeriesReaderOverride(parsed)) {
			console.warn(`Invalid series override in localStorage: ${storageKey}`);
			return null;
		}

		return parsed;
	} catch (error) {
		console.warn(`Failed to read series override from localStorage: ${storageKey}`, error);
		return null;
	}
}

/**
 * Write a series override to localStorage.
 * Returns true on success, false on failure.
 * Dispatches a custom event to notify other hook instances.
 */
function writeSeriesOverride(storageKey: string, override: SeriesReaderOverride): boolean {
	try {
		localStorage.setItem(storageKey, JSON.stringify(override));
		// Dispatch custom event to sync other hook instances
		window.dispatchEvent(
			new CustomEvent(SERIES_SETTINGS_UPDATE_EVENT, { detail: { storageKey } }),
		);
		return true;
	} catch (error) {
		// Handle quota exceeded or other storage errors
		console.error(`Failed to write series override to localStorage: ${storageKey}`, error);
		return false;
	}
}

/**
 * Remove a series override from localStorage.
 * Dispatches a custom event to notify other hook instances.
 */
function removeSeriesOverride(storageKey: string): void {
	try {
		localStorage.removeItem(storageKey);
		// Dispatch custom event to sync other hook instances
		window.dispatchEvent(
			new CustomEvent(SERIES_SETTINGS_UPDATE_EVENT, { detail: { storageKey } }),
		);
	} catch (error) {
		console.warn(`Failed to remove series override from localStorage: ${storageKey}`, error);
	}
}

// =============================================================================
// Hook Types
// =============================================================================

export interface UseSeriesReaderSettingsReturn {
	/** Whether series-specific settings exist */
	hasSeriesOverride: boolean;

	/** The effective settings (series override merged with global) */
	effectiveSettings: ForkableReaderSettings;

	/** Create series override by forking current global settings */
	forkToSeries: () => void;

	/** Delete series override (return to global) */
	resetToGlobal: () => void;

	/** Update a specific setting (creates override if needed) */
	updateSetting: <K extends keyof ForkableReaderSettings>(
		key: K,
		value: ForkableReaderSettings[K],
	) => void;

	/** Whether hook has loaded from localStorage */
	isLoaded: boolean;

	/** The raw series override (null if using global) */
	seriesOverride: SeriesReaderOverride | null;
}

// =============================================================================
// Main Hook
// =============================================================================

/**
 * Hook for managing per-series reader settings.
 *
 * When a seriesId is provided, this hook checks localStorage for series-specific
 * overrides. If found, those settings are merged with global settings.
 * Otherwise, global settings are returned.
 *
 * @param seriesId - The series ID to manage settings for (null/undefined = global only)
 */
export function useSeriesReaderSettings(
	seriesId: string | null | undefined,
): UseSeriesReaderSettingsReturn {
	const userId = useUserId();
	const globalSettings = useReaderStore((state) => state.settings);

	// Local state for series override
	const [seriesOverride, setSeriesOverride] = useState<SeriesReaderOverride | null>(null);
	const [isLoaded, setIsLoaded] = useState(false);

	// Compute storage key
	const storageKey = useMemo(() => {
		if (!seriesId) return null;
		return getSeriesStorageKey(userId, seriesId);
	}, [userId, seriesId]);

	// Load series override from localStorage on mount or when key changes
	useEffect(() => {
		if (!storageKey) {
			setSeriesOverride(null);
			setIsLoaded(true);
			return;
		}

		const override = readSeriesOverride(storageKey);
		setSeriesOverride(override);
		setIsLoaded(true);

		// Listen for updates from other hook instances
		const handleSettingsUpdate = (event: Event) => {
			const customEvent = event as CustomEvent<{ storageKey: string }>;
			if (customEvent.detail?.storageKey === storageKey) {
				const updatedOverride = readSeriesOverride(storageKey);
				setSeriesOverride(updatedOverride);
			}
		};

		window.addEventListener(SERIES_SETTINGS_UPDATE_EVENT, handleSettingsUpdate);
		return () => {
			window.removeEventListener(SERIES_SETTINGS_UPDATE_EVENT, handleSettingsUpdate);
		};
	}, [storageKey]);

	// Compute effective settings (series override merged with global)
	const effectiveSettings = useMemo((): ForkableReaderSettings => {
		const globalForkable = extractForkableSettings(globalSettings);

		if (!seriesOverride) {
			return globalForkable;
		}

		// Series override takes precedence for all forkable settings
		return {
			fitMode: seriesOverride.fitMode,
			pageLayout: seriesOverride.pageLayout,
			readingDirection: seriesOverride.readingDirection,
			backgroundColor: seriesOverride.backgroundColor,
			doublePageShowWideAlone: seriesOverride.doublePageShowWideAlone,
			doublePageStartOnOdd: seriesOverride.doublePageStartOnOdd,
		};
	}, [globalSettings, seriesOverride]);

	// Fork current global settings to create a series override
	const forkToSeries = useCallback(() => {
		if (!storageKey) {
			console.warn("Cannot fork to series: no seriesId provided");
			return;
		}

		const globalForkable = extractForkableSettings(globalSettings);
		const newOverride = createSeriesOverride(globalForkable);

		if (writeSeriesOverride(storageKey, newOverride)) {
			setSeriesOverride(newOverride);
		}
	}, [storageKey, globalSettings]);

	// Reset series settings to use global defaults
	const resetToGlobal = useCallback(() => {
		if (!storageKey) return;

		removeSeriesOverride(storageKey);
		setSeriesOverride(null);
	}, [storageKey]);

	// Update a specific setting
	const updateSetting = useCallback(
		<K extends keyof ForkableReaderSettings>(key: K, value: ForkableReaderSettings[K]) => {
			if (!storageKey) {
				// No series context - update global store directly
				const setters: Record<keyof ForkableReaderSettings, (v: unknown) => void> = {
					fitMode: useReaderStore.getState().setFitMode,
					pageLayout: useReaderStore.getState().setPageLayout,
					readingDirection: useReaderStore.getState().setReadingDirection,
					backgroundColor: useReaderStore.getState().setBackgroundColor,
					doublePageShowWideAlone: useReaderStore.getState().setDoublePageShowWideAlone,
					doublePageStartOnOdd: useReaderStore.getState().setDoublePageStartOnOdd,
				};
				setters[key](value);
				return;
			}

			// Update or create series override
			const currentOverride = seriesOverride;
			let newOverride: SeriesReaderOverride;

			if (currentOverride) {
				// Update existing override
				newOverride = {
					...currentOverride,
					[key]: value,
				};
			} else {
				// Create new override based on current global settings
				const globalForkable = extractForkableSettings(globalSettings);
				newOverride = createSeriesOverride({
					...globalForkable,
					[key]: value,
				});
			}

			if (writeSeriesOverride(storageKey, newOverride)) {
				setSeriesOverride(newOverride);
			}
		},
		[storageKey, seriesOverride, globalSettings],
	);

	return {
		hasSeriesOverride: seriesOverride !== null,
		effectiveSettings,
		forkToSeries,
		resetToGlobal,
		updateSetting,
		isLoaded,
		seriesOverride,
	};
}

// =============================================================================
// localStorage Cleanup Utilities
// =============================================================================

/**
 * Information about a series settings entry in localStorage.
 */
export interface SeriesSettingsEntry {
	/** The full localStorage key */
	key: string;
	/** The series ID extracted from the key */
	seriesId: string;
	/** The stored override data (null if corrupted/invalid) */
	data: SeriesReaderOverride | null;
	/** When the override was created */
	createdAt: number | null;
}

/**
 * Result of a cleanup operation.
 */
export interface CleanupResult {
	/** Number of entries removed */
	removed: number;
	/** Keys that were removed */
	removedKeys: string[];
	/** Number of entries that failed to remove */
	errors: number;
}

/**
 * Get all series settings keys for a specific user from localStorage.
 * @param userId - User ID to find settings for
 * @returns Array of SeriesSettingsEntry objects
 */
export function getSeriesSettingsForUser(userId: string): SeriesSettingsEntry[] {
	const entries: SeriesSettingsEntry[] = [];
	const keyPrefix = `${STORAGE_KEY_PREFIX}${userId}${SERIES_KEY_SUFFIX}`;

	try {
		for (let i = 0; i < localStorage.length; i++) {
			const key = localStorage.key(i);
			if (!key || !key.startsWith(keyPrefix)) continue;

			// Extract series ID from key
			const seriesId = key.slice(keyPrefix.length);
			if (!seriesId) continue;

			// Try to read the data
			const data = readSeriesOverride(key);

			entries.push({
				key,
				seriesId,
				data,
				createdAt: data?.createdAt ?? null,
			});
		}
	} catch (error) {
		console.warn("Failed to enumerate localStorage keys:", error);
	}

	return entries;
}

/**
 * Remove series settings that are not in the provided valid series ID list.
 * This is useful for cleaning up settings for series that have been deleted.
 *
 * @param userId - User ID to clean up settings for
 * @param validSeriesIds - Set of series IDs that are still valid
 * @returns Cleanup result with counts and removed keys
 */
export function cleanupOrphanedSeriesSettings(
	userId: string,
	validSeriesIds: Set<string>,
): CleanupResult {
	const entries = getSeriesSettingsForUser(userId);
	const result: CleanupResult = {
		removed: 0,
		removedKeys: [],
		errors: 0,
	};

	for (const entry of entries) {
		if (!validSeriesIds.has(entry.seriesId)) {
			try {
				localStorage.removeItem(entry.key);
				result.removed++;
				result.removedKeys.push(entry.key);
			} catch (error) {
				console.warn(`Failed to remove orphaned series settings: ${entry.key}`, error);
				result.errors++;
			}
		}
	}

	return result;
}

/**
 * Remove all series settings for a specific user.
 * Useful when a user logs out or wants to reset all customizations.
 *
 * @param userId - User ID to clear all series settings for
 * @returns Cleanup result with counts and removed keys
 */
export function clearAllSeriesSettings(userId: string): CleanupResult {
	const entries = getSeriesSettingsForUser(userId);
	const result: CleanupResult = {
		removed: 0,
		removedKeys: [],
		errors: 0,
	};

	for (const entry of entries) {
		try {
			localStorage.removeItem(entry.key);
			result.removed++;
			result.removedKeys.push(entry.key);
		} catch (error) {
			console.warn(`Failed to remove series settings: ${entry.key}`, error);
			result.errors++;
		}
	}

	return result;
}

/**
 * Remove series settings that have invalid/corrupted data.
 *
 * @param userId - User ID to clean up settings for
 * @returns Cleanup result with counts and removed keys
 */
export function cleanupCorruptedSeriesSettings(userId: string): CleanupResult {
	const entries = getSeriesSettingsForUser(userId);
	const result: CleanupResult = {
		removed: 0,
		removedKeys: [],
		errors: 0,
	};

	for (const entry of entries) {
		// If data is null, it means the entry is corrupted/invalid
		if (entry.data === null) {
			try {
				localStorage.removeItem(entry.key);
				result.removed++;
				result.removedKeys.push(entry.key);
			} catch (error) {
				console.warn(`Failed to remove corrupted series settings: ${entry.key}`, error);
				result.errors++;
			}
		}
	}

	return result;
}
