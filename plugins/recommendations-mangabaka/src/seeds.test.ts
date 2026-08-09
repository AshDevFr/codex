import type { UserLibraryEntry } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it } from "vitest";
import { resolveSeeds } from "./seeds.js";

/** Build a library entry with only the fields seed resolution reads. */
function entry(overrides: Partial<UserLibraryEntry> = {}): UserLibraryEntry {
  return {
    seriesId: "11111111-1111-1111-1111-111111111111",
    title: "Some Series",
    alternateTitles: [],
    genres: [],
    tags: [],
    externalIds: [],
    booksRead: 0,
    booksOwned: 1,
    ...overrides,
  };
}

describe("resolveSeeds", () => {
  it("reads the api:mangabaka external ID", () => {
    const result = resolveSeeds([
      entry({ title: "Berserk", externalIds: [{ source: "api:mangabaka", externalId: "1234" }] }),
    ]);

    expect(result.seeds).toEqual([{ id: 1234, title: "Berserk", rating: 0 }]);
    expect(result.unresolved).toBe(0);
  });

  it("carries the user rating through for downstream seed ranking", () => {
    const result = resolveSeeds([
      entry({
        title: "Vinland Saga",
        userRating: 92,
        externalIds: [{ source: "api:mangabaka", externalId: "7" }],
      }),
    ]);

    expect(result.seeds[0].rating).toBe(92);
  });

  it("accepts the legacy bare mangabaka source name", () => {
    const result = resolveSeeds([
      entry({ externalIds: [{ source: "mangabaka", externalId: "42" }] }),
    ]);

    expect(result.seeds.map((s) => s.id)).toEqual([42]);
  });

  it("prefers the canonical source when both are present", () => {
    const result = resolveSeeds([
      entry({
        externalIds: [
          { source: "mangabaka", externalId: "1" },
          { source: "api:mangabaka", externalId: "2" },
        ],
      }),
    ]);

    expect(result.seeds.map((s) => s.id)).toEqual([2]);
  });

  it("counts entries with no MangaBaka ID as unresolved", () => {
    const result = resolveSeeds([
      entry({ externalIds: [{ source: "api:anilist", externalId: "999" }] }),
      entry({ externalIds: [] }),
    ]);

    expect(result.seeds).toEqual([]);
    expect(result.unresolved).toBe(2);
  });

  it("does not fall back to title search", () => {
    // A bad title match would poison the shared probe vector for every result,
    // not just one, so unmatched entries are skipped rather than guessed at.
    const result = resolveSeeds([entry({ title: "Berserk", externalIds: [] })]);

    expect(result.seeds).toEqual([]);
  });

  it("drops malformed IDs rather than emitting NaN", () => {
    const result = resolveSeeds([
      entry({ externalIds: [{ source: "api:mangabaka", externalId: "not-a-number" }] }),
      entry({ externalIds: [{ source: "api:mangabaka", externalId: "" }] }),
      entry({ externalIds: [{ source: "api:mangabaka", externalId: "12abc" }] }),
    ]);

    expect(result.seeds).toEqual([]);
    expect(result.unresolved).toBe(3);
  });

  it("rejects zero and negative IDs", () => {
    const result = resolveSeeds([
      entry({ externalIds: [{ source: "api:mangabaka", externalId: "0" }] }),
      entry({ externalIds: [{ source: "api:mangabaka", externalId: "-5" }] }),
    ]);

    expect(result.seeds).toEqual([]);
  });

  it("de-duplicates seeds that resolve to the same series", () => {
    // Two Codex series can point at one MangaBaka entry (e.g. a split library).
    // Sending the ID twice would double its weight in the probe vector.
    const result = resolveSeeds([
      entry({ title: "First", externalIds: [{ source: "api:mangabaka", externalId: "5" }] }),
      entry({ title: "Second", externalIds: [{ source: "api:mangabaka", externalId: "5" }] }),
    ]);

    expect(result.seeds).toHaveLength(1);
    expect(result.seeds[0].title).toBe("First");
  });

  it("keeps the higher rating when de-duplicating", () => {
    const result = resolveSeeds([
      entry({ userRating: 40, externalIds: [{ source: "api:mangabaka", externalId: "5" }] }),
      entry({ userRating: 90, externalIds: [{ source: "api:mangabaka", externalId: "5" }] }),
    ]);

    expect(result.seeds[0].rating).toBe(90);
  });

  it("preserves host ordering, which is already curated by rating", () => {
    // The host sends rated entries first, highest first. Downstream code takes
    // the top-K for collaborative lookups, so that order must survive.
    const result = resolveSeeds([
      entry({ title: "A", externalIds: [{ source: "api:mangabaka", externalId: "3" }] }),
      entry({ title: "B", externalIds: [{ source: "api:mangabaka", externalId: "1" }] }),
      entry({ title: "C", externalIds: [{ source: "api:mangabaka", externalId: "2" }] }),
    ]);

    expect(result.seeds.map((s) => s.title)).toEqual(["A", "B", "C"]);
  });

  it("handles an empty library", () => {
    const result = resolveSeeds([]);

    expect(result.seeds).toEqual([]);
    expect(result.unresolved).toBe(0);
  });

  it("tolerates a missing externalIds array", () => {
    // Defensive: the host always sends it, but a protocol change should not
    // crash seed resolution.
    const broken = { title: "X" } as unknown as UserLibraryEntry;

    expect(() => resolveSeeds([broken])).not.toThrow();
  });
});

describe("resolveSeeds in-library ID collection", () => {
  it("collects MangaBaka IDs from every entry, not just resolvable seeds", () => {
    const result = resolveSeeds([
      entry({ externalIds: [{ source: "api:mangabaka", externalId: "10" }] }),
      entry({ externalIds: [{ source: "api:mangabaka", externalId: "20" }] }),
    ]);

    expect([...result.libraryIds].sort((a, b) => a - b)).toEqual([10, 20]);
  });
});
