import type { RecommendationRequest, UserLibraryEntry } from "@ashdev/codex-plugin-sdk";
import { ApiError } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it, vi } from "vitest";
import fixture from "./__fixtures__/mix-response.json" with { type: "json" };
import type { MangaBakaRecommendationClient } from "./api.js";
import { DismissalStore } from "./dismissals.js";
import { generateRecommendations, MIX_OVERFETCH_FACTOR } from "./recommend.js";
import type { MbRecommendationEntry } from "./types.js";

const fixtureEntries = fixture.data as unknown as MbRecommendationEntry[];

/** A library entry carrying a MangaBaka ID. */
function entry(id: number, overrides: Partial<UserLibraryEntry> = {}): UserLibraryEntry {
  return {
    seriesId: `series-${id}`,
    title: `Title ${id}`,
    alternateTitles: [],
    genres: [],
    tags: [],
    externalIds: [{ source: "api:mangabaka", externalId: String(id) }],
    booksRead: 0,
    booksOwned: 1,
    ...overrides,
  };
}

/** A client stub whose `mix` returns the supplied entries. */
function clientReturning(entries: MbRecommendationEntry[]) {
  const mix = vi.fn(async () => entries);
  return { client: { mix } as unknown as MangaBakaRecommendationClient, mix };
}

function request(overrides: Partial<RecommendationRequest> = {}): RecommendationRequest {
  return { library: [entry(3397), entry(84926)], excludeIds: [], ...overrides };
}

