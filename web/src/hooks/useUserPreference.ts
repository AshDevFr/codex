import { useCallback, useEffect } from "react";
import { useAuthStore } from "@/store/authStore";
import {
  selectIsLoaded,
  selectPreference,
  useUserPreferencesHydrated,
  useUserPreferencesStore,
} from "@/store/userPreferencesStore";
import type { PreferenceKey, TypedPreferences } from "@/types/preferences";

/**
 * Hook to access a single user preference with automatic loading and type safety.
 *
 * This hook provides:
 * - Type-safe access to preference values
 * - Automatic loading from server when authenticated
 * - Local caching with localStorage persistence
 * - Debounced sync to server on changes
 *
 * @example
 * ```tsx
 * function ThemeToggle() {
 *   const [theme, setTheme] = useUserPreference("ui.theme");
 *
 *   return (
 *     <Select
 *       value={theme}
 *       onChange={(value) => setTheme(value as typeof theme)}
 *     >
 *       <option value="system">System</option>
 *       <option value="light">Light</option>
 *       <option value="dark">Dark</option>
 *     </Select>
 *   );
 * }
 * ```
 */
export function useUserPreference<K extends PreferenceKey>(
  key: K,
): [TypedPreferences[K], (value: TypedPreferences[K]) => void] {
  const { isAuthenticated } = useAuthStore();
  const value = useUserPreferencesStore(selectPreference(key));
  const setPreference = useUserPreferencesStore((state) => state.setPreference);
  const loadFromServer = useUserPreferencesStore(
    (state) => state.loadFromServer,
  );
  const isLoaded = useUserPreferencesStore(selectIsLoaded);

  // Load preferences from server when authenticated and not yet loaded
  useEffect(() => {
    if (isAuthenticated && !isLoaded) {
      loadFromServer();
    }
  }, [isAuthenticated, isLoaded, loadFromServer]);

  const setValue = useCallback(
    (newValue: TypedPreferences[K]) => {
      setPreference(key, newValue);
    },
    [key, setPreference],
  );

  return [value, setValue];
}

/**
 * Hook to access all user preferences at once.
 *
 * This is useful for settings pages where you need to display/edit multiple preferences.
 *
 * @example
 * ```tsx
 * function SettingsPage() {
 *   const { preferences, isLoaded, setPreference, resetPreference } = useUserPreferences();
 *
 *   if (!isLoaded) return <Loader />;
 *
 *   return (
 *     <form>
 *       <Input
 *         label="Theme"
 *         value={preferences["ui.theme"] ?? "system"}
 *         onChange={(e) => setPreference("ui.theme", e.target.value)}
 *       />
 *     </form>
 *   );
 * }
 * ```
 */
export function useUserPreferences() {
  const { isAuthenticated } = useAuthStore();
  const preferences = useUserPreferencesStore((state) => state.preferences);
  const isLoaded = useUserPreferencesStore(selectIsLoaded);
  const loadError = useUserPreferencesStore((state) => state.loadError);
  const setPreference = useUserPreferencesStore((state) => state.setPreference);
  const resetPreference = useUserPreferencesStore(
    (state) => state.resetPreference,
  );
  const loadFromServer = useUserPreferencesStore(
    (state) => state.loadFromServer,
  );
  const getPreference = useUserPreferencesStore((state) => state.getPreference);
  const hasHydrated = useUserPreferencesHydrated();

  // Load preferences from server when authenticated and not yet loaded
  useEffect(() => {
    if (isAuthenticated && !isLoaded && hasHydrated) {
      loadFromServer();
    }
  }, [isAuthenticated, isLoaded, hasHydrated, loadFromServer]);

  return {
    /**
     * All cached preferences (may not include all keys if some use defaults)
     */
    preferences,
    /**
     * Whether preferences have been loaded from the server
     */
    isLoaded,
    /**
     * Whether the store has hydrated from localStorage
     */
    hasHydrated,
    /**
     * Error message if loading failed
     */
    loadError,
    /**
     * Get a preference value (returns default if not set)
     */
    getPreference,
    /**
     * Set a preference value
     */
    setPreference,
    /**
     * Reset a preference to its default value
     */
    resetPreference,
    /**
     * Reload all preferences from the server
     */
    reload: loadFromServer,
  };
}
