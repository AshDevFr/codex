import { describe, expect, it } from "vitest";
import { compareBooksByNumber, type SortableBook } from "./bookSort";

const book = (
  number: number | null,
  title: string,
  titleSort?: string | null,
): SortableBook => ({ number, title, titleSort });

describe("compareBooksByNumber", () => {
  it("orders numbered books by number", () => {
    const books = [book(10, "Chapter 010"), book(2, "Chapter 002")];
    books.sort((a, b) => compareBooksByNumber(a, b));

    expect(books.map((b) => b.number)).toEqual([2, 10]);
  });

  // A book has no number until the renumber pass runs, which for a library
  // being scanned lasts the whole scan. Coercing that null to 0 put brand new
  // books above chapter 1.
  it("keeps un-numbered books last rather than treating them as zero", () => {
    const books = [
      book(null, "Chapter 060"),
      book(1, "Chapter 001"),
      book(null, "Chapter 059"),
      book(2, "Chapter 002"),
    ];
    books.sort((a, b) => compareBooksByNumber(a, b));

    expect(books.map((b) => b.title)).toEqual([
      "Chapter 001",
      "Chapter 002",
      "Chapter 059",
      "Chapter 060",
    ]);
  });

  it("keeps un-numbered books last when sorting descending", () => {
    const books = [
      book(null, "Chapter 059"),
      book(1, "Chapter 001"),
      book(2, "Chapter 002"),
    ];
    books.sort((a, b) => compareBooksByNumber(a, b, "desc"));

    expect(books.map((b) => b.title)).toEqual([
      "Chapter 002",
      "Chapter 001",
      "Chapter 059",
    ]);
  });

  it("breaks ties on title the way the API orders them", () => {
    const books = [book(32, "Chapter 070"), book(32, "Chapter 032")];
    books.sort((a, b) => compareBooksByNumber(a, b));

    expect(books.map((b) => b.title)).toEqual(["Chapter 032", "Chapter 070"]);
  });

  it("prefers titleSort over title when present", () => {
    const books = [
      book(null, "The Second", "Second, The"),
      book(null, "An Opening", "Opening, An"),
    ];
    books.sort((a, b) => compareBooksByNumber(a, b));

    expect(books.map((b) => b.title)).toEqual(["An Opening", "The Second"]);
  });

  it("compares titles naturally so 9 precedes 10", () => {
    const books = [book(null, "Chapter 10"), book(null, "Chapter 9")];
    books.sort((a, b) => compareBooksByNumber(a, b));

    expect(books.map((b) => b.title)).toEqual(["Chapter 9", "Chapter 10"]);
  });

  // A comparator that mixes two orderings can report a < b < c < a, which
  // leaves the sorted result up to the engine's merge order.
  it("stays transitive when numbers and titles disagree", () => {
    const a = book(1, "Zeta");
    const b = book(2, "Alpha");
    const c = book(null, "Mid");

    expect(compareBooksByNumber(a, b)).toBeLessThan(0);
    expect(compareBooksByNumber(a, c)).toBeLessThan(0);
    expect(compareBooksByNumber(b, c)).toBeLessThan(0);
  });
});
