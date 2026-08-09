import type { Recommendation } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it } from "vitest";
import type { Candidate } from "./filters.js";
import { applyAuthorAdjustment, SAME_AUTHOR_PENALTY } from "./scoring.js";

/** A candidate with a given score and same-author flag. */
function candidate(id: number, score: number, matchedAuthor = false): Candidate {
  return {
    entry: { score, matched_author: matchedAuthor, series: { id } },
    recommendation: { externalId: String(id), score } as Recommendation,
  };
}

describe("applyAuthorAdjustment", () => {
  it("leaves entries by other authors untouched", () => {
    const [result] = applyAuthorAdjustment([candidate(1, 0.8)]);

    expect(result.recommendation.score).toBe(0.8);
  });

  it("penalises same-author entries rather than dropping them", () => {
    // An author's unrelated other work is frequently the best recommendation
    // available, so it is de-ranked, not removed.
    const [result] = applyAuthorAdjustment([candidate(1, 0.8, true)]);

    expect(result).toBeDefined();
    expect(result.recommendation.score).toBeLessThan(0.8);
    expect(result.recommendation.score).toBeCloseTo(0.8 * SAME_AUTHOR_PENALTY, 2);
  });

  it("reorders a same-author entry below a stronger unrelated one", () => {
    const adjusted = applyAuthorAdjustment([candidate(1, 0.9, true), candidate(2, 0.7)]);
    const sorted = [...adjusted].sort((a, b) => b.recommendation.score - a.recommendation.score);

    expect(sorted[0].recommendation.externalId).toBe("2");
  });

  it("keeps a strongly-matched same-author entry ahead of a weak unrelated one", () => {
    // The penalty must not be so blunt that it buries good recommendations.
    const adjusted = applyAuthorAdjustment([candidate(1, 0.9, true), candidate(2, 0.2)]);
    const sorted = [...adjusted].sort((a, b) => b.recommendation.score - a.recommendation.score);

    expect(sorted[0].recommendation.externalId).toBe("1");
  });

  it("drops same-author entries when the user opts out of them", () => {
    const adjusted = applyAuthorAdjustment([candidate(1, 0.9, true), candidate(2, 0.7)], {
      excludeSameAuthor: true,
    });

    expect(adjusted.map((c) => c.recommendation.externalId)).toEqual(["2"]);
  });

  it("accepts a custom penalty", () => {
    const [result] = applyAuthorAdjustment([candidate(1, 1, true)], { penalty: 0.5 });

    expect(result.recommendation.score).toBe(0.5);
  });

  it("rounds the adjusted score to two decimals", () => {
    // Matches the precision the mapper already applies; an unrounded product
    // reintroduces the noise digits the mapper stripped.
    const [result] = applyAuthorAdjustment([candidate(1, 0.77, true)], { penalty: 0.6 });

    expect(result.recommendation.score).toBe(0.46);
  });

  it("never produces a negative or out-of-range score", () => {
    const [low] = applyAuthorAdjustment([candidate(1, 0, true)]);
    const [high] = applyAuthorAdjustment([candidate(2, 1, true)], { penalty: 5 });

    expect(low.recommendation.score).toBe(0);
    expect(high.recommendation.score).toBe(1);
  });

  it("does not mutate the input candidates", () => {
    const input = candidate(1, 0.8, true);

    applyAuthorAdjustment([input]);

    expect(input.recommendation.score).toBe(0.8);
  });

  it("handles an empty input", () => {
    expect(applyAuthorAdjustment([])).toEqual([]);
  });

  it("treats a missing matched_author flag as not-same-author", () => {
    const [result] = applyAuthorAdjustment([
      {
        entry: { score: 0.8, series: { id: 1 } },
        recommendation: { externalId: "1", score: 0.8 } as Recommendation,
      },
    ]);

    expect(result.recommendation.score).toBe(0.8);
  });
});