describe("generateRecommendations", () => {
  it("returns an empty set for an empty library without calling upstream", async () => {
    const { client, mix } = clientReturning([]);

    const result = await generateRecommendations(
      client,
      request({ library: [] }),
      new DismissalStore(),
    );

    expect(result.recommendations).toEqual([]);
    expect(mix).not.toHaveBeenCalled();
  });

  it("returns an empty set when no library entry has a MangaBaka ID", async () => {
    const { client, mix } = clientReturning([]);
    const unmatched = entry(1, { externalIds: [{ source: "api:anilist", externalId: "5" }] });

    const result = await generateRecommendations(
      client,
      request({ library: [unmatched] }),
      new DismissalStore(),
    );

    expect(result.recommendations).toEqual([]);
    // Calling mix with no seeds would be rejected upstream anyway.
    expect(mix).not.toHaveBeenCalled();
  });

  it("sends every resolved seed in a single request", async () => {
    const { client, mix } = clientReturning(fixtureEntries);

    await generateRecommendations(client, request(), new DismissalStore());

    expect(mix).toHaveBeenCalledTimes(1);
    expect(mix.mock.calls[0][0]).toMatchObject({ series: [3397, 84926] });
  });

  it("disables strict mode so tag rules boost rather than hard-filter", async () => {
    const { client, mix } = clientReturning(fixtureEntries);

    await generateRecommendations(client, request(), new DismissalStore());

    expect(mix.mock.calls[0][0].strict).toBe(false);
  });

  it("over-fetches so later filtering does not starve the result set", async () => {
    const { client, mix } = clientReturning(fixtureEntries);

    await generateRecommendations(client, request({ limit: 10 }), new DismissalStore());

    expect(mix.mock.calls[0][0].limit).toBe(10 * MIX_OVERFETCH_FACTOR);
  });

  it("caps the over-fetch at the endpoint maximum", async () => {
    const { client, mix } = clientReturning(fixtureEntries);

    await generateRecommendations(client, request({ limit: 40 }), new DismissalStore());

    expect(mix.mock.calls[0][0].limit).toBe(50);
  });

  it("maps upstream entries into recommendations", async () => {
    const { client } = clientReturning(fixtureEntries);

    const result = await generateRecommendations(client, request(), new DismissalStore());

    expect(result.recommendations.length).toBeGreaterThan(0);
    for (const rec of result.recommendations) {
      expect(rec.externalId).toMatch(/^\d+$/);
      expect(rec.title).toBeTruthy();
      expect(rec.score).toBeGreaterThanOrEqual(0);
      expect(rec.score).toBeLessThanOrEqual(1);
    }
  });

  it("honours the requested limit", async () => {
    const { client } = clientReturning(fixtureEntries);

    const result = await generateRecommendations(
      client,
      request({ limit: 3 }),
      new DismissalStore(),
    );

    expect(result.recommendations).toHaveLength(3);
  });

  it("returns results in descending score order", async () => {
    const { client } = clientReturning(fixtureEntries);

    const result = await generateRecommendations(client, request(), new DismissalStore());

    const scores = result.recommendations.map((r) => r.score);
    expect([...scores].sort((a, b) => b - a)).toEqual(scores);
  });

  it("excludes IDs the host says the user has already read", async () => {
    const { client } = clientReturning(fixtureEntries);
    const excluded = String(fixtureEntries[0].series.id);

    const result = await generateRecommendations(
      client,
      request({ excludeIds: [excluded] }),
      new DismissalStore(),
    );

    expect(result.recommendations.map((r) => r.externalId)).not.toContain(excluded);
  });

  it("excludes previously dismissed recommendations", async () => {
    const { client } = clientReturning(fixtureEntries);
    const dismissed = new DismissalStore();
    const target = String(fixtureEntries[1].series.id);
    await dismissed.add(target);

    const result = await generateRecommendations(client, request(), dismissed);

    expect(result.recommendations.map((r) => r.externalId)).not.toContain(target);
  });

  it("never recommends a seed back to the user", async () => {
    // The probe is built from these; returning one is pure noise.
    const seedId = 3397;
    const { client } = clientReturning([
      { score: 0.9, series: { id: seedId, title: "Solo Leveling" } },
      { score: 0.5, series: { id: 111, title: "Something Else" } },
    ]);

    const result = await generateRecommendations(client, request(), new DismissalStore());

    expect(result.recommendations.map((r) => r.externalId)).toEqual(["111"]);
  });

  it("de-duplicates entries repeated within one upstream response", async () => {
    const { client } = clientReturning([
      { score: 0.9, series: { id: 111, title: "Dup" } },
      { score: 0.4, series: { id: 111, title: "Dup" } },
    ]);

    const result = await generateRecommendations(client, request(), new DismissalStore());

    expect(result.recommendations).toHaveLength(1);
    // The stronger score wins.
    expect(result.recommendations[0].score).toBe(0.9);
  });

  it("stamps a generation timestamp and reports results as fresh", async () => {
    const { client } = clientReturning(fixtureEntries);

    const result = await generateRecommendations(client, request(), new DismissalStore());

    expect(result.cached).toBe(false);
    expect(Number.isNaN(Date.parse(result.generatedAt ?? ""))).toBe(false);
  });

  it("degrades to an empty set when upstream fails", async () => {
    // A failed recommendation refresh should not fail the host's task.
    const client = {
      mix: vi.fn(async () => {
        throw new ApiError("API error: 503 Service Unavailable", 503);
      }),
    } as unknown as MangaBakaRecommendationClient;

    const result = await generateRecommendations(client, request(), new DismissalStore());

    expect(result.recommendations).toEqual([]);
    expect(result.cached).toBe(false);
  });

  it("drops entries the mapper rejects rather than emitting nulls", async () => {
    const { client } = clientReturning([
      { score: 0.9, series: { id: 111, title: "Fine" } },
      { score: 0.8, series: { id: 222, title: "Tombstone", state: "deleted" } },
    ]);

    const result = await generateRecommendations(client, request(), new DismissalStore());

    expect(result.recommendations.map((r) => r.externalId)).toEqual(["111"]);
  });

  it("names seeds in basedOn using their Codex titles", async () => {
    const { client } = clientReturning([
      { score: 0.5, matched_seed_ids: [3397], series: { id: 999, title: "Rec" } },
    ]);
    const library = [entry(3397, { title: "My Local Title" })];

    const result = await generateRecommendations(
      client,
      request({ library }),
      new DismissalStore(),
    );

    expect(result.recommendations[0].basedOn).toEqual(["My Local Title"]);
  });
});
