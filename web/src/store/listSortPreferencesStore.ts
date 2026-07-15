import { create } from "zustand";
import { devtools, persist } from "zustand/middleware";
import type { CollectionSeriesSort, SortDirection } from "@/api/collections";
import type { ReadListBookSort } from "@/api/readlists";

/**
 * Per-list sort choice on the collection / read list detail pages, persisted
 * to localStorage so each list reopens in the order it was last viewed in.
 *
 * `sort: undefined` means "no explicit choice": the page sends no sort param
 * and the server applies the list's default (manual when `ordered`, title /
 * release date otherwise).
 */
export interface ListSortChoice<S> {
  sort?: S;
  direction?: SortDirection;
}

export interface ListSortPreferencesState {
  collections: Record<string, ListSortChoice<CollectionSeriesSort>>;
  readLists: Record<string, ListSortChoice<ReadListBookSort>>;
  setCollectionSort: (
    id: string,
    choice: ListSortChoice<CollectionSeriesSort>,
  ) => void;
  setReadListSort: (
    id: string,
    choice: ListSortChoice<ReadListBookSort>,
  ) => void;
}

export const useListSortPreferencesStore = create<ListSortPreferencesState>()(
  devtools(
    persist(
      (set) => ({
        collections: {},
        readLists: {},
        setCollectionSort: (id, choice) =>
          set((state) => ({
            collections: {
              ...state.collections,
              [id]: { ...state.collections[id], ...choice },
            },
          })),
        setReadListSort: (id, choice) =>
          set((state) => ({
            readLists: {
              ...state.readLists,
              [id]: { ...state.readLists[id], ...choice },
            },
          })),
      }),
      { name: "list-sort-preferences-storage" },
    ),
    { name: "ListSortPreferences", enabled: import.meta.env.DEV },
  ),
);
