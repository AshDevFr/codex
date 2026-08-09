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

/** N mutually-unrelated candidates with descending scores. */
function unrelated(count: number): MbRecommendationEntry[] {
  return Array.from({ length: count }, (_, i) => ({
    score: 0.9 - i * 0.05,
    series: { id: 1000 + i, title: `Distinct Series ${i}` },
  }));
}

/**
 * A client stub whose `mix` returns the supplied entries. `readersAlsoLike`
 * defaults to empty so tests that only care about the content signal are not
 * silently relying on a missing method throwing.
 */
function clientReturning(
  entries: MbRecommendationEntry[],
  collaborative: Record<number, MbRecommendationEntry[]> = {},
) {
  const mix = vi.fn(async () => entries);
  const readersAlsoLike = vi.fn(async (seriesId: number) => collaborative[seriesId] ?? []);
  return {
    client: { mix, readersAlsoLike } as unknown as MangaBakaRecommendationClient,
    mix,
    readersAlsoLike,
  };
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
    const { client } = clientReturning(unrelated(5));

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
    const { client } = clientReturning(unrelated(10));

    const result = await generateRecommendations(
      client,
      request({ limit: 3 }),
      new DismissalStore(),
    );

    expect(result.recommendations).toHaveLength(3);
  });

  it("returns results in descending score order", async () => {
    const { client } = clientReturning(unrelated(5));

    const result = await generateRecommendations(client, request(), new DismissalStore());

    const scores = result.recommendations.map((r) => r.score);
    expect([...scores].sort((a, b) => b - a)).toEqual(scores);
  });

  it("excludes IDs the host says the user has already read", async () => {
    const entries = unrelated(5);
    const { client } = clientReturning(entries);
    const excluded = String(entries[0].series.id);

    const result = await generateRecommendations(
      client,
      request({ excludeIds: [excluded] }),
      new DismissalStore(),
    );

    expect(result.recommendations.map((r) => r.externalId)).not.toContain(excluded);
  });

  it("excludes previously dismissed recommendations", async () => {
    const entries = unrelated(5);
    const { client } = clientReturning(entries);
    const dismissed = new DismissalStore();
    const target = String(entries[1].series.id);
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
      { score: 0.6, series: { id: 112, title: "Other" } },
    ]);

    const result = await generateRecommendations(client, request(), new DismissalStore());

    expect(result.recommendations.map((r) => r.externalId)).toEqual(["111", "112"]);
    // The stronger of the two duplicate scores is the one that survived, so it
    // still outranks the unrelated entry.
    expect(result.recommendations[0].score).toBeGreaterThan(result.recommendations[1].score);
  });

  it("stamps a generation timestamp and reports results as fresh", async () => {
    const { client } = clientReturning(unrelated(3));

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

describe("generateRecommendations against the captured Re:Zero probe", () => {
  it("returns only genuine recommendations, not franchise members of the seeds", async () => {
    // The end-to-end regression for the problem this filtering exists to solve.
    // Untreated, this response yields five Re:Zero chapter volumes plus the
    // Solo Leveling novel, and nothing the user could actually act on.
    const { client } = clientReturning(fixtureEntries);

    const result = await generateRecommendations(client, request(), new DismissalStore());

    expect(result.recommendations.map((r) => r.externalId).sort()).toEqual(["7559", "808"]);
  });

  it("returns fewer than requested rather than padding with franchise entries", async () => {
    const { client } = clientReturning(fixtureEntries);

    const result = await generateRecommendations(
      client,
      request({ limit: 20 }),
      new DismissalStore(),
    );

    expect(result.recommendations).toHaveLength(2);
  });

  it("still yields results when the user opts out of same-author entries", async () => {
    const { client } = clientReturning(fixtureEntries);

    const result = await generateRecommendations(client, request(), new DismissalStore(), {
      excludeSameAuthor: true,
    });

    // Neither survivor shares an author with a seed, so the stricter setting
    // costs nothing here.
    expect(result.recommendations.map((r) => r.externalId).sort()).toEqual(["7559", "808"]);
  });
});

describe("generateRecommendations franchise handling", () => {
  it("keeps only the best entry when several volumes of one work are returned", async () => {
    const { client } = clientReturning([
      { score: 0.5, series: { id: 501, title: "Unknown Work, Vol. 1" } },
      { score: 0.8, series: { id: 502, title: "Unknown Work Vol. 2" } },
      { score: 0.6, series: { id: 503, title: "A Different Work" } },
    ]);

    const result = await generateRecommendations(client, request(), new DismissalStore());

    expect(result.recommendations.map((r) => r.externalId)).toEqual(["502", "503"]);
  });

  it("de-ranks a same-author result below a stronger unrelated one", async () => {
    const { client } = clientReturning([
      { score: 0.9, matched_author: true, series: { id: 601, title: "By The Same Author" } },
      { score: 0.7, series: { id: 602, title: "By Someone Else" } },
    ]);

    const result = await generateRecommendations(client, request(), new DismissalStore());

    expect(result.recommendations.map((r) => r.externalId)).toEqual(["602", "601"]);
  });

  it("flags a recommendation the user already holds under another provider's ID", async () => {
    const library = [
      entry(3397),
      {
        ...entry(999),
        externalIds: [{ source: "api:anilist", externalId: "144738" }],
      },
    ];
    const { client } = clientReturning([
      {
        score: 0.8,
        series: { id: 700, title: "Held Elsewhere", source: { anilist: { id: 144738 } } },
      },
    ]);

    const result = await generateRecommendations(
      client,
      request({ library }),
      new DismissalStore(),
    );

    expect(result.recommendations[0].inLibrary).toBe(true);
  });
});

describe("generateRecommendations collaborative blend", () => {
  it("surfaces a series only the collaborative signal found", async () => {
    // The point of the second signal: reachable by reader overlap, invisible
    // to tag similarity.
    const { client } = clientReturning([{ score: 0.5, series: { id: 900, title: "From Tags" } }], {
      3397: [{ score: 40, series: { id: 901, title: "From Readers" } }],
    });

    const result = await generateRecommendations(client, request(), new DismissalStore());

    expect(result.recommendations.map((r) => r.externalId)).toContain("901");
  });

  it("attributes a collaborative-only result to other readers", async () => {
    const library = [entry(3397, { title: "Solo Leveling" })];
    const { client } = clientReturning([], {
      3397: [{ score: 40, series: { id: 901, title: "From Readers" } }],
    });

    const result = await generateRecommendations(
      client,
      request({ library }),
      new DismissalStore(),
    );

    expect(result.recommendations[0].reason).toBe("Readers of Solo Leveling also read this");
  });

  it("ranks a series both signals agree on above an equally-similar content-only one", async () => {
    // Identical content scores, so the collaborative endorsement is the only
    // thing separating them.
    const { client } = clientReturning(
      [
        { score: 0.6, series: { id: 900, title: "Content Only" } },
        { score: 0.6, series: { id: 902, title: "Both Signals" } },
      ],
      { 3397: [{ score: 100, series: { id: 902, title: "Both Signals" } }] },
    );

    const result = await generateRecommendations(client, request(), new DismissalStore());

    expect(result.recommendations[0].externalId).toBe("902");
  });

  it("queries the collaborative endpoint once per chosen seed", async () => {
    const { client, readersAlsoLike } = clientReturning([]);

    await generateRecommendations(client, request(), new DismissalStore(), {
      collaborativeSeeds: 2,
    });

    expect(readersAlsoLike).toHaveBeenCalledTimes(2);
  });

  it("reproduces content-only behaviour when the collaborative signal is disabled", async () => {
    // The escape hatch has to actually work, or the blend cannot be backed out.
    const entries = [{ score: 0.5, series: { id: 900, title: "From Tags" } }];
    const collaborative = { 3397: [{ score: 40, series: { id: 901, title: "From Readers" } }] };

    const withBlend = clientReturning(entries, collaborative);
    const withoutBlend = clientReturning(entries, collaborative);

    const blended = await generateRecommendations(
      withBlend.client,
      request(),
      new DismissalStore(),
    );
    const contentOnly = await generateRecommendations(
      withoutBlend.client,
      request(),
      new DismissalStore(),
      { collaborativeSeeds: 0 },
    );

    expect(blended.recommendations.map((r) => r.externalId)).toContain("901");
    expect(contentOnly.recommendations.map((r) => r.externalId)).toEqual(["900"]);
  });

  it("drops a related work that arrives only through the collaborative path", async () => {
    // Collaborative results carry no matched_related flag, so the relationship
    // data is the only thing standing between the user and franchise spam here.
    const { client } = clientReturning([], {
      3397: [
        { score: 100, series: { id: 950, title: "Spin-off", relationships: { other: [3397] } } },
        { score: 90, series: { id: 951, title: "Genuinely Different" } },
      ],
    });

    const result = await generateRecommendations(client, request(), new DismissalStore());

    expect(result.recommendations.map((r) => r.externalId)).toEqual(["951"]);
  });

  it("still returns content results when every collaborative lookup fails", async () => {
    const client = {
      mix: vi.fn(async () => [{ score: 0.5, series: { id: 900, title: "From Tags" } }]),
      readersAlsoLike: vi.fn(async () => {
        throw new ApiError("API error: 503", 503);
      }),
    } as unknown as MangaBakaRecommendationClient;

    const result = await generateRecommendations(client, request(), new DismissalStore());

    expect(result.recommendations.map((r) => r.externalId)).toEqual(["900"]);
  });

  it("still returns collaborative results when the content probe fails", async () => {
    const client = {
      mix: vi.fn(async () => {
        throw new ApiError("API error: 503", 503);
      }),
      readersAlsoLike: vi.fn(async (seriesId: number) =>
        seriesId === 3397 ? [{ score: 40, series: { id: 901, title: "From Readers" } }] : [],
      ),
    } as unknown as MangaBakaRecommendationClient;

    const result = await generateRecommendations(client, request(), new DismissalStore());

    expect(result.recommendations.map((r) => r.externalId)).toEqual(["901"]);
  });

  it("returns nothing when both signals fail", async () => {
    const client = {
      mix: vi.fn(async () => {
        throw new ApiError("API error: 503", 503);
      }),
      readersAlsoLike: vi.fn(async () => {
        throw new ApiError("API error: 503", 503);
      }),
    } as unknown as MangaBakaRecommendationClient;

    const result = await generateRecommendations(client, request(), new DismissalStore());

    expect(result.recommendations).toEqual([]);
  });
});
