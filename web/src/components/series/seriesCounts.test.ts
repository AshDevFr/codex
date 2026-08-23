import { describe, expect, it } from "vitest";
import { formatChapterCount, formatSeriesCounts } from "./seriesCounts";

describe("formatChapterCount", () => {
  it("renders integers without a decimal", () => {
    expect(formatChapterCount(109)).toBe("109");
  });

  it("preserves fractional chapter counts", () => {
    expect(formatChapterCount(109.5)).toBe("109.5");
  });
});

describe("formatSeriesCounts", () => {
  it("returns null when there is nothing to show", () => {
    expect(
      formatSeriesCounts({
        bookCount: null,
        totalVolumeCount: null,
        totalChapterCount: null,
      }),
    ).toBeNull();
  });

  it("renders the book count alone when nothing else is known", () => {
    expect(
      formatSeriesCounts({
        bookCount: 12,
        totalVolumeCount: null,
        totalChapterCount: null,
      }),
    ).toBe("12 books");
  });

  it("uses the singular 'book' for a single-book series", () => {
    expect(formatSeriesCounts({ bookCount: 1 })).toBe("1 book");
  });

  it("treats a zero book count as a real count, not as missing", () => {
    expect(formatSeriesCounts({ bookCount: 0 })).toBe("0 books");
  });

  it("renders every axis for a mixed series", () => {
    expect(
      formatSeriesCounts({
        bookCount: 39,
        totalVolumeCount: 17,
        totalChapterCount: 223,
        localMaxVolume: 14,
        localMaxChapter: 150,
      }),
    ).toBe("39 books · 14/17 vol · 150/223 ch");
  });

  it("renders local maxima without a denominator when totals are unknown", () => {
    expect(
      formatSeriesCounts({
        bookCount: 20,
        localMaxVolume: 14,
        localMaxChapter: 109.5,
      }),
    ).toBe("20 books · 14 vol · 109.5 ch");
  });

  // The library-shape bugs this formatter exists to avoid: `bookCount` is a
  // file count and must never be borrowed as a numerator on an axis the books
  // carry no metadata for, and a total on such an axis is never converted into
  // a position on the other one.
  it("never uses the book count as a volume numerator", () => {
    expect(
      formatSeriesCounts({
        bookCount: 225,
        totalVolumeCount: 8,
        totalChapterCount: 223,
        localMaxVolume: null,
        localMaxChapter: 223,
      }),
    ).toBe("225 books · 8 vol total · 223/223 ch");
  });

  it("never uses the book count as a chapter numerator", () => {
    expect(
      formatSeriesCounts({
        bookCount: 2,
        totalVolumeCount: null,
        totalChapterCount: 169,
        localMaxVolume: 2,
        localMaxChapter: null,
      }),
    ).toBe("2 books · 2 vol · 169 ch total");
  });

  it("keeps a known total on an axis with no local signal at all", () => {
    expect(
      formatSeriesCounts({
        bookCount: 2,
        totalVolumeCount: 8,
        totalChapterCount: 169,
      }),
    ).toBe("2 books · 8 vol total · 169 ch total");
  });

  it("omits an axis that has neither a local maximum nor a total", () => {
    expect(
      formatSeriesCounts({
        bookCount: 60,
        totalVolumeCount: null,
        totalChapterCount: 158,
        localMaxChapter: 137,
      }),
    ).toBe("60 books · 137/158 ch");
  });

  it("prefers the highest volume owned over the file count", () => {
    expect(
      formatSeriesCounts({
        // 17 files on disk: v01..v15 plus loose chapter files.
        bookCount: 17,
        totalVolumeCount: 17,
        localMaxVolume: 14,
      }),
    ).toBe("17 books · 14/17 vol");
  });

  it("preserves fractional local chapter maxima", () => {
    expect(
      formatSeriesCounts({
        bookCount: 42,
        totalChapterCount: 30,
        localMaxChapter: 30.1,
      }),
    ).toBe("42 books · 30.1/30 ch");
  });

  it("renders the axes without a book count when the count is missing", () => {
    expect(
      formatSeriesCounts({
        bookCount: undefined,
        totalVolumeCount: 17,
        totalChapterCount: 158,
        localMaxVolume: 14,
      }),
    ).toBe("14/17 vol · 158 ch total");
  });

  it("treats a zero volume total as a real total, not as missing", () => {
    expect(
      formatSeriesCounts({
        bookCount: 3,
        totalVolumeCount: 0,
        localMaxVolume: 0,
      }),
    ).toBe("3 books · 0/0 vol");
  });
});
