import { describe, expect, it } from "vitest";
import type { UserLibraryEntry } from "@ashdev/codex-plugin-sdk";
import { pickSeedEntries } from "./index.js";

// =============================================================================
// Helpers
// =============================================================================

function makeEntry(overrides: Partial<UserLibraryEntry> & { seriesId: string }): UserLibraryEntry {
  return {
    title: `Series ${overrides.seriesId}`,
    alternateTitles: [],
    genres: [],
    tags: [],
    externalIds: [],
    booksRead: 0,
    booksOwned: 0,
    ...overrides,
  };
}

// =============================================================================
// pickSeedEntries Tests
// =============================================================================

describe("pickSeedEntries", () => {
  it("returns empty array for empty library", () => {
    expect(pickSeedEntries([], 10)).toEqual([]);
  });

  it("returns all entries when fewer than maxSeeds", () => {
    const entries = [makeEntry({ seriesId: "a", userRating: 80, booksRead: 5 })];
    const result = pickSeedEntries(entries, 10);
    expect(result).toHaveLength(1);
    expect(result[0].seriesId).toBe("a");
  });

  it("limits to maxSeeds", () => {
    const entries = Array.from({ length: 20 }, (_, i) =>
      makeEntry({ seriesId: `s${i}`, userRating: 50, booksRead: 1 }),
    );
    const result = pickSeedEntries(entries, 5);
    expect(result).toHaveLength(5);
  });

  it("prioritizes by rating descending", () => {
    const entries = [
      makeEntry({ seriesId: "low", userRating: 30, booksRead: 10 }),
      makeEntry({ seriesId: "high", userRating: 90, booksRead: 1 }),
      makeEntry({ seriesId: "mid", userRating: 60, booksRead: 5 }),
    ];
    const result = pickSeedEntries(entries, 3);
    expect(result.map((e) => e.seriesId)).toEqual(["high", "mid", "low"]);
  });

  it("breaks ties by booksRead descending", () => {
    const entries = [
      makeEntry({ seriesId: "fewer", userRating: 80, booksRead: 2 }),
      makeEntry({ seriesId: "more", userRating: 80, booksRead: 10 }),
    ];
    const result = pickSeedEntries(entries, 2);
    expect(result[0].seriesId).toBe("more");
    expect(result[1].seriesId).toBe("fewer");
  });

  it("treats undefined rating as 0", () => {
    const entries = [
      makeEntry({ seriesId: "rated", userRating: 50, booksRead: 1 }),
      makeEntry({ seriesId: "unrated", booksRead: 1 }),
    ];
    const result = pickSeedEntries(entries, 2);
    expect(result[0].seriesId).toBe("rated");
    expect(result[1].seriesId).toBe("unrated");
  });

  it("does not mutate the original array", () => {
    const entries = [
      makeEntry({ seriesId: "b", userRating: 90, booksRead: 1 }),
      makeEntry({ seriesId: "a", userRating: 50, booksRead: 1 }),
    ];
    const originalOrder = entries.map((e) => e.seriesId);
    pickSeedEntries(entries, 2);
    expect(entries.map((e) => e.seriesId)).toEqual(originalOrder);
  });
});
