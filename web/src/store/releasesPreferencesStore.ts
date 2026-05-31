import { create } from "zustand";
import { devtools, persist } from "zustand/middleware";
import type { ReleaseSortDirection, ReleaseSortField } from "@/api/releases";

/**
 * Default sort for the releases inbox: group by series name ascending. Mirrors
 * the server-side default so the first load matches what the API returns when
 * no `sort` param is sent.
 */
export const DEFAULT_RELEASE_SORT_FIELD: ReleaseSortField = "series";
export const DEFAULT_RELEASE_SORT_DIRECTION: ReleaseSortDirection = "asc";

/** Direction applied when the user first switches to a given column. */
const DEFAULT_DIRECTION_FOR_FIELD: Record<
  ReleaseSortField,
  ReleaseSortDirection
> = {
  // Most recently detected first when sorting by detection date.
  observed: "desc",
  // Newest release date first when sorting by release date.
  released: "desc",
  // Alphabetical A→Z when grouping by series.
  series: "asc",
};

export interface ReleasesPreferencesState {
  sortField: ReleaseSortField;
  sortDirection: ReleaseSortDirection;
  /**
   * Toggle sorting on a column. Clicking the active column flips the
   * direction; clicking a different column switches to it with that column's
   * natural default direction.
   */
  toggleSort: (field: ReleaseSortField) => void;
}

export const useReleasesPreferencesStore = create<ReleasesPreferencesState>()(
  devtools(
    persist(
      (set) => ({
        sortField: DEFAULT_RELEASE_SORT_FIELD,
        sortDirection: DEFAULT_RELEASE_SORT_DIRECTION,
        toggleSort: (field) =>
          set((state) =>
            state.sortField === field
              ? {
                  sortDirection: state.sortDirection === "asc" ? "desc" : "asc",
                }
              : {
                  sortField: field,
                  sortDirection: DEFAULT_DIRECTION_FOR_FIELD[field],
                },
          ),
      }),
      { name: "releases-preferences-storage" },
    ),
    { name: "ReleasesPreferences", enabled: import.meta.env.DEV },
  ),
);

/** Build the API `sort` query value (`"field,direction"`) from the store. */
export function buildReleaseSortParam(
  field: ReleaseSortField,
  direction: ReleaseSortDirection,
): string {
  return `${field},${direction}`;
}
