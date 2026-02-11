import type { MetadataMatchParams, SearchResult } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it } from "vitest";
import { scoreResult } from "./match.js";

function makeResult(overrides: Partial<SearchResult>): SearchResult {
  return {
    externalId: "1",
    title: "Test",
    alternateTitles: [],
    ...overrides,
  };
}

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
