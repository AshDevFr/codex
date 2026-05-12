import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useTableShiftSelection } from "./useTableShiftSelection";

interface Row {
  id: string;
}

const rows = (...ids: string[]): Row[] => ids.map((id) => ({ id }));

describe("useTableShiftSelection", () => {
  it("toggles a single id when shiftKey is false", () => {
    const { result } = renderHook(() =>
      useTableShiftSelection(rows("a", "b", "c")),
    );

    act(() => result.current.toggleOne("b", false));
    expect(Array.from(result.current.selected)).toEqual(["b"]);

    act(() => result.current.toggleOne("b", false));
    expect(result.current.selected.size).toBe(0);
  });

  it("selects a contiguous range when shift+clicking forward", () => {
    const { result } = renderHook(() =>
      useTableShiftSelection(rows("a", "b", "c", "d", "e")),
    );

    // Anchor: select "b" with a normal click.
    act(() => result.current.toggleOne("b", false));
    // Shift+click "d" — should fill in "c" and "d".
    act(() => result.current.toggleOne("d", true));

    expect(Array.from(result.current.selected).sort()).toEqual(["b", "c", "d"]);
  });

  it("selects a contiguous range when shift+clicking backward", () => {
    const { result } = renderHook(() =>
      useTableShiftSelection(rows("a", "b", "c", "d", "e")),
    );

    act(() => result.current.toggleOne("d", false));
    act(() => result.current.toggleOne("a", true));

    expect(Array.from(result.current.selected).sort()).toEqual([
      "a",
      "b",
      "c",
      "d",
    ]);
  });

  it("mirrors anchor state on shift+click — deselecting a range", () => {
    const { result } = renderHook(() =>
      useTableShiftSelection(rows("a", "b", "c", "d")),
    );

    // Pre-select everything.
    act(() => result.current.toggleAll());
    expect(result.current.selected.size).toBe(4);

    // Click "a" without shift — deselects it, sets anchor to deselected.
    act(() => result.current.toggleOne("a", false));
    // Shift+click "c" — range should now be deselected.
    act(() => result.current.toggleOne("c", true));

    expect(Array.from(result.current.selected).sort()).toEqual(["d"]);
  });

  it("keeps the anchor stable across successive shift+clicks", () => {
    const { result } = renderHook(() =>
      useTableShiftSelection(rows("a", "b", "c", "d", "e")),
    );

    act(() => result.current.toggleOne("b", false)); // anchor = b (selected)
    act(() => result.current.toggleOne("c", true)); // range b..c
    expect(Array.from(result.current.selected).sort()).toEqual(["b", "c"]);

    // Second shift+click should extend from the original anchor "b",
    // not from the most recent shift target "c".
    act(() => result.current.toggleOne("e", true));
    expect(Array.from(result.current.selected).sort()).toEqual([
      "b",
      "c",
      "d",
      "e",
    ]);
  });

  it("falls back to plain toggle when the anchor is no longer visible", () => {
    // Anchor on "b", then re-render with a different entry list that
    // excludes "b" (simulates paging or filtering away the anchor).
    const { result, rerender } = renderHook(
      ({ entries }: { entries: Row[] }) => useTableShiftSelection(entries),
      { initialProps: { entries: rows("a", "b", "c") } },
    );

    act(() => result.current.toggleOne("b", false));
    rerender({ entries: rows("x", "y", "z") });

    // Shift+clicking "z" can't expand a range (anchor "b" isn't in view).
    // Should degrade to a plain toggle so the click isn't lost.
    act(() => result.current.toggleOne("z", true));
    expect(result.current.selected.has("z")).toBe(true);
  });

  it("clear() drops selection and resets the anchor", () => {
    const { result } = renderHook(() =>
      useTableShiftSelection(rows("a", "b", "c")),
    );

    act(() => result.current.toggleOne("a", false));
    act(() => result.current.clear());
    expect(result.current.selected.size).toBe(0);

    // After clear(), a shift+click should not expand from a stale anchor.
    act(() => result.current.toggleOne("c", true));
    expect(Array.from(result.current.selected)).toEqual(["c"]);
  });

  it("toggleAll selects every visible entry and toggles off on second call", () => {
    const { result } = renderHook(() =>
      useTableShiftSelection(rows("a", "b", "c")),
    );

    act(() => result.current.toggleAll());
    expect(result.current.selected.size).toBe(3);

    act(() => result.current.toggleAll());
    expect(result.current.selected.size).toBe(0);
  });
});
