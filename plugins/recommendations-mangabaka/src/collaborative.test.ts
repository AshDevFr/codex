import { ApiError } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it, vi } from "vitest";
import type { MangaBakaRecommendationClient } from "./api.js";
import {
  COLLABORATIVE_CONCURRENCY,
  DEFAULT_COLLABORATIVE_SEEDS,
  fetchCollaborative,
} from "./collaborative.js";
import type { ResolvedSeed } from "./seeds.js";
import type { MbRecommendationEntry } from "./types.js";

function seed(id: number, title: string, rating = 0): ResolvedSeed {
  return { id, title, rating };
}

/** An upstream collaborative entry. */
function row(id: number, score: number): MbRecommendationEntry {
  return { score, series: { id, title: `Series ${id}` } };
}

/** A client stub returning per-seed responses. */
function clientReturning(bySeed: Record<number, MbRecommendationEntry[]>) {
  const readersAlsoLike = vi.fn(async (seriesId: number) => bySeed[seriesId] ?? []);
  return {
    client: { readersAlsoLike } as unknown as MangaBakaRecommendationClient,
    readersAlsoLike,
  };
}

describe("fetchCollaborative", () => {
  it("returns nothing when there are no seeds", async () => {
    const { client, readersAlsoLike } = clientReturning({});

    const result = await fetchCollaborative(client, []);

    expect(result.size).toBe(0);
    expect(readersAlsoLike).not.toHaveBeenCalled();
  });

  it("returns nothing when the signal is disabled", async () => {
    const { client, readersAlsoLike } = clientReturning({ 1: [row(10, 5)] });

    const result = await fetchCollaborative(client, [seed(1, "A")], { collaborativeSeeds: 0 });

    expect(result.size).toBe(0);
    expect(readersAlsoLike).not.toHaveBeenCalled();
  });

  it("queries only the top-K seeds, which arrive rating-ordered", async () => {
    const { client, readersAlsoLike } = clientReturning({});

    await fetchCollaborative(
      client,
      [seed(1, "A", 90), seed(2, "B", 80), seed(3, "C", 70), seed(4, "D", 60)],
      { collaborativeSeeds: 2 },
    );

    expect(readersAlsoLike).toHaveBeenCalledTimes(2);
    expect(readersAlsoLike.mock.calls.map((c) => c[0])).toEqual([1, 2]);
  });

  it("defaults to a small number of seeds", async () => {
    const { client, readersAlsoLike } = clientReturning({});
    const seeds = Array.from({ length: 20 }, (_, i) => seed(i + 1, `S${i}`));

    await fetchCollaborative(client, seeds);

    expect(readersAlsoLike).toHaveBeenCalledTimes(DEFAULT_COLLABORATIVE_SEEDS);
  });

  it("normalises each response against its own maximum", async () => {
    // Raw scores are unbounded co-occurrence weights whose scale differs by
    // more than an order of magnitude between seeds (10 vs 306 observed live),
    // so they are only meaningful relative to their own response.
    const { client } = clientReturning({
      1: [row(100, 10), row(101, 5)],
      2: [row(200, 300), row(201, 150)],
    });

    const result = await fetchCollaborative(client, [seed(1, "A"), seed(2, "B")], {
      collaborativeSeeds: 2,
    });

    expect(result.get(100)?.score).toBe(1);
    expect(result.get(101)?.score).toBe(0.5);
    expect(result.get(200)?.score).toBe(1);
    expect(result.get(201)?.score).toBe(0.5);
  });

  it("keeps the strongest endorsement when several seeds return the same series", async () => {
    const { client } = clientReturning({
      1: [row(100, 10), row(999, 2)],
      2: [row(100, 100)],
    });

    const result = await fetchCollaborative(client, [seed(1, "A"), seed(2, "B")], {
      collaborativeSeeds: 2,
    });

    // 1.0 from seed 2's list beats 0.2 from seed 1's.
    expect(result.get(100)?.score).toBe(1);
  });

  it("records every seed that endorsed a series", async () => {
    const { client } = clientReturning({
      1: [row(100, 10)],
      2: [row(100, 5)],
    });

    const result = await fetchCollaborative(client, [seed(1, "Alpha"), seed(2, "Beta")], {
      collaborativeSeeds: 2,
    });

    expect(result.get(100)?.seedTitles.sort()).toEqual(["Alpha", "Beta"]);
  });

  it("tolerates a failure on one seed and keeps the rest", async () => {
    // A single bad seed must not cost the user their whole recommendation run.
    const readersAlsoLike = vi.fn(async (seriesId: number) => {
      if (seriesId === 1) throw new ApiError("API error: 503", 503);
      return [row(200, 10)];
    });
    const client = { readersAlsoLike } as unknown as MangaBakaRecommendationClient;

    const result = await fetchCollaborative(client, [seed(1, "A"), seed(2, "B")], {
      collaborativeSeeds: 2,
    });

    expect(result.size).toBe(1);
    expect(result.get(200)?.score).toBe(1);
  });

  it("returns nothing when every seed fails", async () => {
    const readersAlsoLike = vi.fn(async () => {
      throw new ApiError("API error: 503", 503);
    });
    const client = { readersAlsoLike } as unknown as MangaBakaRecommendationClient;

    const result = await fetchCollaborative(client, [seed(1, "A")], { collaborativeSeeds: 1 });

    expect(result.size).toBe(0);
  });

  it("never runs more requests concurrently than the bound allows", async () => {
    let inFlight = 0;
    let peak = 0;
    const readersAlsoLike = vi.fn(async () => {
      inFlight++;
      peak = Math.max(peak, inFlight);
      await new Promise((resolve) => setTimeout(resolve, 5));
      inFlight--;
      return [];
    });
    const client = { readersAlsoLike } as unknown as MangaBakaRecommendationClient;
    const seeds = Array.from({ length: 10 }, (_, i) => seed(i + 1, `S${i}`));

    await fetchCollaborative(client, seeds, { collaborativeSeeds: 10 });

    expect(peak).toBeLessThanOrEqual(COLLABORATIVE_CONCURRENCY);
    expect(readersAlsoLike).toHaveBeenCalledTimes(10);
  });

  it("still issues every request despite the concurrency bound", async () => {
    const { client, readersAlsoLike } = clientReturning({});
    const seeds = Array.from({ length: 7 }, (_, i) => seed(i + 1, `S${i}`));

    await fetchCollaborative(client, seeds, { collaborativeSeeds: 7 });

    expect(readersAlsoLike).toHaveBeenCalledTimes(7);
  });

  it("ignores entries with a non-positive score", async () => {
    // Guards against a divide-by-zero normalisation on an all-zero response.
    const { client } = clientReturning({ 1: [row(100, 0), row(101, 0)] });

    const result = await fetchCollaborative(client, [seed(1, "A")], { collaborativeSeeds: 1 });

    expect(result.size).toBe(0);
  });

  it("does not return the seed itself", async () => {
    // Seeds routinely appear in each other's collaborative lists.
    const { client } = clientReturning({ 1: [row(2, 10), row(100, 5)] });

    const result = await fetchCollaborative(client, [seed(1, "A"), seed(2, "B")], {
      collaborativeSeeds: 1,
    });

    expect(result.has(2)).toBe(false);
    expect(result.has(100)).toBe(true);
  });
});
