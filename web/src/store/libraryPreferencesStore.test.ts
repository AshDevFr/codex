import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  useLibraryPreferencesHydrated,
  useLibraryPreferencesStore,
} from "./libraryPreferencesStore";

describe("libraryPreferencesStore", () => {
  beforeEach(() => {
    // Reset store state before each test
    useLibraryPreferencesStore.setState({
      libraries: {},
    });
    localStorage.clear();
  });

  describe("initial state", () => {
    it("should have empty libraries object", () => {
      const state = useLibraryPreferencesStore.getState();
      expect(state.libraries).toEqual({});
    });
  });

  describe("lastTab", () => {
    it("should return default tab when no preference exists", () => {
      const { getLastTab } = useLibraryPreferencesStore.getState();
      expect(getLastTab("library-1")).toBe("recommended");
    });

    it("should set and get last tab", () => {
      const { setLastTab, getLastTab } = useLibraryPreferencesStore.getState();

      setLastTab("library-1", "books");

      expect(getLastTab("library-1")).toBe("books");
    });

    it("should validate tab names and use default for invalid tabs", () => {
      const { setLastTab, getLastTab } = useLibraryPreferencesStore.getState();
      const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

      setLastTab("library-1", "invalid-tab");

      expect(getLastTab("library-1")).toBe("recommended");
      expect(warnSpy).toHaveBeenCalledWith(
        'Invalid tab "invalid-tab" for library library-1. Using default.',
      );
      warnSpy.mockRestore();
    });

    it("should store tabs independently per library", () => {
      const { setLastTab, getLastTab } = useLibraryPreferencesStore.getState();

      setLastTab("library-1", "books");
      setLastTab("library-2", "series");

      expect(getLastTab("library-1")).toBe("books");
      expect(getLastTab("library-2")).toBe("series");
    });
  });

  describe("tabPreferences", () => {
    it("should return undefined when no preferences exist", () => {
      const { getTabPreferences } = useLibraryPreferencesStore.getState();
      expect(getTabPreferences("library-1", "books")).toBeUndefined();
    });

    it("should set and get tab preferences", () => {
      const { setTabPreferences, getTabPreferences } =
        useLibraryPreferencesStore.getState();

      setTabPreferences("library-1", "books", {
        pageSize: 50,
        sort: "title,asc",
      });

      const prefs = getTabPreferences("library-1", "books");
      expect(prefs).toEqual({
        pageSize: 50,
        sort: "title,asc",
      });
    });

    it("should merge preferences when updating", () => {
      const { setTabPreferences, getTabPreferences } =
        useLibraryPreferencesStore.getState();

      // Set initial preferences
      setTabPreferences("library-1", "books", {
        pageSize: 50,
      });

      // Update with additional preferences
      setTabPreferences("library-1", "books", {
        sort: "title,desc",
      });

      const prefs = getTabPreferences("library-1", "books");
      expect(prefs).toEqual({
        pageSize: 50,
        sort: "title,desc",
      });
    });

    it("should store preferences independently per tab", () => {
      const { setTabPreferences, getTabPreferences } =
        useLibraryPreferencesStore.getState();

      setTabPreferences("library-1", "books", { pageSize: 50 });
      setTabPreferences("library-1", "series", { pageSize: 25 });

      expect(getTabPreferences("library-1", "books")?.pageSize).toBe(50);
      expect(getTabPreferences("library-1", "series")?.pageSize).toBe(25);
    });

    it("should store filters", () => {
      const { setTabPreferences, getTabPreferences } =
        useLibraryPreferencesStore.getState();

      setTabPreferences("library-1", "books", {
        filters: { status: "reading", format: "cbz" },
      });

      const prefs = getTabPreferences("library-1", "books");
      expect(prefs?.filters).toEqual({ status: "reading", format: "cbz" });
    });

    it("should store and merge viewMode without clobbering other fields", () => {
      const { setTabPreferences, getTabPreferences } =
        useLibraryPreferencesStore.getState();

      setTabPreferences("library-1", "series-books", {
        pageSize: 50,
        sort: "number,asc",
      });
      setTabPreferences("library-1", "series-books", { viewMode: "table" });

      const prefs = getTabPreferences("library-1", "series-books");
      expect(prefs).toEqual({
        pageSize: 50,
        sort: "number,asc",
        viewMode: "table",
      });
    });

    it("should preserve viewMode when other preferences change", () => {
      const { setTabPreferences, getTabPreferences } =
        useLibraryPreferencesStore.getState();

      setTabPreferences("library-1", "series-books", { viewMode: "table" });
      setTabPreferences("library-1", "series-books", { pageSize: 100 });

      expect(getTabPreferences("library-1", "series-books")?.viewMode).toBe(
        "table",
      );
    });
  });

  describe("clearLibraryPreferences", () => {
    it("should clear all preferences for a library", () => {
      const {
        setLastTab,
        setTabPreferences,
        clearLibraryPreferences,
        getLastTab,
        getTabPreferences,
      } = useLibraryPreferencesStore.getState();

      // Set some preferences
      setLastTab("library-1", "books");
      setTabPreferences("library-1", "books", { pageSize: 50 });

      // Clear them
      clearLibraryPreferences("library-1");

      // Should return defaults
      expect(getLastTab("library-1")).toBe("recommended");
      expect(getTabPreferences("library-1", "books")).toBeUndefined();
    });

    it("should not affect other libraries", () => {
      const { setLastTab, clearLibraryPreferences, getLastTab } =
        useLibraryPreferencesStore.getState();

      setLastTab("library-1", "books");
      setLastTab("library-2", "series");

      clearLibraryPreferences("library-1");

      expect(getLastTab("library-1")).toBe("recommended");
      expect(getLastTab("library-2")).toBe("series");
    });
  });

  describe("persistence", () => {
    it("should persist preferences to localStorage", () => {
      const { setTabPreferences } = useLibraryPreferencesStore.getState();

      setTabPreferences("library-1", "books", { pageSize: 50 });

      const stored = localStorage.getItem("library-preferences-storage");
      expect(stored).toBeTruthy();

      const parsed = JSON.parse(stored as string);
      expect(parsed.state.libraries["library-1"].tabs.books.pageSize).toBe(50);
    });
  });

  describe("useLibraryPreferencesHydrated", () => {
    it("should return true when store is hydrated", async () => {
      const { result } = renderHook(() => useLibraryPreferencesHydrated());

      // The store should hydrate quickly in tests (synchronous localStorage)
      await waitFor(() => {
        expect(result.current).toBe(true);
      });
    });

    it("should work with persisted data", async () => {
      // Pre-populate localStorage with preferences
      const persistedState = {
        state: {
          libraries: {
            "library-1": {
              lastTab: "books",
              tabs: {
                books: { pageSize: 100 },
              },
            },
          },
        },
        version: 0,
      };
      localStorage.setItem(
        "library-preferences-storage",
        JSON.stringify(persistedState),
      );

      // Rehydrate the store
      await act(async () => {
        await useLibraryPreferencesStore.persist.rehydrate();
      });

      const { result } = renderHook(() => useLibraryPreferencesHydrated());

      await waitFor(() => {
        expect(result.current).toBe(true);
      });

      // Verify persisted data was loaded
      const { getTabPreferences } = useLibraryPreferencesStore.getState();
      expect(getTabPreferences("library-1", "books")?.pageSize).toBe(100);
    });
  });
});
