import { describe, expect, it } from "vitest";
import type { SeriesCondition } from "@/types/filters";
import { countRuleConditions, describesPersonalData } from "./collectionRules";

const tag = (value: string): SeriesCondition => ({
  tag: { operator: "is", value },
});

describe("describesPersonalData", () => {
  it("is false for a rule over library metadata only", () => {
    expect(describesPersonalData(tag("isekai"))).toBe(false);
    expect(
      describesPersonalData({ anyOf: [tag("isekai"), tag("mecha")] }),
    ).toBe(false);
  });

  it("is false for nothing at all", () => {
    expect(describesPersonalData(null)).toBe(false);
    expect(describesPersonalData(undefined)).toBe(false);
  });

  // These three fields resolve against the viewer, so the collection holds
  // different series for different people. The UI has to say so.
  it("detects each personal field", () => {
    expect(
      describesPersonalData({ userRating: { operator: "gte", value: 85 } }),
    ).toBe(true);
    expect(
      describesPersonalData({ readStatus: { operator: "is", value: "read" } }),
    ).toBe(true);
    expect(
      describesPersonalData({ hasUserRating: { operator: "isTrue" } }),
    ).toBe(true);
  });

  it("is false for communityRating, which is the same for everyone", () => {
    expect(
      describesPersonalData({
        communityRating: { operator: "gte", value: 85 },
      }),
    ).toBe(false);
  });

  it("finds a personal field nested in groups", () => {
    const deep: SeriesCondition = {
      allOf: [
        tag("isekai"),
        {
          anyOf: [
            tag("mecha"),
            { allOf: [{ userRating: { operator: "gte", value: 85 } }] },
          ],
        },
      ],
    };
    expect(describesPersonalData(deep)).toBe(true);
  });
});

describe("countRuleConditions", () => {
  it("counts a single leaf", () => {
    expect(countRuleConditions(tag("isekai"))).toBe(1);
  });

  it("counts leaves, not groups", () => {
    expect(countRuleConditions({ anyOf: [tag("a"), tag("b")] })).toBe(2);
    expect(
      countRuleConditions({
        allOf: [tag("a"), { anyOf: [tag("b"), tag("c")] }],
      }),
    ).toBe(3);
  });

  it("is zero for nothing", () => {
    expect(countRuleConditions(null)).toBe(0);
    expect(countRuleConditions(undefined)).toBe(0);
    expect(countRuleConditions({ allOf: [] })).toBe(0);
  });
});
