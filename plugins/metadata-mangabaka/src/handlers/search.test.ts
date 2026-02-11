import type { SearchResult } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it } from "vitest";
import { scoreSearchResult } from "./search.js";

function makeResult(overrides: Partial<SearchResult>): SearchResult {
  return {
    externalId: "1",
    title: "Test",
    alternateTitles: [],
    ...overrides,
  };
}

describe("scoreSearchResult", () => {
  it("should return 1.0 for exact title match", () => {
    const result = makeResult({ title: "Air" });
    expect(scoreSearchResult(result, "Air")).toBe(1.0);
  });

  it("should return 1.0 for case-insensitive exact match", () => {
    const result = makeResult({ title: "One Piece" });
    expect(scoreSearchResult(result, "one piece")).toBe(1.0);
  });

  it("should score partial containment lower than exact match", () => {
    const exact = makeResult({ title: "Air" });
    const partial = makeResult({ title: "Air Gear" });

    const exactScore = scoreSearchResult(exact, "Air");
    const partialScore = scoreSearchResult(partial, "Air");

    expect(exactScore).toBeGreaterThan(partialScore);
    expect(exactScore).toBe(1.0);
    expect(partialScore).toBeLessThan(0.8);
  });

  it("should check alternate titles and use the best score", () => {
    const result = makeResult({
      title: "AIR (TV)",
      alternateTitles: ["Air"],
    });
    // Alternate title "Air" is an exact match -> 1.0
    expect(scoreSearchResult(result, "Air")).toBe(1.0);
  });

  it("should prefer primary title exact match over alternate partial match", () => {
    const primary = makeResult({ title: "Air", alternateTitles: [] });
    const alternate = makeResult({
      title: "Something Else",
      alternateTitles: ["Air Gear"],
    });

    expect(scoreSearchResult(primary, "Air")).toBeGreaterThan(scoreSearchResult(alternate, "Air"));
  });

  it("should return 0 for completely unrelated titles", () => {
    const result = makeResult({ title: "Naruto" });
    expect(scoreSearchResult(result, "Bleach")).toBe(0);
  });

  it("should sort results correctly when used for ranking", () => {
    const results = [
      makeResult({ title: "Air Gear", externalId: "air-gear" }),
      makeResult({ title: "Airmaster", externalId: "airmaster" }),
      makeResult({ title: "Air", externalId: "air" }),
      makeResult({
        title: "AIR (TV)",
        externalId: "air-tv",
        alternateTitles: ["Air"],
      }),
    ];

    const scored = results
      .map((r) => ({ result: r, score: scoreSearchResult(r, "Air") }))
      .sort((a, b) => b.score - a.score);

    // Exact matches should come first
    expect(scored[0].result.externalId).toBe("air");
    // Alternate title exact match should be tied with primary exact match
    expect(scored[1].result.externalId).toBe("air-tv");
    // Partial matches should come after
    expect(scored[0].score).toBeGreaterThan(scored[2].score);
    expect(scored[0].score).toBeGreaterThan(scored[3].score);
  });
});
