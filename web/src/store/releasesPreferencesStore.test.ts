import { beforeEach, describe, expect, it } from "vitest";
import {
  buildReleaseSortParam,
  DEFAULT_RELEASE_SORT_DIRECTION,
  DEFAULT_RELEASE_SORT_FIELD,
  useReleasesPreferencesStore,
} from "./releasesPreferencesStore";

describe("releasesPreferencesStore", () => {
  beforeEach(() => {
    // Reset to defaults between cases; the store persists to localStorage.
    useReleasesPreferencesStore.setState({
      sortField: DEFAULT_RELEASE_SORT_FIELD,
      sortDirection: DEFAULT_RELEASE_SORT_DIRECTION,
    });
  });

  it("defaults to series ascending (matching the server default)", () => {
    const { sortField, sortDirection } = useReleasesPreferencesStore.getState();
    expect(sortField).toBe("series");
    expect(sortDirection).toBe("asc");
  });

  it("flips direction when toggling the active column", () => {
    const { toggleSort } = useReleasesPreferencesStore.getState();
    toggleSort("series");
    expect(useReleasesPreferencesStore.getState().sortDirection).toBe("desc");
    toggleSort("series");
    expect(useReleasesPreferencesStore.getState().sortDirection).toBe("asc");
  });

  it("switches column with that column's natural default direction", () => {
    const { toggleSort } = useReleasesPreferencesStore.getState();
    toggleSort("observed");
    const state = useReleasesPreferencesStore.getState();
    // Observed defaults to newest-first.
    expect(state.sortField).toBe("observed");
    expect(state.sortDirection).toBe("desc");
  });

  it("defaults the released column to newest-first (desc)", () => {
    const { toggleSort } = useReleasesPreferencesStore.getState();
    toggleSort("released");
    const state = useReleasesPreferencesStore.getState();
    expect(state.sortField).toBe("released");
    expect(state.sortDirection).toBe("desc");
  });

  it("builds the API sort param as field,direction", () => {
    expect(buildReleaseSortParam("observed", "desc")).toBe("observed,desc");
    expect(buildReleaseSortParam("released", "desc")).toBe("released,desc");
    expect(buildReleaseSortParam("series", "asc")).toBe("series,asc");
  });
});
