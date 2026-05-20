import { describe, expect, it } from "vitest";
import type { BookCondition, SeriesCondition } from "@/types/filters";
import {
  decodeCondition,
  encodeCondition,
  parseSearchUrl,
  type SearchUrlState,
  serializeSearchUrl,
} from "./searchUrl";

describe("searchUrl encode/decode condition", () => {
  it("round-trips a simple condition", () => {
    const c: SeriesCondition = {
      name: { operator: "contains", value: "one punch" },
    };
    const encoded = encodeCondition(c);
    expect(typeof encoded).toBe("string");
    expect(encoded).not.toMatch(/[+/=]/); // base64url, not standard base64

    const decoded = decodeCondition<SeriesCondition>(encoded);
    expect(decoded).toEqual(c);
  });

  it("round-trips a nested allOf/anyOf condition", () => {
    const c: BookCondition = {
      allOf: [
        { title: { operator: "contains", value: "one punch" } },
        { format: { operator: "is", value: "cbz" } },
        {
          anyOf: [
            { readStatus: { operator: "is", value: "unread" } },
            { readStatus: { operator: "is", value: "in_progress" } },
          ],
        },
      ],
    };
    const encoded = encodeCondition(c);
    const decoded = decodeCondition<BookCondition>(encoded);
    expect(decoded).toEqual(c);
  });

  it("returns null for malformed condition string", () => {
    expect(decodeCondition("not-valid-base64$$$")).toBeNull();
    expect(decodeCondition("dGhpcyBpcyBub3QganNvbg")).toBeNull(); // "this is not json"
  });
});

describe("searchUrl serialize/parse round-trip", () => {
  it("preserves all fields", () => {
    const state: SearchUrlState = {
      query: "one punch",
      sort: "year,desc",
      tab: "books",
      page: 3,
      condition: {
        allOf: [
          { title: { operator: "contains", value: "punch" } },
          { format: { operator: "is", value: "cbz" } },
        ],
      },
    };
    const { params, conditionDropped } = serializeSearchUrl(state);
    expect(conditionDropped).toBe(false);
    expect(params.get("q")).toBe("one punch");
    expect(params.get("sort")).toBe("year,desc");
    expect(params.get("tab")).toBe("books");
    expect(params.get("page")).toBe("3");
    expect(params.get("c")).toBeTruthy();

    const parsed = parseSearchUrl(params);
    expect(parsed).toEqual(state);
  });

  it("omits defaults from URL", () => {
    const state: SearchUrlState = {
      query: "",
      sort: "",
      tab: "series",
      page: 1,
    };
    const { params } = serializeSearchUrl(state);
    expect(params.toString()).toBe("");

    const parsed = parseSearchUrl(params);
    expect(parsed).toEqual({
      query: "",
      sort: "",
      tab: "series",
      page: 1,
      condition: undefined,
    });
  });

  it("drops the condition when it exceeds the length cap and flags it", () => {
    const huge: SeriesCondition = {
      allOf: Array.from({ length: 200 }, (_, i) => ({
        tag: { operator: "is" as const, value: `tag-${i}-with-padding-data` },
      })),
    };
    const { params, conditionDropped } = serializeSearchUrl(
      { query: "", sort: "", tab: "series", page: 1, condition: huge },
      { maxConditionLength: 100 },
    );
    expect(conditionDropped).toBe(true);
    expect(params.get("c")).toBeNull();
  });

  it("falls back to defaults when the URL has garbage", () => {
    const params = new URLSearchParams({
      tab: "unknown",
      page: "not-a-number",
      c: "not-base64$$$",
    });
    const parsed = parseSearchUrl(params);
    expect(parsed.tab).toBe("series");
    expect(parsed.page).toBe(1);
    expect(parsed.condition).toBeUndefined();
  });
});
