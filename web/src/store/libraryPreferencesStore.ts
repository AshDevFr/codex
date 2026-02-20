import { useEffect, useState } from "react";
import { create } from "zustand";
import { devtools, persist } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";

/** Default number of items per page for paginated views */
export const DEFAULT_PAGE_SIZE = 50;

/** Maximum number of items per page allowed by the API */
export const MAX_PAGE_SIZE = 500;

export interface TabPreferences {
  pageSize?: number;
  sort?: string;
  filters?: Record<string, string>;
}

export interface LibraryPreferences {
  lastTab: string;
  tabs: {
    [tabName: string]: TabPreferences;
  };
}

export interface LibraryPreferencesState {
  libraries: Record<string, LibraryPreferences>;

  // Actions
  getLastTab: (libraryId: string) => string;
  setLastTab: (libraryId: string, tab: string) => void;

  getTabPreferences: (
    libraryId: string,
    tab: string,
  ) => TabPreferences | undefined;
  setTabPreferences: (
    libraryId: string,
    tab: string,
    preferences: TabPreferences,
  ) => void;

  clearLibraryPreferences: (libraryId: string) => void;
}

const VALID_TABS = ["recommended", "series", "books", "series-books"];
const DEFAULT_TAB = "recommended";

export const useLibraryPreferencesStore = create<LibraryPreferencesState>()(
  devtools(
    persist(
      immer((set, get) => ({
        libraries: {},

        getLastTab: (libraryId: string) => {
          const library = get().libraries[libraryId];
          return library?.lastTab || DEFAULT_TAB;
        },

        setLastTab: (libraryId: string, tab: string) => {
          // Validate tab name
          if (!VALID_TABS.includes(tab)) {
            console.warn(
              `Invalid tab "${tab}" for library ${libraryId}. Using default.`,
            );
            tab = DEFAULT_TAB;
          }

          set((state) => {
            // Immer allows mutation syntax - much cleaner!
            if (!state.libraries[libraryId]) {
              state.libraries[libraryId] = {
                lastTab: DEFAULT_TAB,
                tabs: {},
              };
            }
            state.libraries[libraryId].lastTab = tab;
          });
        },

        getTabPreferences: (libraryId: string, tab: string) => {
          const library = get().libraries[libraryId];
          return library?.tabs[tab];
        },

        setTabPreferences: (
          libraryId: string,
          tab: string,
          preferences: TabPreferences,
        ) => {
          set((state) => {
            // Immer mutation syntax - cleaner than spread operators
            if (!state.libraries[libraryId]) {
              state.libraries[libraryId] = {
                lastTab: DEFAULT_TAB,
                tabs: {},
              };
            }

            if (!state.libraries[libraryId].tabs[tab]) {
              state.libraries[libraryId].tabs[tab] = {};
            }

            // Merge preferences
            Object.assign(state.libraries[libraryId].tabs[tab], preferences);
          });
        },

        clearLibraryPreferences: (libraryId: string) => {
          set((state) => {
            delete state.libraries[libraryId];
          });
        },
      })),
      {
        name: "library-preferences-storage",
        partialize: (state) => ({
          libraries: state.libraries,
        }),
      },
    ),
    {
      name: "LibraryPreferences", // Shows in Redux DevTools
      enabled: import.meta.env.DEV, // Only in development
    },
  ),
);

// ============================================================================
// Performance Selectors
// ============================================================================
// These selectors allow components to subscribe to specific slices of state,
// preventing unnecessary re-renders when unrelated data changes.

/**
 * Select the last active tab for a specific library.
 * Components using this will only re-render when the tab changes for THIS library.
 */
export const selectLastTab =
  (libraryId: string) => (state: LibraryPreferencesState) => {
    return state.libraries[libraryId]?.lastTab ?? DEFAULT_TAB;
  };

/**
 * Select tab preferences for a specific library and tab.
 * Components using this will only re-render when preferences change for THIS specific tab.
 */
export const selectTabPreferences =
  (libraryId: string, tab: string) => (state: LibraryPreferencesState) => {
    return state.libraries[libraryId]?.tabs[tab];
  };

/**
 * Select page size for a specific library and tab.
 */
export const selectPageSize =
  (libraryId: string, tab: string) => (state: LibraryPreferencesState) => {
    return state.libraries[libraryId]?.tabs[tab]?.pageSize;
  };

/**
 * Select sort preference for a specific library and tab.
 */
export const selectSort =
  (libraryId: string, tab: string) => (state: LibraryPreferencesState) => {
    return state.libraries[libraryId]?.tabs[tab]?.sort;
  };

/**
 * Select filters for a specific library and tab.
 */
export const selectFilters =
  (libraryId: string, tab: string) => (state: LibraryPreferencesState) => {
    return state.libraries[libraryId]?.tabs[tab]?.filters;
  };

/**
 * Check if a library has any custom preferences set.
 * Useful for showing indicators or badges.
 */
export const selectHasCustomPreferences =
  (libraryId: string) => (state: LibraryPreferencesState) => {
    const library = state.libraries[libraryId];
    if (!library) return false;

    return (
      library.lastTab !== DEFAULT_TAB || Object.keys(library.tabs).length > 0
    );
  };

// ============================================================================
// Hydration Hook
// ============================================================================

/**
 * Hook that returns true once the store has finished hydrating from localStorage.
 * Use this to prevent flash of default values before persisted state loads.
 *
 * @example
 * function MyComponent() {
 *   const hasHydrated = useLibraryPreferencesHydrated();
 *   if (!hasHydrated) return <Loader />;
 *   // ... render with persisted preferences
 * }
 */
export function useLibraryPreferencesHydrated(): boolean {
  const [hasHydrated, setHasHydrated] = useState(
    useLibraryPreferencesStore.persist.hasHydrated(),
  );

  useEffect(() => {
    const unsub = useLibraryPreferencesStore.persist.onFinishHydration(() => {
      setHasHydrated(true);
    });
    return unsub;
  }, []);

  return hasHydrated;
}
