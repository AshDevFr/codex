import { describe, expect, it } from "vitest";
import { externalReleaseId, feedItemToCandidate, toSpans } from "./candidate.js";
import type { FeedItem } from "./fetcher.js";
import type { MatchResult } from "./matcher.js";

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

function feedItem(overrides: Partial<FeedItem> = {}): FeedItem {
  return {
    seriesId: 87,
    canonicalTitle: "Example Series",
    externalIds: [{ provider: "mangabaka", externalId: "9741", fetchedAt: 1_780_943_416 }],
    volumeCoverage: [{ start: 1, end: 16 }],
    chapterCoverage: [],
    highestVolume: 16,
    highestChapter: null,
    updatedAt: 1_780_943_416,
    ...overrides,
  };
}

const match: MatchResult = {
  codexSeriesId: "uuid-a",
  provider: "mangabaka",
  externalId: "9741",
};

const opts = {
  baseUrl: "https://t.example.com",
  language: "en",
  observedAt: "2026-06-08T00:00:00.000Z",
};

// -----------------------------------------------------------------------------
// toSpans
// -----------------------------------------------------------------------------

describe("toSpans", () => {
  it("maps coverage spans verbatim", () => {
    expect(
      toSpans([
        { start: 1, end: 16 },
        { start: 18, end: 20 },
      ]),
    ).toEqual([
      { start: 1, end: 16 },
      { start: 18, end: 20 },
    ]);
  });

  it("returns null for an empty coverage list", () => {
    expect(toSpans([])).toBeNull();
  });

  it("preserves decimal chapter spans", () => {
    expect(toSpans([{ start: 1, end: 45.5 }])).toEqual([{ start: 1, end: 45.5 }]);
  });
});

// -----------------------------------------------------------------------------
// externalReleaseId
// -----------------------------------------------------------------------------

describe("externalReleaseId", () => {
  it("keys on series id and both high-water marks", () => {
    expect(externalReleaseId(feedItem({ highestVolume: 16, highestChapter: 45 }))).toBe(
      "tsundoku:87:v16:c45",
    );
  });

  it("renders null high-water values as a dash", () => {
    expect(externalReleaseId(feedItem({ highestVolume: null, highestChapter: null }))).toBe(
      "tsundoku:87:v-:c-",
    );
    expect(externalReleaseId(feedItem({ highestVolume: 16, highestChapter: null }))).toBe(
      "tsundoku:87:v16:c-",
    );
  });

  it("is stable across re-delivery of the same coverage", () => {
    expect(externalReleaseId(feedItem())).toBe(externalReleaseId(feedItem()));
  });

  it("changes when the frontier advances", () => {
    expect(externalReleaseId(feedItem({ highestVolume: 16 }))).not.toBe(
      externalReleaseId(feedItem({ highestVolume: 17 })),
    );
  });
});

// -----------------------------------------------------------------------------
// feedItemToCandidate
// -----------------------------------------------------------------------------

describe("feedItemToCandidate", () => {
  it("builds an exact-match candidate (confidence 1.0)", () => {
    const c = feedItemToCandidate(feedItem(), match, opts);
    expect(c.seriesMatch).toEqual({
      codexSeriesId: "uuid-a",
      confidence: 1.0,
      reason: "tsundoku:mangabaka:9741",
    });
    expect(c.externalReleaseId).toBe("tsundoku:87:v16:c-");
    expect(c.language).toBe("en");
    expect(c.groupOrUploader).toBeNull();
  });

  it("maps coverage onto volume/chapter axes (empty -> null)", () => {
    const c = feedItemToCandidate(
      feedItem({
        volumeCoverage: [{ start: 1, end: 4 }],
        chapterCoverage: [{ start: 1, end: 21 }],
      }),
      match,
      opts,
    );
    expect(c.volumes).toEqual([{ start: 1, end: 4 }]);
    expect(c.chapters).toEqual([{ start: 1, end: 21 }]);

    const volumeOnly = feedItemToCandidate(feedItem({ chapterCoverage: [] }), match, opts);
    expect(volumeOnly.chapters).toBeNull();
    expect(volumeOnly.volumes).toEqual([{ start: 1, end: 16 }]);
  });

  it("builds the series landing URL and tolerates a trailing slash on baseUrl", () => {
    const c = feedItemToCandidate(feedItem(), match, {
      ...opts,
      baseUrl: "https://t.example.com/",
    });
    expect(c.payloadUrl).toBe("https://t.example.com/series/87");
  });

  it("derives releasedAt from updatedAt (epoch seconds) and uses observedAt", () => {
    const c = feedItemToCandidate(feedItem({ updatedAt: 1_780_943_416 }), match, opts);
    expect(c.releasedAt).toBe(new Date(1_780_943_416 * 1000).toISOString());
    expect(c.observedAt).toBe("2026-06-08T00:00:00.000Z");
  });

  it("carries Tsundoku context in metadata", () => {
    const c = feedItemToCandidate(feedItem(), match, opts);
    expect(c.metadata).toEqual({
      tsundokuSeriesId: 87,
      canonicalTitle: "Example Series",
      highestVolume: 16,
      highestChapter: null,
    });
  });
});
