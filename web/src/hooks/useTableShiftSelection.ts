import { useCallback, useRef, useState } from "react";

/**
 * Selection state + handlers for a checkbox-driven table where shift+click
 * extends the selection over the contiguous range from the previous click.
 *
 * Range semantics match the GitHub / GMail convention: the anchor is the row
 * the user clicked *without* shift, and shift+click applies the anchor's
 * resulting state (selected vs. deselected) to every row between the anchor
 * and the shift-clicked row. The anchor stays put so subsequent shift-clicks
 * keep extending from the same point.
 *
 * `entries` is the ordered list currently rendered in the table. If the anchor
 * has scrolled or paginated out of view, the handler falls back to a plain
 * toggle so the click is never lost.
 */
export interface UseTableShiftSelectionResult {
  selected: Set<string>;
  toggleOne: (id: string, shiftKey: boolean) => void;
  toggleAll: () => void;
  clear: () => void;
}

export function useTableShiftSelection<T extends { id: string }>(
  entries: readonly T[],
): UseTableShiftSelectionResult {
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const anchorRef = useRef<{ id: string; selectedAfter: boolean } | null>(null);

  const clear = useCallback(() => {
    setSelected(new Set());
    anchorRef.current = null;
  }, []);

  const toggleOne = useCallback(
    (id: string, shiftKey: boolean) => {
      setSelected((prev) => {
        const next = new Set(prev);
        const anchor = anchorRef.current;
        if (shiftKey && anchor && anchor.id !== id) {
          const a = entries.findIndex((e) => e.id === anchor.id);
          const b = entries.findIndex((e) => e.id === id);
          if (a !== -1 && b !== -1) {
            const [lo, hi] = a < b ? [a, b] : [b, a];
            for (let i = lo; i <= hi; i++) {
              if (anchor.selectedAfter) next.add(entries[i].id);
              else next.delete(entries[i].id);
            }
            // Anchor intentionally stays so the user can keep extending the
            // range from the same starting point.
            return next;
          }
        }
        if (next.has(id)) next.delete(id);
        else next.add(id);
        anchorRef.current = { id, selectedAfter: next.has(id) };
        return next;
      });
    },
    [entries],
  );

  const toggleAll = useCallback(() => {
    setSelected((prev) => {
      const allSelected =
        entries.length > 0 && entries.every((e) => prev.has(e.id));
      const next = new Set(prev);
      if (allSelected) {
        for (const e of entries) next.delete(e.id);
      } else {
        for (const e of entries) next.add(e.id);
      }
      return next;
    });
    anchorRef.current = null;
  }, [entries]);

  return { selected, toggleOne, toggleAll, clear };
}
