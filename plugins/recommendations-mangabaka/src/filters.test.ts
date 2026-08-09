import type { Recommendation } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it } from "vitest";
import fixture from "./__fixtures__/mix-response.json" with { type: "json" };
import {
  type Candidate,
  collapseFranchises,
  type FilterContext,
  filterCandidates,
  relatedIdsOf,
} from "./filters.js";
import { mapRecommendation } from "./mappers.js";
import { LibraryIndex } from "./seeds.js";
import type { MbRecommendationEntry, MbSeries } from "./types.js";

const fixtureEntries = fixture.data as unknown as MbRecommendationEntry[];

/** Pair an upstream entry with its mapped recommendation. */
function candidate(entry: MbRecommendationEntry): Candidate {
  const recommendation = mapRecommendation(entry, {
    seedTitles: new Map(),
    library: new LibraryIndex(),
  }) as Recommendation;
  return { entry, recommendation };
}

function ctx(overrides: Partial<FilterContext> = {}): FilterContext {
  return {
    seedIds: new Set<number>(),
    seedTitleKeys: new Set<string>(),
    excludeIds: new Set<string>(),
    isDismissed: () => false,
    ...overrides,
  };
}

function series(overrides: Partial<MbSeries> = {}): MbSeries {
  return { id: 1, title: "Test", ...overrides };
}

describe("relatedIdsOf", () => {
  it("reads the relationships map", () => {
    expect(relatedIdsOf(series({ relationships: { other: [5, 6], sequel: [7] } }))).toEqual(
      new Set([5, 6, 7]),
    );
  });

  it("reads relationships_v2", () => {
    expect(
      relatedIdsOf(series({ relationships_v2: [{ to_series_id: 9, relation_type: "sequel" }] })),
    ).toEqual(new Set([9]));
  });

  it("unions both shapes, which do not always agree", () => {
    // Observed upstream: relationships_v2 carried an ID absent from relationships.
    const ids = relatedIdsOf(
      series({
        relationships: { other: [1, 2] },
        relationships_v2: [{ to_series_id: 3 }],
      }),
    );

    expect(ids).toEqual(new Set([1, 2, 3]));
  });

  it("returns an empty set when there are no relationships", () => {
    expect(relatedIdsOf(series())).toEqual(new Set());
  });
});

describe("filterCandidates", () => {
  it("drops entries upstream flags as related to a seed", () => {
    const kept = filterCandidates(
      [candidate({ score: 0.9, matched_related: true, series: series({ id: 100 }) })],
      ctx(),
    );

    expect(kept).toEqual([]);
  });

  it("drops entries whose relationships name a seed, even without the flag", () => {
    // matched_related is a beta annotation; the relationship data is the
    // independent check.
    const kept = filterCandidates(
      [
        candidate({
          score: 0.9,
          matched_related: false,
          series: series({ id: 100, relationships: { other: [42] } }),
        }),
      ],
      ctx({ seedIds: new Set([42]) }),
    );

    expect(kept).toEqual([]);
  });

  it("drops entries whose normalised title collides with a seed title", () => {
    const kept = filterCandidates(
      [candidate({ score: 0.9, series: series({ id: 100, title: "Solo  Leveling" }) })],
      ctx({ seedTitleKeys: new Set(["solo leveling"]) }),
    );

    expect(kept).toEqual([]);
  });

  it("keeps a series whose title merely resembles a seed", () => {
    // Exact-key matching only: "Solo Leveling Ragnarok" is a different series.
    const kept = filterCandidates(
      [candidate({ score: 0.9, series: series({ id: 100, title: "Solo Leveling Ragnarok" }) })],
      ctx({ seedTitleKeys: new Set(["solo leveling"]) }),
    );

    expect(kept).toHaveLength(1);
  });

  it("drops host-excluded IDs", () => {
    const kept = filterCandidates(
      [candidate({ score: 0.9, series: series({ id: 100 }) })],
      ctx({ excludeIds: new Set(["100"]) }),
    );

    expect(kept).toEqual([]);
  });

  it("drops dismissed IDs", () => {
    const kept = filterCandidates(
      [candidate({ score: 0.9, series: series({ id: 100 }) })],
      ctx({ isDismissed: (id) => id === "100" }),
    );

    expect(kept).toEqual([]);
  });

  it("drops seeds echoed back as recommendations", () => {
    const kept = filterCandidates(
      [candidate({ score: 0.9, series: series({ id: 42 }) })],
      ctx({ seedIds: new Set([42]) }),
    );

    expect(kept).toEqual([]);
  });

  it("keeps an unrelated, unexcluded candidate", () => {
    const kept = filterCandidates(
      [candidate({ score: 0.9, series: series({ id: 100, title: "Something Else" }) })],
      ctx({ seedIds: new Set([42]), seedTitleKeys: new Set(["berserk"]) }),
    );

    expect(kept).toHaveLength(1);
  });

  describe("against the captured Re:Zero response", () => {
    const seedIds = new Set([3397, 84926]);

    it("removes every franchise entry and keeps only genuine recommendations", () => {
      // The regression case. Untreated, six of these eight results are
      // franchise members of a seed.
      const kept = filterCandidates(fixtureEntries.map(candidate), ctx({ seedIds }));

      expect(kept.map((c) => c.entry.series.id).sort((a, b) => a - b)).toEqual([808, 7559]);
    });

    it("removes the Solo Leveling novel, which is a distinct ID from the manhwa seed", () => {
      // Not caught by the seed-echo check: different MangaBaka ID entirely.
      const kept = filterCandidates(fixtureEntries.map(candidate), ctx({ seedIds }));

      expect(kept.map((c) => c.entry.series.id)).not.toContain(85266);
    });
  });
});

