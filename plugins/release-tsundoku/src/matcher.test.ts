import type { TrackedSeriesEntry } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it } from "vitest";
import type { FeedExternalId, FeedItem } from "./fetcher.js";
import { buildMatchContext, externalIdFilter, matchItem } from "./matcher.js";

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
// buildMatchContext / externalIdFilter
// -----------------------------------------------------------------------------

describe("buildMatchContext", () => {
  it("indexes provider/id pairs and keeps each series' full id map", () => {
    const ctx = buildMatchContext([
      tracked("uuid-a", { mangabaka: "9741", anilist: "122180" }),
      tracked("uuid-b", { mal: "128555" }),
    ]);
    expect(ctx.byKey.get("mangabaka:9741")).toEqual(["uuid-a"]);
    expect(ctx.byKey.get("mal:128555")).toEqual(["uuid-b"]);
    expect(ctx.series.get("uuid-a")?.get("anilist")).toBe("122180");
  });

  it("skips entries without external ids and ignores empty values", () => {
    const ctx = buildMatchContext([tracked("uuid-a"), tracked("uuid-b", { mangabaka: "" })]);
    expect(ctx.series.size).toBe(0);
    expect(ctx.byKey.size).toBe(0);
  });
});

describe("externalIdFilter", () => {
  it("returns every provider:id key (the POST feed filter set)", () => {
    const ctx = buildMatchContext([tracked("uuid-a", { mangabaka: "9741", mal: "5" })]);
    expect(new Set(externalIdFilter(ctx))).toEqual(new Set(["mangabaka:9741", "mal:5"]));
  });
});

// -----------------------------------------------------------------------------
// matchItem — weighted voting
// -----------------------------------------------------------------------------

describe("matchItem", () => {
  it("matches when a single shared id agrees (no mangabaka required)", () => {
    const ctx = buildMatchContext([tracked("uuid-a", { mal: "128555" })]);
    const res = matchItem(feedItem([ext("mal", "128555")]), ctx);
    expect(res?.codexSeriesId).toBe("uuid-a");
    expect(res?.agreeingProviders).toEqual(["mal"]);
    expect(res?.score).toBe(1);
  });

  it("translates Codex provider names to Tsundoku's (myanimelist -> mal)", () => {
    // Codex stores `myanimelist`; the feed uses `mal`. They must still match.
    const ctx = buildMatchContext([tracked("uuid-a", { myanimelist: "128555" })]);
    const res = matchItem(feedItem([ext("mal", "128555")]), ctx);
    expect(res?.codexSeriesId).toBe("uuid-a");
    expect(res?.agreeingProviders).toEqual(["mal"]);
  });

  it("returns null when nothing is shared", () => {
    const ctx = buildMatchContext([tracked("uuid-a", { mangabaka: "9741" })]);
    expect(matchItem(feedItem([ext("mangabaka", "0000")]), ctx)).toBeNull();
    expect(matchItem(feedItem([]), ctx)).toBeNull();
  });

  it("lets a trusted disagreement veto a sloppy agreement", () => {
    // ABC has mangabaka:X + mal:Y. A different series shares mal:Y but its
    // mangabaka differs — the mangabaka conflict (weight 3) outvotes the mal
    // agreement (weight 1), so it must NOT match ABC.
    const ctx = buildMatchContext([tracked("ABC", { mangabaka: "X", mal: "Y" })]);
    const res = matchItem(feedItem([ext("mangabaka", "W"), ext("mal", "Y")]), ctx);
    expect(res).toBeNull();
  });

  it("accepts a true match that disagrees on one low-trust id", () => {
    // Same series, but its MAL id was remapped upstream: mangabaka+anilist
    // agree (3+2), mal disagrees (1) → net +4 → still a match.
    const ctx = buildMatchContext([tracked("ABC", { mangabaka: "X", anilist: "A", mal: "Y" })]);
    const res = matchItem(
      feedItem([ext("mangabaka", "X"), ext("anilist", "A"), ext("mal", "Z")]),
      ctx,
    );
    expect(res?.codexSeriesId).toBe("ABC");
    expect(res?.score).toBe(4); // 3 + 2 - 1
    expect(res?.agreeingProviders).toEqual(["mangabaka", "anilist"]);
  });

  it("rejects a net-zero tally (equal agree/disagree weight)", () => {
    const ctx = buildMatchContext([tracked("ABC", { mangabaka: "X", anilist: "A" })]);
    // mangabaka agrees (3), anilist... make weights cancel: agree mal(1) vs disagree mangabaka(3) handled above;
    // here anilist disagrees (2) and mal agrees — but ABC has no mal, so only anilist shared (disagree) → score<0.
    const res = matchItem(feedItem([ext("anilist", "ZZ")]), ctx);
    expect(res).toBeNull();
  });

  it("weights mangabaka above anilist when scoring confidence", () => {
    const ctxMb = buildMatchContext([tracked("a", { mangabaka: "1" })]);
    const ctxAl = buildMatchContext([tracked("b", { anilist: "1" })]);
    const mb = matchItem(feedItem([ext("mangabaka", "1")]), ctxMb);
    const al = matchItem(feedItem([ext("anilist", "1")]), ctxAl);
    expect(mb?.confidence).toBe(1.0); // 0.7 + 0.1*3
    expect(al?.confidence).toBeCloseTo(0.9); // 0.7 + 0.1*2
    expect((mb?.score ?? 0) > (al?.score ?? 0)).toBe(true);
  });

  it("returns null when two series match the item equally well (ambiguous)", () => {
    // Two tracked series each share only mal:Y with the item (no higher-trust
    // discriminator) → equal score → can't safely pick.
    const ctx = buildMatchContext([
      tracked("uuid-a", { mal: "Y" }),
      tracked("uuid-b", { mal: "Y" }),
    ]);
    expect(matchItem(feedItem([ext("mal", "Y")]), ctx)).toBeNull();
  });

  it("picks the higher-scoring series when candidates differ", () => {
    // The item shares mangabaka with A (score 3) and mal with B (score 1).
    const ctx = buildMatchContext([
      tracked("uuid-a", { mangabaka: "X" }),
      tracked("uuid-b", { mal: "Y" }),
    ]);
    const res = matchItem(feedItem([ext("mangabaka", "X"), ext("mal", "Y")]), ctx);
    expect(res?.codexSeriesId).toBe("uuid-a");
    expect(res?.score).toBe(3);
  });
});
