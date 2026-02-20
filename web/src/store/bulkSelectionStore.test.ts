import { beforeEach, describe, expect, it } from "vitest";
import {
  selectCanSelectType,
  selectIsItemSelected,
  selectIsSelectionMode,
  selectSelectedIdsArray,
  selectSelectionCount,
  selectSelectionType,
  useBulkSelectionStore,
} from "./bulkSelectionStore";

describe("bulkSelectionStore", () => {
  beforeEach(() => {
    // Reset store state before each test
    useBulkSelectionStore.setState({
      selectedIds: new Set<string>(),
      selectionType: null,
      isSelectionMode: false,
      lastSelectedIndices: new Map<string, number>(),
      pageItems: null,
    });
  });

  describe("initial state", () => {
    it("should have empty selection", () => {
      const state = useBulkSelectionStore.getState();
      expect(state.selectedIds.size).toBe(0);
    });

    it("should have no selection type", () => {
      const state = useBulkSelectionStore.getState();
      expect(state.selectionType).toBeNull();
    });

    it("should not be in selection mode", () => {
      const state = useBulkSelectionStore.getState();
      expect(state.isSelectionMode).toBe(false);
    });

    it("should have empty lastSelectedIndices", () => {
      const state = useBulkSelectionStore.getState();
      expect(state.lastSelectedIndices.size).toBe(0);
    });
  });

  describe("toggleSelection", () => {
    it("should add item to selection", () => {
      const { toggleSelection, isSelected } = useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");

      expect(isSelected("book-1")).toBe(true);
    });

    it("should set selection type on first selection", () => {
      const { toggleSelection } = useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");

      const state = useBulkSelectionStore.getState();
      expect(state.selectionType).toBe("book");
    });

    it("should enter selection mode on first selection", () => {
      const { toggleSelection } = useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");

      const state = useBulkSelectionStore.getState();
      expect(state.isSelectionMode).toBe(true);
    });

    it("should deselect when toggling already selected item", () => {
      const { toggleSelection, isSelected } = useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");
      expect(isSelected("book-1")).toBe(true);

      toggleSelection("book-1", "book");
      expect(isSelected("book-1")).toBe(false);
    });

    it("should exit selection mode when last item is deselected", () => {
      const { toggleSelection } = useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");
      expect(useBulkSelectionStore.getState().isSelectionMode).toBe(true);

      toggleSelection("book-1", "book");
      const state = useBulkSelectionStore.getState();
      expect(state.isSelectionMode).toBe(false);
      expect(state.selectionType).toBeNull();
    });

    it("should clear lastSelectedIndices when exiting selection mode", () => {
      const { toggleSelection } = useBulkSelectionStore.getState();

      toggleSelection("book-1", "book", "grid-1", 0);
      expect(useBulkSelectionStore.getState().lastSelectedIndices.size).toBe(1);

      toggleSelection("book-1", "book");
      expect(useBulkSelectionStore.getState().lastSelectedIndices.size).toBe(0);
    });

    it("should not allow selecting different type when selection exists", () => {
      const { toggleSelection, isSelected } = useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");
      toggleSelection("series-1", "series");

      expect(isSelected("book-1")).toBe(true);
      expect(isSelected("series-1")).toBe(false);
      expect(useBulkSelectionStore.getState().selectionType).toBe("book");
    });

    it("should track lastSelectedIndex when gridId and index provided", () => {
      const { toggleSelection, getLastSelectedIndex } =
        useBulkSelectionStore.getState();

      toggleSelection("book-1", "book", "grid-1", 5);

      expect(getLastSelectedIndex("grid-1")).toBe(5);
    });

    it("should update lastSelectedIndex on subsequent selections", () => {
      const { toggleSelection, getLastSelectedIndex } =
        useBulkSelectionStore.getState();

      toggleSelection("book-1", "book", "grid-1", 5);
      toggleSelection("book-2", "book", "grid-1", 10);

      expect(getLastSelectedIndex("grid-1")).toBe(10);
    });

    it("should track indices per grid", () => {
      const { toggleSelection, getLastSelectedIndex } =
        useBulkSelectionStore.getState();

      toggleSelection("book-1", "book", "grid-1", 5);
      toggleSelection("book-2", "book", "grid-2", 10);

      expect(getLastSelectedIndex("grid-1")).toBe(5);
      expect(getLastSelectedIndex("grid-2")).toBe(10);
    });
  });

  describe("selectItem", () => {
    it("should add item to selection without toggling", () => {
      const { selectItem, isSelected } = useBulkSelectionStore.getState();

      selectItem("book-1", "book");
      expect(isSelected("book-1")).toBe(true);

      // Calling again should not remove it
      selectItem("book-1", "book");
      expect(isSelected("book-1")).toBe(true);
    });

    it("should not allow selecting different type", () => {
      const { selectItem, toggleSelection, isSelected } =
        useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");
      selectItem("series-1", "series");

      expect(isSelected("series-1")).toBe(false);
    });
  });

  describe("selectRange", () => {
    it("should add all items in range to selection", () => {
      const { selectRange, isSelected } = useBulkSelectionStore.getState();

      selectRange(["book-1", "book-2", "book-3"], "book");

      expect(isSelected("book-1")).toBe(true);
      expect(isSelected("book-2")).toBe(true);
      expect(isSelected("book-3")).toBe(true);
    });

    it("should set selection type on first range selection", () => {
      const { selectRange } = useBulkSelectionStore.getState();

      selectRange(["book-1", "book-2"], "book");

      expect(useBulkSelectionStore.getState().selectionType).toBe("book");
    });

    it("should enter selection mode on range selection", () => {
      const { selectRange } = useBulkSelectionStore.getState();

      selectRange(["book-1", "book-2"], "book");

      expect(useBulkSelectionStore.getState().isSelectionMode).toBe(true);
    });

    it("should not allow range selection of different type", () => {
      const { toggleSelection, selectRange, isSelected } =
        useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");
      selectRange(["series-1", "series-2"], "series");

      expect(isSelected("series-1")).toBe(false);
      expect(isSelected("series-2")).toBe(false);
    });

    it("should handle empty range", () => {
      const { selectRange } = useBulkSelectionStore.getState();

      selectRange([], "book");

      const state = useBulkSelectionStore.getState();
      expect(state.selectedIds.size).toBe(0);
      expect(state.isSelectionMode).toBe(false);
    });

    it("should merge with existing selection", () => {
      const { toggleSelection, selectRange, isSelected } =
        useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");
      selectRange(["book-2", "book-3"], "book");

      expect(isSelected("book-1")).toBe(true);
      expect(isSelected("book-2")).toBe(true);
      expect(isSelected("book-3")).toBe(true);
    });
  });

  describe("clearSelection", () => {
    it("should clear all selected items", () => {
      const { toggleSelection, clearSelection } =
        useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");
      toggleSelection("book-2", "book");

      clearSelection();

      const state = useBulkSelectionStore.getState();
      expect(state.selectedIds.size).toBe(0);
    });

    it("should reset selection type", () => {
      const { toggleSelection, clearSelection } =
        useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");
      clearSelection();

      expect(useBulkSelectionStore.getState().selectionType).toBeNull();
    });

    it("should exit selection mode", () => {
      const { toggleSelection, clearSelection } =
        useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");
      clearSelection();

      expect(useBulkSelectionStore.getState().isSelectionMode).toBe(false);
    });

    it("should clear lastSelectedIndices", () => {
      const { toggleSelection, clearSelection } =
        useBulkSelectionStore.getState();

      toggleSelection("book-1", "book", "grid-1", 5);
      clearSelection();

      expect(useBulkSelectionStore.getState().lastSelectedIndices.size).toBe(0);
    });
  });

  describe("isSelected", () => {
    it("should return true for selected item", () => {
      const { toggleSelection, isSelected } = useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");

      expect(isSelected("book-1")).toBe(true);
    });

    it("should return false for unselected item", () => {
      const { isSelected } = useBulkSelectionStore.getState();

      expect(isSelected("book-1")).toBe(false);
    });
  });

  describe("canSelect", () => {
    it("should return true when no selection exists", () => {
      const { canSelect } = useBulkSelectionStore.getState();

      expect(canSelect("book")).toBe(true);
      expect(canSelect("series")).toBe(true);
    });

    it("should return true for matching type", () => {
      const { toggleSelection, canSelect } = useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");

      expect(canSelect("book")).toBe(true);
    });

    it("should return false for non-matching type", () => {
      const { toggleSelection, canSelect } = useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");

      expect(canSelect("series")).toBe(false);
    });
  });

  describe("getLastSelectedIndex", () => {
    it("should return undefined for unknown grid", () => {
      const { getLastSelectedIndex } = useBulkSelectionStore.getState();

      expect(getLastSelectedIndex("unknown-grid")).toBeUndefined();
    });

    it("should return index for known grid", () => {
      const { toggleSelection, getLastSelectedIndex } =
        useBulkSelectionStore.getState();

      toggleSelection("book-1", "book", "grid-1", 5);

      expect(getLastSelectedIndex("grid-1")).toBe(5);
    });
  });

  describe("selectors", () => {
    describe("selectSelectionCount", () => {
      it("should return 0 for empty selection", () => {
        const state = useBulkSelectionStore.getState();
        expect(selectSelectionCount(state)).toBe(0);
      });

      it("should return correct count", () => {
        const { toggleSelection } = useBulkSelectionStore.getState();

        toggleSelection("book-1", "book");
        toggleSelection("book-2", "book");

        const state = useBulkSelectionStore.getState();
        expect(selectSelectionCount(state)).toBe(2);
      });
    });

    describe("selectIsSelectionMode", () => {
      it("should return false initially", () => {
        const state = useBulkSelectionStore.getState();
        expect(selectIsSelectionMode(state)).toBe(false);
      });

      it("should return true when items selected", () => {
        const { toggleSelection } = useBulkSelectionStore.getState();

        toggleSelection("book-1", "book");

        const state = useBulkSelectionStore.getState();
        expect(selectIsSelectionMode(state)).toBe(true);
      });
    });

    describe("selectSelectionType", () => {
      it("should return null initially", () => {
        const state = useBulkSelectionStore.getState();
        expect(selectSelectionType(state)).toBeNull();
      });

      it("should return correct type", () => {
        const { toggleSelection } = useBulkSelectionStore.getState();

        toggleSelection("series-1", "series");

        const state = useBulkSelectionStore.getState();
        expect(selectSelectionType(state)).toBe("series");
      });
    });

    describe("selectIsItemSelected", () => {
      it("should return false for unselected item", () => {
        const state = useBulkSelectionStore.getState();
        expect(selectIsItemSelected("book-1")(state)).toBe(false);
      });

      it("should return true for selected item", () => {
        const { toggleSelection } = useBulkSelectionStore.getState();

        toggleSelection("book-1", "book");

        const state = useBulkSelectionStore.getState();
        expect(selectIsItemSelected("book-1")(state)).toBe(true);
      });
    });

    describe("selectCanSelectType", () => {
      it("should return true when no selection", () => {
        const state = useBulkSelectionStore.getState();
        expect(selectCanSelectType("book")(state)).toBe(true);
        expect(selectCanSelectType("series")(state)).toBe(true);
      });

      it("should return true for matching type", () => {
        const { toggleSelection } = useBulkSelectionStore.getState();

        toggleSelection("book-1", "book");

        const state = useBulkSelectionStore.getState();
        expect(selectCanSelectType("book")(state)).toBe(true);
      });

      it("should return false for non-matching type", () => {
        const { toggleSelection } = useBulkSelectionStore.getState();

        toggleSelection("book-1", "book");

        const state = useBulkSelectionStore.getState();
        expect(selectCanSelectType("series")(state)).toBe(false);
      });
    });

    describe("selectSelectedIdsArray", () => {
      it("should return empty array for empty selection", () => {
        const state = useBulkSelectionStore.getState();
        expect(selectSelectedIdsArray(state)).toEqual([]);
      });

      it("should return array of selected ids", () => {
        const { toggleSelection } = useBulkSelectionStore.getState();

        toggleSelection("book-1", "book");
        toggleSelection("book-2", "book");

        const state = useBulkSelectionStore.getState();
        const ids = selectSelectedIdsArray(state);
        expect(ids).toHaveLength(2);
        expect(ids).toContain("book-1");
        expect(ids).toContain("book-2");
      });
    });
  });

  describe("type-locking scenarios", () => {
    it("should allow selecting multiple books", () => {
      const { toggleSelection, isSelected } = useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");
      toggleSelection("book-2", "book");
      toggleSelection("book-3", "book");

      expect(isSelected("book-1")).toBe(true);
      expect(isSelected("book-2")).toBe(true);
      expect(isSelected("book-3")).toBe(true);
    });

    it("should allow selecting multiple series", () => {
      const { toggleSelection, isSelected } = useBulkSelectionStore.getState();

      toggleSelection("series-1", "series");
      toggleSelection("series-2", "series");

      expect(isSelected("series-1")).toBe(true);
      expect(isSelected("series-2")).toBe(true);
    });

    it("should block series selection when books are selected", () => {
      const { toggleSelection, isSelected } = useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");
      toggleSelection("series-1", "series");

      expect(isSelected("book-1")).toBe(true);
      expect(isSelected("series-1")).toBe(false);
      expect(useBulkSelectionStore.getState().selectionType).toBe("book");
    });

    it("should block book selection when series are selected", () => {
      const { toggleSelection, isSelected } = useBulkSelectionStore.getState();

      toggleSelection("series-1", "series");
      toggleSelection("book-1", "book");

      expect(isSelected("series-1")).toBe(true);
      expect(isSelected("book-1")).toBe(false);
      expect(useBulkSelectionStore.getState().selectionType).toBe("series");
    });

    it("should allow different type after clearing selection", () => {
      const { toggleSelection, clearSelection, isSelected } =
        useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");
      clearSelection();
      toggleSelection("series-1", "series");

      expect(isSelected("series-1")).toBe(true);
      expect(useBulkSelectionStore.getState().selectionType).toBe("series");
    });
  });

  describe("selectAll", () => {
    it("should select all provided IDs", () => {
      const { selectAll, isSelected } = useBulkSelectionStore.getState();

      selectAll(["book-1", "book-2", "book-3"], "book");

      expect(isSelected("book-1")).toBe(true);
      expect(isSelected("book-2")).toBe(true);
      expect(isSelected("book-3")).toBe(true);
      expect(useBulkSelectionStore.getState().selectedIds.size).toBe(3);
    });

    it("should set selection type and enter selection mode", () => {
      const { selectAll } = useBulkSelectionStore.getState();

      selectAll(["series-1", "series-2"], "series");

      const state = useBulkSelectionStore.getState();
      expect(state.selectionType).toBe("series");
      expect(state.isSelectionMode).toBe(true);
    });

    it("should replace existing selection of the same type", () => {
      const { toggleSelection, selectAll, isSelected } =
        useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");
      selectAll(["book-2", "book-3"], "book");

      expect(isSelected("book-1")).toBe(false);
      expect(isSelected("book-2")).toBe(true);
      expect(isSelected("book-3")).toBe(true);
    });

    it("should not select if type conflicts with existing selection", () => {
      const { toggleSelection, selectAll, isSelected } =
        useBulkSelectionStore.getState();

      toggleSelection("book-1", "book");
      selectAll(["series-1", "series-2"], "series");

      expect(isSelected("book-1")).toBe(true);
      expect(isSelected("series-1")).toBe(false);
      expect(useBulkSelectionStore.getState().selectionType).toBe("book");
    });

    it("should handle empty array", () => {
      const { selectAll } = useBulkSelectionStore.getState();

      selectAll([], "book");

      const state = useBulkSelectionStore.getState();
      expect(state.selectedIds.size).toBe(0);
      expect(state.isSelectionMode).toBe(false);
    });

    it("should clear lastSelectedIndices", () => {
      const { toggleSelection, selectAll } = useBulkSelectionStore.getState();

      toggleSelection("book-1", "book", "grid-1", 0);
      expect(useBulkSelectionStore.getState().lastSelectedIndices.size).toBe(1);

      selectAll(["book-1", "book-2"], "book");
      expect(useBulkSelectionStore.getState().lastSelectedIndices.size).toBe(0);
    });
  });

  describe("setPageItems", () => {
    it("should store page items", () => {
      const { setPageItems } = useBulkSelectionStore.getState();

      setPageItems({ ids: ["book-1", "book-2"], type: "book" });

      expect(useBulkSelectionStore.getState().pageItems).toEqual({
        ids: ["book-1", "book-2"],
        type: "book",
      });
    });

    it("should clear page items with null", () => {
      const { setPageItems } = useBulkSelectionStore.getState();

      setPageItems({ ids: ["book-1"], type: "book" });
      setPageItems(null);

      expect(useBulkSelectionStore.getState().pageItems).toBeNull();
    });
  });
});