describe("collapseFranchises", () => {
  it("keeps only the strongest entry of a related cluster", () => {
    const kept = collapseFranchises([
      candidate({ score: 0.5, series: series({ id: 1, relationships: { other: [2] } }) }),
      candidate({ score: 0.9, series: series({ id: 2, relationships: { other: [1] } }) }),
    ]);

    expect(kept).toHaveLength(1);
    expect(kept[0].entry.series.id).toBe(2);
  });

  it("collapses a cluster linked in only one direction", () => {
    // Relationship data is not always symmetric.
    const kept = collapseFranchises([
      candidate({ score: 0.9, series: series({ id: 1, relationships: { other: [2] } }) }),
      candidate({ score: 0.5, series: series({ id: 2 }) }),
    ]);

    expect(kept).toHaveLength(1);
    expect(kept[0].entry.series.id).toBe(1);
  });

  it("collapses a chain transitively", () => {
    // 1-2 and 2-3 are linked, so all three are one franchise.
    const kept = collapseFranchises([
      candidate({ score: 0.5, series: series({ id: 1, relationships: { other: [2] } }) }),
      candidate({ score: 0.7, series: series({ id: 2, relationships: { other: [3] } }) }),
      candidate({ score: 0.6, series: series({ id: 3 }) }),
    ]);

    expect(kept).toHaveLength(1);
    expect(kept[0].entry.series.id).toBe(2);
  });

  it("collapses volumes sharing a title even with no relationship data", () => {
    const kept = collapseFranchises([
      candidate({ score: 0.4, series: series({ id: 1, title: "Berserk, Vol. 1" }) }),
      candidate({ score: 0.8, series: series({ id: 2, title: "Berserk Vol. 2" }) }),
    ]);

    expect(kept).toHaveLength(1);
    expect(kept[0].entry.series.id).toBe(2);
  });

  it("keeps unrelated series with distinct titles", () => {
    const kept = collapseFranchises([
      candidate({ score: 0.9, series: series({ id: 1, title: "Berserk" }) }),
      candidate({ score: 0.8, series: series({ id: 2, title: "Vinland Saga" }) }),
      candidate({ score: 0.7, series: series({ id: 3, title: "Vagabond" }) }),
    ]);

    expect(kept).toHaveLength(3);
  });

  it("does not merge distinct series that link to a common third party", () => {
    // Both reference series 99, which is not itself a candidate. Sharing one
    // relative does not make two series the same franchise.
    const kept = collapseFranchises([
      candidate({ score: 0.9, series: series({ id: 1, title: "A", relationships: { o: [99] } }) }),
      candidate({ score: 0.8, series: series({ id: 2, title: "B", relationships: { o: [99] } }) }),
    ]);

    expect(kept).toHaveLength(2);
  });

  it("preserves input order among surviving entries", () => {
    const kept = collapseFranchises([
      candidate({ score: 0.9, series: series({ id: 1, title: "A" }) }),
      candidate({ score: 0.8, series: series({ id: 2, title: "B" }) }),
    ]);

    expect(kept.map((c) => c.entry.series.id)).toEqual([1, 2]);
  });

  it("handles an empty input", () => {
    expect(collapseFranchises([])).toEqual([]);
  });
});
