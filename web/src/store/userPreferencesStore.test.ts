import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { userPreferencesApi } from "@/api/userPreferences";
import { PREFERENCE_DEFAULTS } from "@/types/preferences";
import {
  selectIsLoaded,
  selectPreference,
  useUserPreferencesHydrated,
  useUserPreferencesStore,
} from "./userPreferencesStore";

// Mock the API client
vi.mock("@/api/userPreferences", () => ({
  userPreferencesApi: {
    getAll: vi.fn(),
    get: vi.fn(),
    set: vi.fn(),
    bulkSet: vi.fn(),
    delete: vi.fn(),
  },
}));

describe("userPreferencesStore", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    // Reset store state before each test
    useUserPreferencesStore.setState({
      preferences: {},
      isLoaded: false,
      loadError: null,
    });
    localStorage.clear();
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  describe("initial state", () => {
    it("should have empty preferences", () => {
      const state = useUserPreferencesStore.getState();
      expect(state.preferences).toEqual({});
    });

    it("should not be loaded initially", () => {
      const state = useUserPreferencesStore.getState();
      expect(state.isLoaded).toBe(false);
    });

    it("should have no load error", () => {
      const state = useUserPreferencesStore.getState();
      expect(state.loadError).toBeNull();
    });
  });

  describe("getPreference", () => {
    it("should return default value when preference not set", () => {
      const { getPreference } = useUserPreferencesStore.getState();
      expect(getPreference("ui.theme")).toBe(PREFERENCE_DEFAULTS["ui.theme"]);
      expect(getPreference("library.show_deleted_books")).toBe(
        PREFERENCE_DEFAULTS["library.show_deleted_books"],
      );
    });

    it("should return cached value when preference is set", () => {
      useUserPreferencesStore.setState({
        preferences: { "ui.theme": "dark" },
      });

      const { getPreference } = useUserPreferencesStore.getState();
      expect(getPreference("ui.theme")).toBe("dark");
    });
  });

  describe("setPreference", () => {
    it("should update preference in state", () => {
      const { setPreference, getPreference } =
        useUserPreferencesStore.getState();

      setPreference("ui.theme", "dark");

      expect(getPreference("ui.theme")).toBe("dark");
    });

    it("should sync to server with debounce", async () => {
      vi.mocked(userPreferencesApi.set).mockResolvedValue({
        key: "ui.theme",
        value: "dark",
        valueType: "string",
        updatedAt: new Date().toISOString(),
      });

      const { setPreference } = useUserPreferencesStore.getState();

      setPreference("ui.theme", "dark");

      // Should not sync immediately
      expect(userPreferencesApi.set).not.toHaveBeenCalled();

      // Advance timers to trigger debounced sync
      await act(async () => {
        vi.advanceTimersByTime(600);
      });

      expect(userPreferencesApi.set).toHaveBeenCalledWith("ui.theme", "dark");
    });

    it("should debounce multiple rapid changes", async () => {
      vi.mocked(userPreferencesApi.set).mockResolvedValue({
        key: "ui.theme",
        value: "light",
        valueType: "string",
        updatedAt: new Date().toISOString(),
      });

      const { setPreference } = useUserPreferencesStore.getState();

      // Make rapid changes
      setPreference("ui.theme", "dark");
      setPreference("ui.theme", "system");
      setPreference("ui.theme", "light");

      // Advance timers
      await act(async () => {
        vi.advanceTimersByTime(600);
      });

      // Should only sync the final value once
      expect(userPreferencesApi.set).toHaveBeenCalledTimes(1);
      expect(userPreferencesApi.set).toHaveBeenCalledWith("ui.theme", "light");
    });
  });

  describe("resetPreference", () => {
    it("should remove preference from state", () => {
      // Mock delete to return a Promise
      vi.mocked(userPreferencesApi.delete).mockResolvedValue(undefined);

      useUserPreferencesStore.setState({
        preferences: { "ui.theme": "dark" },
      });

      const { resetPreference, getPreference } =
        useUserPreferencesStore.getState();

      resetPreference("ui.theme");

      // Should return default after reset
      expect(getPreference("ui.theme")).toBe(PREFERENCE_DEFAULTS["ui.theme"]);
    });

    it("should delete from server", () => {
      vi.mocked(userPreferencesApi.delete).mockResolvedValue(undefined);

      useUserPreferencesStore.setState({
        preferences: { "ui.theme": "dark" },
      });

      const { resetPreference } = useUserPreferencesStore.getState();
      resetPreference("ui.theme");

      expect(userPreferencesApi.delete).toHaveBeenCalledWith("ui.theme");
    });
  });

  describe("loadFromServer", () => {
    it("should load preferences from server", async () => {
      vi.mocked(userPreferencesApi.getAll).mockResolvedValue([
        {
          key: "ui.theme",
          value: "dark",
          valueType: "string",
          updatedAt: new Date().toISOString(),
        },
        {
          key: "library.show_deleted_books",
          value: true,
          valueType: "boolean",
          updatedAt: new Date().toISOString(),
        },
      ]);

      const { loadFromServer, getPreference } =
        useUserPreferencesStore.getState();

      await act(async () => {
        await loadFromServer();
      });

      expect(getPreference("ui.theme")).toBe("dark");
      expect(getPreference("library.show_deleted_books")).toBe(true);
    });

    it("should set isLoaded to true after loading", async () => {
      vi.mocked(userPreferencesApi.getAll).mockResolvedValue([]);

      const { loadFromServer } = useUserPreferencesStore.getState();

      await act(async () => {
        await loadFromServer();
      });

      expect(useUserPreferencesStore.getState().isLoaded).toBe(true);
    });

    it("should set loadError on failure", async () => {
      // Suppress expected console.error for this test
      const consoleErrorSpy = vi
        .spyOn(console, "error")
        .mockImplementation(() => {});

      vi.mocked(userPreferencesApi.getAll).mockRejectedValue(
        new Error("Network error"),
      );

      const { loadFromServer } = useUserPreferencesStore.getState();

      await act(async () => {
        await loadFromServer();
      });

      expect(useUserPreferencesStore.getState().loadError).toBe(
        "Network error",
      );

      consoleErrorSpy.mockRestore();
    });

    it("should ignore unknown preference keys", async () => {
      vi.mocked(userPreferencesApi.getAll).mockResolvedValue([
        {
          key: "unknown.key",
          value: "test",
          valueType: "string",
          updatedAt: new Date().toISOString(),
        },
      ]);

      const { loadFromServer } = useUserPreferencesStore.getState();

      await act(async () => {
        await loadFromServer();
      });

      expect(useUserPreferencesStore.getState().preferences).toEqual({});
    });
  });

  describe("clearCache", () => {
    it("should clear all preferences", () => {
      useUserPreferencesStore.setState({
        preferences: { "ui.theme": "dark", "library.show_deleted_books": true },
        isLoaded: true,
      });

      const { clearCache } = useUserPreferencesStore.getState();
      clearCache();

      const state = useUserPreferencesStore.getState();
      expect(state.preferences).toEqual({});
      expect(state.isLoaded).toBe(false);
    });
  });

  describe("selectors", () => {
    describe("selectPreference", () => {
      it("should select a specific preference", () => {
        useUserPreferencesStore.setState({
          preferences: { "ui.theme": "dark" },
        });

        const state = useUserPreferencesStore.getState();
        const result = selectPreference("ui.theme")(state);
        expect(result).toBe("dark");
      });

      it("should return default for unset preference", () => {
        const state = useUserPreferencesStore.getState();
        const result = selectPreference("ui.theme")(state);
        expect(result).toBe(PREFERENCE_DEFAULTS["ui.theme"]);
      });
    });

    describe("selectIsLoaded", () => {
      it("should select isLoaded state", () => {
        useUserPreferencesStore.setState({ isLoaded: true });

        const state = useUserPreferencesStore.getState();
        expect(selectIsLoaded(state)).toBe(true);
      });
    });
  });

  describe("persistence", () => {
    it("should persist preferences to localStorage", () => {
      const { setPreference } = useUserPreferencesStore.getState();

      setPreference("ui.theme", "dark");

      const stored = localStorage.getItem("user-preferences-storage");
      expect(stored).toBeTruthy();

      const parsed = JSON.parse(stored as string);
      expect(parsed.state.preferences["ui.theme"]).toBe("dark");
    });
  });

  describe("useUserPreferencesHydrated", () => {
    it("should return true when store is hydrated", async () => {
      // Use real timers for hydration tests
      vi.useRealTimers();

      const { result } = renderHook(() => useUserPreferencesHydrated());

      // The store should hydrate quickly in tests (synchronous localStorage)
      await waitFor(() => {
        expect(result.current).toBe(true);
      });
    });

    it("should work with persisted data", async () => {
      // Use real timers for hydration tests
      vi.useRealTimers();

      // Pre-populate localStorage with preferences
      const persistedState = {
        state: {
          preferences: {
            "ui.theme": "dark",
          },
        },
        version: 0,
      };
      localStorage.setItem(
        "user-preferences-storage",
        JSON.stringify(persistedState),
      );

      // Rehydrate the store
      await act(async () => {
        await useUserPreferencesStore.persist.rehydrate();
      });

      const { result } = renderHook(() => useUserPreferencesHydrated());

      await waitFor(() => {
        expect(result.current).toBe(true);
      });

      // Verify persisted data was loaded
      const { getPreference } = useUserPreferencesStore.getState();
      expect(getPreference("ui.theme")).toBe("dark");
    });
  });
});
