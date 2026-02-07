import type { MetadataMatchParams, SearchResult } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it } from "vitest";
import { scoreResult, similarity } from "./match.js";

function makeResult(overrides: Partial<SearchResult>): SearchResult {
  return {
    externalId: "1",
    title: "Test",
    alternateTitles: [],
    ...overrides,
  };
}

describe("similarity", () => {
  it("should return 1.0 for exact case-insensitive match", () => {
    expect(similarity("Air", "Air")).toBe(1.0);
    expect(similarity("Air", "air")).toBe(1.0);
    expect(similarity("DRAGON BALL", "dragon ball")).toBe(1.0);
  });

  it("should return 0 when one string is empty", () => {
    expect(similarity("", "Air")).toBe(0);
    expect(similarity("Air", "")).toBe(0);
  });

  it("should return 1.0 when both strings are empty (identical)", () => {
    expect(similarity("", "")).toBe(1.0);
  });

  it("should penalize containment by length ratio", () => {
    // "Air" (3 chars) in "Air Gear" (8 chars): containment = 0.8 * 3/8 = 0.3
    // Jaccard: {"air"} vs {"air", "gear"} = 1/2 = 0.5
    const score = similarity("Air", "Air Gear");
    expect(score).toBeCloseTo(0.5, 2);
    expect(score).toBeLessThan(0.8);
  });

  it("should give reasonable score for similar-length containment", () => {
    // "Dragon Ball" (11 chars) in "Dragon Ball Z" (13 chars): containment = 0.8 * 11/13 ≈ 0.677
    // Jaccard: {"dragon", "ball"} vs {"dragon", "ball", "z"} = 2/3 ≈ 0.667
    const score = similarity("Dragon Ball", "Dragon Ball Z");
    expect(score).toBeGreaterThan(0.6);
    expect(score).toBeLessThan(0.8);
  });

  it("should return 0 for completely different strings", () => {
    expect(similarity("Air", "Naruto")).toBe(0);
    expect(similarity("One Piece", "Bleach")).toBe(0);
  });

  it("should handle single-word containment with length penalty", () => {
    // "Air" (3 chars) in "Airing" (6 chars): containment = 0.8 * 3/6 = 0.4
    // Jaccard: {"air"} vs {"airing"} = 0/2 = 0 (different words)
    const score = similarity("Air", "Airing");
    expect(score).toBeCloseTo(0.4, 2);
  });

  it("should use Jaccard when it produces higher score than containment", () => {
    // "Air" in "Air Gear": containment = 0.3, Jaccard = 0.5 -> Jaccard wins
    const score = similarity("Air", "Air Gear");
    expect(score).toBeCloseTo(0.5, 2);
  });

  it("should handle word overlap without containment", () => {
    // "One Piece" vs "Piece of Cake"
    // No containment (neither contains the other)
    // Jaccard: {"one", "piece"} vs {"piece", "of", "cake"} = 1/4 = 0.25
    const score = similarity("One Piece", "Piece of Cake");
    expect(score).toBeCloseTo(0.25, 2);
  });

  it("should be symmetric", () => {
    expect(similarity("Air", "Air Gear")).toBe(similarity("Air Gear", "Air"));
    expect(similarity("Naruto", "Naruto Shippuden")).toBe(similarity("Naruto Shippuden", "Naruto"));
  });
});

describe("scoreResult", () => {
  it("should score exact title match at 0.8 without year", () => {
    const result = makeResult({ title: "Air" });
    const params: MetadataMatchParams = { title: "Air" };
    expect(scoreResult(result, params)).toBeCloseTo(0.8, 2);
  });

  it("should score exact title match at 1.0 with matching year", () => {
    const result = makeResult({ title: "Air", year: 2005 });
    const params: MetadataMatchParams = { title: "Air", year: 2005 };
    expect(scoreResult(result, params)).toBeCloseTo(1.0, 2);
  });

  it("should score containment match significantly lower than exact", () => {
    const air = makeResult({ title: "Air" });
    const airGear = makeResult({ title: "Air Gear" });
    const params: MetadataMatchParams = { title: "Air" };

    const airScore = scoreResult(air, params);
    const airGearScore = scoreResult(airGear, params);

    expect(airScore).toBeGreaterThan(airGearScore + 0.2);
  });

  it("should prefer 'Air' over 'Air Gear' when searching for 'Air'", () => {
    const air = makeResult({ title: "Air", externalId: "air" });
    const airGear = makeResult({ title: "Air Gear", externalId: "air-gear" });
    const params: MetadataMatchParams = { title: "Air" };

    expect(scoreResult(air, params)).toBe(0.8);
    expect(scoreResult(airGear, params)).toBe(0.3);
  });

  it("should check alternate titles for best similarity", () => {
    const result = makeResult({
      title: "AIR (TV)",
      alternateTitles: ["Air", "エアー"],
    });
    const params: MetadataMatchParams = { title: "Air" };
    // Alternate title "Air" is an exact match -> similarity 1.0
    // Score = 1.0 * 0.6 + 0.2 (exact bonus) = 0.8
    expect(scoreResult(result, params)).toBeCloseTo(0.8, 2);
  });

  it("should give exact match bonus when only alternate title matches", () => {
    const result = makeResult({
      title: "Completely Different Title",
      alternateTitles: ["Air"],
    });
    const params: MetadataMatchParams = { title: "Air" };
    // bestSimilarity from "Air" alt = 1.0
    // Score = 1.0 * 0.6 + 0.2 (exact alt match) = 0.8
    expect(scoreResult(result, params)).toBeCloseTo(0.8, 2);
  });

  it("should give partial year credit for year off by 1", () => {
    const result = makeResult({ title: "Air", year: 2006 });
    const params: MetadataMatchParams = { title: "Air", year: 2005 };
    // 1.0 * 0.6 + 0.1 (year ±1) + 0.2 (exact) = 0.9
    expect(scoreResult(result, params)).toBeCloseTo(0.9, 2);
  });

  it("should give no year credit when years differ by more than 1", () => {
    const result = makeResult({ title: "Air", year: 2010 });
    const params: MetadataMatchParams = { title: "Air", year: 2005 };
    // 1.0 * 0.6 + 0 (year too far) + 0.2 (exact) = 0.8
    expect(scoreResult(result, params)).toBeCloseTo(0.8, 2);
  });

  it("should not give year credit when year is missing from result", () => {
    const result = makeResult({ title: "Air" });
    const params: MetadataMatchParams = { title: "Air", year: 2005 };
    expect(scoreResult(result, params)).toBeCloseTo(0.8, 2);
  });

  it("should cap score at 1.0", () => {
    const result = makeResult({ title: "Air", year: 2005 });
    const params: MetadataMatchParams = { title: "Air", year: 2005 };
    expect(scoreResult(result, params)).toBeLessThanOrEqual(1.0);
  });
});
