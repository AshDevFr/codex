import { create } from "zustand";
import { devtools } from "zustand/middleware";

/**
 * Type of item that can be selected
 */
export type SelectionType = "book" | "series";

export interface BulkSelectionState {
  /**
   * Set of selected item IDs
   */
  selectedIds: Set<string>;

  /**
   * Type of items currently selected (null when no selection)
   * First selected item determines the type - type-locked selection
   */
  selectionType: SelectionType | null;

  /**
   * Whether selection mode is currently active
   * True when at least one item is selected
   */
  isSelectionMode: boolean;

  /**
   * Track last selected index per grid for shift+click range selection
   * Key is gridId (e.g., "books-library-123", "keep-reading")
   */
  lastSelectedIndices: Map<string, number>;

  // Actions

  /**
   * Toggle selection of a single item.
   * If this is the first selection, it sets the selection type.
   * If the item type doesn't match current selection type, it's a no-op.
   */
  toggleSelection: (
    id: string,
    type: SelectionType,
    gridId?: string,
    index?: number,
  ) => void;

  /**
   * Select a single item (without toggling off).
   * Used primarily for shift+click to add without removing.
   */
  selectItem: (id: string, type: SelectionType) => void;

  /**
   * Select a range of items (for shift+click).
   * All items must be of the same type as current selection.
   */
  selectRange: (ids: string[], type: SelectionType) => void;

  /**
   * Clear all selection and exit selection mode
   */
  clearSelection: () => void;

  /**
   * Check if a specific item is selected
   */
  isSelected: (id: string) => boolean;

  /**
   * Check if a specific type can be selected.
   * Returns true if no selection exists or if the type matches current selection.
   */
  canSelect: (type: SelectionType) => boolean;

  /**
   * Get the last selected index for a specific grid
   */
  getLastSelectedIndex: (gridId: string) => number | undefined;
}

export const useBulkSelectionStore = create<BulkSelectionState>()(
  devtools(
    (set, get) => ({
      selectedIds: new Set<string>(),
      selectionType: null,
      isSelectionMode: false,
      lastSelectedIndices: new Map<string, number>(),

      toggleSelection: (
        id: string,
        type: SelectionType,
        gridId?: string,
        index?: number,
      ) => {
        const state = get();

        // If type doesn't match current selection type (and we have a selection), do nothing
        if (state.selectionType !== null && state.selectionType !== type) {
          return;
        }

        const newSelectedIds = new Set(state.selectedIds);
        const newLastSelectedIndices = new Map(state.lastSelectedIndices);

        if (newSelectedIds.has(id)) {
          // Deselect
          newSelectedIds.delete(id);

          // If no items left, exit selection mode
          if (newSelectedIds.size === 0) {
            set({
              selectedIds: newSelectedIds,
              selectionType: null,
              isSelectionMode: false,
              lastSelectedIndices: new Map(),
            });
          } else {
            set({ selectedIds: newSelectedIds });
          }
        } else {
          // Select
          newSelectedIds.add(id);

          // Track last selected index for range selection
          if (gridId !== undefined && index !== undefined) {
            newLastSelectedIndices.set(gridId, index);
          }

          set({
            selectedIds: newSelectedIds,
            selectionType: type,
            isSelectionMode: true,
            lastSelectedIndices: newLastSelectedIndices,
          });
        }
      },

      selectItem: (id: string, type: SelectionType) => {
        const state = get();

        // If type doesn't match current selection type (and we have a selection), do nothing
        if (state.selectionType !== null && state.selectionType !== type) {
          return;
        }

        const newSelectedIds = new Set(state.selectedIds);
        newSelectedIds.add(id);

        set({
          selectedIds: newSelectedIds,
          selectionType: type,
          isSelectionMode: true,
        });
      },

      selectRange: (ids: string[], type: SelectionType) => {
        const state = get();

        // If type doesn't match current selection type (and we have a selection), do nothing
        if (state.selectionType !== null && state.selectionType !== type) {
          return;
        }

        if (ids.length === 0) {
          return;
        }

        const newSelectedIds = new Set(state.selectedIds);
        for (const id of ids) {
          newSelectedIds.add(id);
        }

        set({
          selectedIds: newSelectedIds,
          selectionType: type,
          isSelectionMode: true,
        });
      },

      clearSelection: () => {
        set({
          selectedIds: new Set(),
          selectionType: null,
          isSelectionMode: false,
          lastSelectedIndices: new Map(),
        });
      },

      isSelected: (id: string) => {
        return get().selectedIds.has(id);
      },

      canSelect: (type: SelectionType) => {
        const state = get();
        return state.selectionType === null || state.selectionType === type;
      },

      getLastSelectedIndex: (gridId: string) => {
        return get().lastSelectedIndices.get(gridId);
      },
    }),
    {
      name: "BulkSelection",
      enabled: import.meta.env.DEV,
    },
  ),
);

// =============================================================================
// Performance Selectors
// =============================================================================

/**
 * Select only the selection count.
 * Components using this will only re-render when the count changes.
 */
export const selectSelectionCount = (state: BulkSelectionState): number =>
  state.selectedIds.size;

/**
 * Select whether selection mode is active.
 * Components using this will only re-render when selection mode changes.
 */
export const selectIsSelectionMode = (state: BulkSelectionState): boolean =>
  state.isSelectionMode;

/**
 * Select the current selection type.
 * Components using this will only re-render when selection type changes.
 */
export const selectSelectionType = (
  state: BulkSelectionState,
): SelectionType | null => state.selectionType;

/**
 * Create a selector for checking if a specific item is selected.
 * Components using this will re-render when the selected state of that specific item changes.
 */
export const selectIsItemSelected =
  (id: string) =>
  (state: BulkSelectionState): boolean =>
    state.selectedIds.has(id);

// Pre-created selectors for canSelectType to avoid creating new functions on each render
const canSelectBookSelector = (state: BulkSelectionState): boolean =>
  state.selectionType === null || state.selectionType === "book";

const canSelectSeriesSelector = (state: BulkSelectionState): boolean =>
  state.selectionType === null || state.selectionType === "series";

/**
 * Get a memoized selector for checking if a specific type can be selected.
 * Returns a stable function reference for the given type.
 */
export const selectCanSelectType = (
  type: SelectionType,
): ((state: BulkSelectionState) => boolean) => {
  return type === "book" ? canSelectBookSelector : canSelectSeriesSelector;
};

/**
 * Select all selected IDs as an array.
 * Note: This creates a new array on each call, so use sparingly.
 */
export const selectSelectedIdsArray = (state: BulkSelectionState): string[] =>
  Array.from(state.selectedIds);
