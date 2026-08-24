import type { Book } from "@/types";

/** Numeric-aware collation, so "Chapter 9" precedes "Chapter 10". */
const collator = new Intl.Collator(undefined, {
  numeric: true,
  sensitivity: "base",
});

export type SortableBook = Pick<Book, "number" | "title" | "titleSort">;

function titleKey(book: SortableBook): string {
  return book.titleSort ?? book.title;
}

/**
 * Compare two books by their position in a series.
 *
 * A book has no number until the renumber pass assigns one, and because that
 * pass runs behind the analysis queue, a library being scanned can hold
 * un-numbered books for the whole scan. Those sort last in both directions
 * rather than collapsing to 0, which would float them above the first chapter.
 *
 * Books that share a number, or that both lack one, fall back to title order,
 * matching the `number, title_sort, title` ordering the API already applies.
 *
 * Ordering by (has number, number, title) keeps the comparison transitive; a
 * comparator that switched between number and title order depending on the
 * pair could report a < b < c < a and leave the result to the sort engine.
 */
export function compareBooksByNumber(
  a: SortableBook,
  b: SortableBook,
  direction: "asc" | "desc" = "asc",
): number {
  const aNumber = a.number ?? null;
  const bNumber = b.number ?? null;

  if ((aNumber === null) !== (bNumber === null)) {
    return aNumber === null ? 1 : -1;
  }

  const sign = direction === "desc" ? -1 : 1;

  if (aNumber !== null && bNumber !== null && aNumber !== bNumber) {
    return sign * (aNumber - bNumber);
  }

  return sign * collator.compare(titleKey(a), titleKey(b));
}
