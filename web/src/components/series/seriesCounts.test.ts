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
        localCount: null,
        totalVolumeCount: null,
        totalChapterCount: null,
      }),
    ).toBeNull();
  });

  it("falls back to legacy 'N books' when only the local count is known", () => {
    expect(
      formatSeriesCounts({
        localCount: 12,
        totalVolumeCount: null,
        totalChapterCount: null,
      }),
    ).toBe("12 books");
  });

  it("renders volumes only when chapter total is missing", () => {
    expect(
      formatSeriesCounts({
        localCount: 3,
        totalVolumeCount: 14,
        totalChapterCount: null,
      }),
    ).toBe("3/14 vol");
  });

  it("renders volume total only when local count is missing", () => {
    expect(
      formatSeriesCounts({
        localCount: null,
        totalVolumeCount: 14,
        totalChapterCount: null,
      }),
    ).toBe("14 vol");
  });

  it("renders chapter-only counts (the chapter-organized fix case)", () => {
    expect(
      formatSeriesCounts({
        localCount: 109,
        totalVolumeCount: null,
        totalChapterCount: 109,
      }),
    ).toBe("109/109 ch");
  });

  it("renders chapter total only when local count is missing", () => {
    expect(
      formatSeriesCounts({
        localCount: null,
        totalVolumeCount: null,
        totalChapterCount: 109.5,
      }),
    ).toBe("109.5 ch");
  });

  it("renders both axes when both totals are known", () => {
    expect(
      formatSeriesCounts({
        localCount: 109,
        totalVolumeCount: 14,
        totalChapterCount: 109,
      }),
    ).toBe("109/14 vol · 109 ch");
  });

  it("renders both axes without local count when local is missing", () => {
    expect(
      formatSeriesCounts({
        localCount: undefined,
        totalVolumeCount: 14,
        totalChapterCount: 109,
      }),
    ).toBe("14 vol · 109 ch");
  });

  it("treats zero as a real count, not as missing", () => {
    expect(
      formatSeriesCounts({
        localCount: 0,
        totalVolumeCount: 0,
        totalChapterCount: null,
      }),
    ).toBe("0/0 vol");
  });
});
