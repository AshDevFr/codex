import type { TrackedSeriesEntry } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it } from "vitest";
import type { FeedExternalId, FeedItem } from "./fetcher.js";
import { buildIndex, matchItem } from "./matcher.js";

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

function tracked(seriesId: string, externalIds?: Record<string, string>): TrackedSeriesEntry {
  return externalIds ? { seriesId, externalIds } : { seriesId };
}

function feedItem(externalIds: FeedExternalId[], seriesId = 1): FeedItem {
  return {
    seriesId,
    canonicalTitle: "T",
    externalIds,
    volumeCoverage: [],
    chapterCoverage: [],
    highestVolume: null,
    highestChapter: null,
    updatedAt: 1_700_000_000,
  };
}

function ext(provider: string, externalId: string): FeedExternalId {
  return { provider, externalId, fetchedAt: 1_700_000_000 };
}

// -----------------------------------------------------------------------------
// buildIndex
// -----------------------------------------------------------------------------

describe("buildIndex", () => {
  it("indexes each provider/id pair to its series", () => {
    const index = buildIndex([
      tracked("uuid-a", { mangabaka: "9741", anilist: "122180" }),
      tracked("uuid-b", { mal: "128555" }),
    ]);
    expect(index.get("mangabaka:9741")).toBe("uuid-a");
    expect(index.get("anilist:122180")).toBe("uuid-a");
    expect(index.get("mal:128555")).toBe("uuid-b");
    expect(index.size).toBe(3);
  });

  it("skips entries without external IDs", () => {
    const index = buildIndex([tracked("uuid-a"), tracked("uuid-b", {})]);
    expect(index.size).toBe(0);
  });

  it("ignores empty external-id values", () => {
    const index = buildIndex([tracked("uuid-a", { mangabaka: "" })]);
    expect(index.size).toBe(0);
  });
});

// -----------------------------------------------------------------------------
// matchItem
// -----------------------------------------------------------------------------

describe("matchItem", () => {
  it("matches on a single provider id", () => {
    const index = buildIndex([tracked("uuid-a", { mangabaka: "9741" })]);
    const result = matchItem(feedItem([ext("mangabaka", "9741")]), index);
    expect(result).toEqual({ codexSeriesId: "uuid-a", provider: "mangabaka", externalId: "9741" });
  });

  it("returns null when no provider id is tracked", () => {
    const index = buildIndex([tracked("uuid-a", { mangabaka: "9741" })]);
    expect(matchItem(feedItem([ext("mangabaka", "0000")]), index)).toBeNull();
    expect(matchItem(feedItem([ext("anilist", "9741")]), index)).toBeNull();
  });

  it("prefers the highest-priority provider when several would match", () => {
    // The same series is tracked under both mangabaka and mal; mangabaka leads
    // the priority order, so it should win regardless of array order on the item.
    const index = buildIndex([tracked("uuid-a", { mangabaka: "9741", mal: "128555" })]);
    const result = matchItem(feedItem([ext("mal", "128555"), ext("mangabaka", "9741")]), index);
    expect(result?.provider).toBe("mangabaka");
  });

  it("falls through to a lower-priority provider when the leader misses", () => {
    const index = buildIndex([tracked("uuid-a", { mal: "128555" })]);
    const result = matchItem(
      feedItem([ext("mangabaka", "not-tracked"), ext("mal", "128555")]),
      index,
    );
    expect(result).toEqual({ codexSeriesId: "uuid-a", provider: "mal", externalId: "128555" });
  });

  it("ignores providers outside the supported set", () => {
    const index = new Map<string, string>([["someunknownprovider:1", "uuid-a"]]);
    expect(matchItem(feedItem([ext("someunknownprovider", "1")]), index)).toBeNull();
  });

  it("returns null for an item with no external ids", () => {
    const index = buildIndex([tracked("uuid-a", { mangabaka: "9741" })]);
    expect(matchItem(feedItem([]), index)).toBeNull();
  });
});
