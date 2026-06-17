import { describe, expect, it } from "vitest";
import type {
  BookCondition,
  FilterGroupState,
  SeriesCondition,
} from "./filters";
import {
  bookFilterStateToCondition,
  conditionToBookFilterState,
  conditionToSeriesFilterState,
  countActiveFilters,
  countBookActiveFilters,
  createEmptyBookFilterState,
  createEmptyFilterGroup,
  createEmptySeriesFilterState,
  filterGroupToConditions,
  getExcludedValues,
  getIncludedValues,
  hasActiveFilters,
  parseBookFilters,
  parseFilterGroup,
  parseSeriesFilters,
  serializeBookFilters,
  serializeFilterGroup,
  serializeSeriesFilters,
  seriesFilterStateToCondition,
} from "./filters";

describe("Filter Types - Helper Functions", () => {
  describe("createEmptyFilterGroup", () => {
    it("should create an empty filter group with anyOf mode", () => {
      const group = createEmptyFilterGroup();

      expect(group.mode).toBe("anyOf");
      expect(group.values.size).toBe(0);
    });
  });

  describe("createEmptySeriesFilterState", () => {
    it("should create empty state for all filter groups", () => {
      const state = createEmptySeriesFilterState();

      expect(state.genres.values.size).toBe(0);
      expect(state.tags.values.size).toBe(0);
      expect(state.status.values.size).toBe(0);
      expect(state.readStatus.values.size).toBe(0);
      expect(state.publisher.values.size).toBe(0);
      expect(state.language.values.size).toBe(0);
      expect(state.sharingTags.values.size).toBe(0);
      expect(state.completion).toBe("neutral");
    });
  });

  describe("hasActiveFilters", () => {
    it("should return false for empty group", () => {
      const group = createEmptyFilterGroup();
      expect(hasActiveFilters(group)).toBe(false);
    });

    it("should return false for group with only neutral values", () => {
      const group: FilterGroupState = {
        mode: "anyOf",
        values: new Map([
          ["action", "neutral"],
          ["comedy", "neutral"],
        ]),
      };
      expect(hasActiveFilters(group)).toBe(false);
    });

    it("should return true for group with include value", () => {
      const group: FilterGroupState = {
        mode: "anyOf",
        values: new Map([["action", "include"]]),
      };
      expect(hasActiveFilters(group)).toBe(true);
    });

    it("should return true for group with exclude value", () => {
      const group: FilterGroupState = {
        mode: "anyOf",
        values: new Map([["horror", "exclude"]]),
      };
      expect(hasActiveFilters(group)).toBe(true);
    });
  });

  describe("countActiveFilters", () => {
    it("should return 0 for empty group", () => {
      const group = createEmptyFilterGroup();
      expect(countActiveFilters(group)).toBe(0);
    });

    it("should count only non-neutral values", () => {
      const group: FilterGroupState = {
        mode: "anyOf",
        values: new Map([
          ["action", "include"],
          ["comedy", "neutral"],
          ["horror", "exclude"],
        ]),
      };
      expect(countActiveFilters(group)).toBe(2);
    });
  });

  describe("getIncludedValues", () => {
    it("should return only included values", () => {
      const group: FilterGroupState = {
        mode: "anyOf",
        values: new Map([
          ["action", "include"],
          ["comedy", "neutral"],
          ["horror", "exclude"],
          ["drama", "include"],
        ]),
      };
      expect(getIncludedValues(group)).toEqual(["action", "drama"]);
    });
  });

  describe("getExcludedValues", () => {
    it("should return only excluded values", () => {
      const group: FilterGroupState = {
        mode: "anyOf",
        values: new Map([
          ["action", "include"],
          ["horror", "exclude"],
          ["thriller", "exclude"],
        ]),
      };
      expect(getExcludedValues(group)).toEqual(["horror", "thriller"]);
    });
  });
});

describe("Filter Types - Condition Building", () => {
  describe("filterGroupToConditions", () => {
    it("should return empty array for empty group", () => {
      const group = createEmptyFilterGroup();
      const conditions = filterGroupToConditions(group, "genre");
      expect(conditions).toEqual([]);
    });

    it("should create single include condition", () => {
      const group: FilterGroupState = {
        mode: "anyOf",
        values: new Map([["action", "include"]]),
      };
      const conditions = filterGroupToConditions(group, "genre");

      expect(conditions).toHaveLength(1);
      expect(conditions[0]).toEqual({
        genre: { operator: "is", value: "action" },
      });
    });

    it("should wrap multiple includes in anyOf for anyOf mode", () => {
      const group: FilterGroupState = {
        mode: "anyOf",
        values: new Map([
          ["action", "include"],
          ["comedy", "include"],
        ]),
      };
      const conditions = filterGroupToConditions(group, "genre");

      expect(conditions).toHaveLength(1);
      expect(conditions[0]).toHaveProperty("anyOf");
      const anyOf = (conditions[0] as { anyOf: SeriesCondition[] }).anyOf;
      expect(anyOf).toHaveLength(2);
    });

    it("should wrap multiple includes in allOf for allOf mode", () => {
      const group: FilterGroupState = {
        mode: "allOf",
        values: new Map([
          ["action", "include"],
          ["comedy", "include"],
        ]),
      };
      const conditions = filterGroupToConditions(group, "genre");

      expect(conditions).toHaveLength(1);
      expect(conditions[0]).toHaveProperty("allOf");
      const allOf = (conditions[0] as { allOf: SeriesCondition[] }).allOf;
      expect(allOf).toHaveLength(2);
      expect(allOf).toContainEqual({
        genre: { operator: "is", value: "action" },
      });
      expect(allOf).toContainEqual({
        genre: { operator: "is", value: "comedy" },
      });
    });

    it("should create exclude conditions with isNot operator", () => {
      const group: FilterGroupState = {
        mode: "anyOf",
        values: new Map([["horror", "exclude"]]),
      };
      const conditions = filterGroupToConditions(group, "genre");

      expect(conditions).toHaveLength(1);
      expect(conditions[0]).toEqual({
        genre: { operator: "isNot", value: "horror" },
      });
    });

    it("should combine includes and excludes in same wrapper", () => {
      const group: FilterGroupState = {
        mode: "anyOf",
        values: new Map([
          ["action", "include"],
          ["horror", "exclude"],
        ]),
      };
      const conditions = filterGroupToConditions(group, "genre");

      // Both include and exclude should be wrapped in the group's mode (anyOf)
      expect(conditions).toHaveLength(1);
      expect(conditions[0]).toHaveProperty("anyOf");
      const anyOf = (conditions[0] as { anyOf: SeriesCondition[] }).anyOf;
      expect(anyOf).toHaveLength(2);
      expect(anyOf).toContainEqual({
        genre: { operator: "is", value: "action" },
      });
      expect(anyOf).toContainEqual({
        genre: { operator: "isNot", value: "horror" },
      });
    });

    it("should combine includes and excludes in allOf wrapper for allOf mode", () => {
      const group: FilterGroupState = {
        mode: "allOf",
        values: new Map([
          ["action", "include"],
          ["horror", "exclude"],
        ]),
      };
      const conditions = filterGroupToConditions(group, "genre");

      // Both include and exclude should be wrapped in the group's mode (allOf)
      expect(conditions).toHaveLength(1);
      expect(conditions[0]).toHaveProperty("allOf");
      const allOf = (conditions[0] as { allOf: SeriesCondition[] }).allOf;
      expect(allOf).toHaveLength(2);
      expect(allOf).toContainEqual({
        genre: { operator: "is", value: "action" },
      });
      expect(allOf).toContainEqual({
        genre: { operator: "isNot", value: "horror" },
      });
    });
  });

  describe("seriesFilterStateToCondition", () => {
    it("should return undefined for empty state", () => {
      const state = createEmptySeriesFilterState();
      const condition = seriesFilterStateToCondition(state);
      expect(condition).toBeUndefined();
    });

    it("should return single condition for single filter", () => {
      const state = createEmptySeriesFilterState();
      state.genres.values.set("action", "include");

      const condition = seriesFilterStateToCondition(state);

      expect(condition).toEqual({
        genre: { operator: "is", value: "action" },
      });
    });

    it("should wrap multiple conditions in allOf", () => {
      const state = createEmptySeriesFilterState();
      state.genres.values.set("action", "include");
      state.tags.values.set("favorite", "include");

      const condition = seriesFilterStateToCondition(state);

      expect(condition).toHaveProperty("allOf");
      const allOf = (condition as { allOf: SeriesCondition[] }).allOf;
      expect(allOf).toHaveLength(2);
    });

    it("should create completion condition with isTrue for include", () => {
      const state = createEmptySeriesFilterState();
      state.completion = "include";

      const condition = seriesFilterStateToCondition(state);

      expect(condition).toEqual({
        completion: { operator: "isTrue" },
      });
    });

    it("should create completion condition with isFalse for exclude", () => {
      const state = createEmptySeriesFilterState();
      state.completion = "exclude";

      const condition = seriesFilterStateToCondition(state);

      expect(condition).toEqual({
        completion: { operator: "isFalse" },
      });
    });

    it("should not create completion condition for neutral", () => {
      const state = createEmptySeriesFilterState();
      state.completion = "neutral";

      const condition = seriesFilterStateToCondition(state);

      expect(condition).toBeUndefined();
    });

    it("should combine completion with other conditions", () => {
      const state = createEmptySeriesFilterState();
      state.genres.values.set("action", "include");
      state.completion = "include";

      const condition = seriesFilterStateToCondition(state);

      expect(condition).toHaveProperty("allOf");
      const allOf = (condition as { allOf: SeriesCondition[] }).allOf;
      expect(allOf).toHaveLength(2);
      expect(allOf).toContainEqual({
        genre: { operator: "is", value: "action" },
      });
      expect(allOf).toContainEqual({
        completion: { operator: "isTrue" },
      });
    });
  });
});

describe("Filter Types - URL Serialization", () => {
  describe("serializeFilterGroup", () => {
    it("should return null for empty group", () => {
      const group = createEmptyFilterGroup();
      expect(serializeFilterGroup(group)).toBeNull();
    });

    it("should serialize single include", () => {
      const group: FilterGroupState = {
        mode: "anyOf",
        values: new Map([["action", "include"]]),
      };
      expect(serializeFilterGroup(group)).toBe("any:action");
    });

    it("should serialize multiple includes", () => {
      const group: FilterGroupState = {
        mode: "anyOf",
        values: new Map([
          ["action", "include"],
          ["comedy", "include"],
        ]),
      };
      expect(serializeFilterGroup(group)).toBe("any:action,comedy");
    });

    it("should serialize excludes with :- prefix", () => {
      const group: FilterGroupState = {
        mode: "anyOf",
        values: new Map([["horror", "exclude"]]),
      };
      expect(serializeFilterGroup(group)).toBe("any::-horror");
    });

    it("should serialize includes and excludes", () => {
      const group: FilterGroupState = {
        mode: "allOf",
        values: new Map([
          ["action", "include"],
          ["horror", "exclude"],
        ]),
      };
      expect(serializeFilterGroup(group)).toBe("all:action:-horror");
    });

    it("should use all for allOf mode", () => {
      const group: FilterGroupState = {
        mode: "allOf",
        values: new Map([["action", "include"]]),
      };
      expect(serializeFilterGroup(group)).toBe("all:action");
    });
  });

  describe("parseFilterGroup", () => {
    it("should return empty group for null", () => {
      const group = parseFilterGroup(null);
      expect(group.mode).toBe("anyOf");
      expect(group.values.size).toBe(0);
    });

    it("should parse single include", () => {
      const group = parseFilterGroup("any:action");
      expect(group.mode).toBe("anyOf");
      expect(group.values.get("action")).toBe("include");
    });

    it("should parse multiple includes", () => {
      const group = parseFilterGroup("any:action,comedy");
      expect(group.values.get("action")).toBe("include");
      expect(group.values.get("comedy")).toBe("include");
    });

    it("should parse excludes", () => {
      const group = parseFilterGroup("any::-horror");
      expect(group.values.get("horror")).toBe("exclude");
    });

    it("should parse includes and excludes", () => {
      const group = parseFilterGroup("all:action,comedy:-horror,thriller");
      expect(group.mode).toBe("allOf");
      expect(group.values.get("action")).toBe("include");
      expect(group.values.get("comedy")).toBe("include");
      expect(group.values.get("horror")).toBe("exclude");
      expect(group.values.get("thriller")).toBe("exclude");
    });

    it("should handle malformed input gracefully", () => {
      const group = parseFilterGroup("invalid");
      expect(group.mode).toBe("anyOf");
      expect(group.values.size).toBe(0);
    });
  });

  describe("roundtrip serialization", () => {
    it("should preserve state through serialize/parse cycle", () => {
      const original: FilterGroupState = {
        mode: "allOf",
        values: new Map([
          ["action", "include"],
          ["comedy", "include"],
          ["horror", "exclude"],
        ]),
      };

      const serialized = serializeFilterGroup(original);
      const parsed = parseFilterGroup(serialized);

      expect(parsed.mode).toBe(original.mode);
      expect(parsed.values.get("action")).toBe("include");
      expect(parsed.values.get("comedy")).toBe("include");
      expect(parsed.values.get("horror")).toBe("exclude");
    });
  });

  describe("serializeSeriesFilters / parseSeriesFilters", () => {
    it("should serialize and parse series filter state", () => {
      const state = createEmptySeriesFilterState();
      state.genres.values.set("action", "include");
      state.genres.mode = "allOf";
      state.tags.values.set("favorite", "include");
      state.status.values.set("ongoing", "include");

      const params = serializeSeriesFilters(state);
      const parsed = parseSeriesFilters(params);

      expect(parsed.genres.mode).toBe("allOf");
      expect(parsed.genres.values.get("action")).toBe("include");
      expect(parsed.tags.values.get("favorite")).toBe("include");
      expect(parsed.status.values.get("ongoing")).toBe("include");
    });

    it("should serialize completion include state", () => {
      const state = createEmptySeriesFilterState();
      state.completion = "include";

      const params = serializeSeriesFilters(state);

      expect(params.get("cf")).toBe("include");
    });

    it("should serialize completion exclude state", () => {
      const state = createEmptySeriesFilterState();
      state.completion = "exclude";

      const params = serializeSeriesFilters(state);

      expect(params.get("cf")).toBe("exclude");
    });

    it("should not serialize completion neutral state", () => {
      const state = createEmptySeriesFilterState();
      state.completion = "neutral";

      const params = serializeSeriesFilters(state);

      expect(params.has("cf")).toBe(false);
    });

    it("should parse completion include state", () => {
      const params = new URLSearchParams("cf=include");
      const parsed = parseSeriesFilters(params);

      expect(parsed.completion).toBe("include");
    });

    it("should parse completion exclude state", () => {
      const params = new URLSearchParams("cf=exclude");
      const parsed = parseSeriesFilters(params);

      expect(parsed.completion).toBe("exclude");
    });

    it("should default to neutral for missing or invalid completion param", () => {
      const params1 = new URLSearchParams();
      expect(parseSeriesFilters(params1).completion).toBe("neutral");

      const params2 = new URLSearchParams("cf=invalid");
      expect(parseSeriesFilters(params2).completion).toBe("neutral");
    });

    it("should roundtrip completion state", () => {
      const state = createEmptySeriesFilterState();
      state.completion = "include";
      state.genres.values.set("action", "include");

      const params = serializeSeriesFilters(state);
      const parsed = parseSeriesFilters(params);

      expect(parsed.completion).toBe("include");
      expect(parsed.genres.values.get("action")).toBe("include");
    });
  });
});

describe("Filter Types - Condition → UI state", () => {
  describe("conditionToSeriesFilterState", () => {
    it("returns an empty state for undefined", () => {
      const state = conditionToSeriesFilterState(undefined);
      expect(state).not.toBeNull();
      expect(state).toEqual(createEmptySeriesFilterState());
    });

    it("round-trips a single-field condition", () => {
      const original = createEmptySeriesFilterState();
      original.genres.values.set("action", "include");
      const condition = seriesFilterStateToCondition(original);

      const restored = conditionToSeriesFilterState(condition);
      expect(restored).not.toBeNull();
      expect(restored?.genres.values.get("action")).toBe("include");
    });

    it("round-trips a multi-group condition", () => {
      const original = createEmptySeriesFilterState();
      original.genres.values.set("Action", "include");
      original.genres.values.set("Horror", "exclude");
      original.genres.mode = "anyOf";
      original.tags.values.set("favorite", "include");
      original.status.values.set("ongoing", "include");
      original.completion = "include";
      original.hasUserRating = "exclude";
      original.inCollection = "exclude";
      const condition = seriesFilterStateToCondition(original);

      const restored = conditionToSeriesFilterState(condition);
      expect(restored).not.toBeNull();
      expect(restored?.genres.values.get("Action")).toBe("include");
      expect(restored?.genres.values.get("Horror")).toBe("exclude");
      expect(restored?.genres.mode).toBe("anyOf");
      expect(restored?.tags.values.get("favorite")).toBe("include");
      expect(restored?.status.values.get("ongoing")).toBe("include");
      expect(restored?.completion).toBe("include");
      expect(restored?.hasUserRating).toBe("exclude");
      expect(restored?.inCollection).toBe("exclude");
    });

    it("preserves allOf mode on multi-value groups", () => {
      const original = createEmptySeriesFilterState();
      original.tags.mode = "allOf";
      original.tags.values.set("a", "include");
      original.tags.values.set("b", "include");
      const condition = seriesFilterStateToCondition(original);

      const restored = conditionToSeriesFilterState(condition);
      expect(restored?.tags.mode).toBe("allOf");
      expect(restored?.tags.values.get("a")).toBe("include");
      expect(restored?.tags.values.get("b")).toBe("include");
    });

    it("returns null when the condition uses an unknown field", () => {
      const condition: SeriesCondition = {
        year: { operator: "eq", value: 2020 },
      };
      expect(conditionToSeriesFilterState(condition)).toBeNull();
    });

    it("returns null for nested allOf groups", () => {
      const condition: SeriesCondition = {
        allOf: [
          {
            allOf: [
              { genre: { operator: "is", value: "Action" } },
              {
                anyOf: [{ tag: { operator: "is", value: "favorite" } }],
              },
            ],
          },
        ],
      };
      expect(conditionToSeriesFilterState(condition)).toBeNull();
    });
  });

  describe("conditionToBookFilterState", () => {
    it("returns an empty state for undefined", () => {
      const state = conditionToBookFilterState(undefined);
      expect(state).not.toBeNull();
      expect(state).toEqual(createEmptyBookFilterState());
    });

    it("round-trips a multi-group condition", () => {
      const original = createEmptyBookFilterState();
      original.genres.values.set("Action", "include");
      original.tags.values.set("favorite", "exclude");
      original.bookType.values.set("manga", "include");
      original.bookType.values.set("comic", "include");
      original.readStatus.values.set("unread", "include");
      original.hasError = "exclude";
      original.inReadList = "include";
      const condition = bookFilterStateToCondition(original);

      const restored = conditionToBookFilterState(condition);
      expect(restored).not.toBeNull();
      expect(restored?.genres.values.get("Action")).toBe("include");
      expect(restored?.tags.values.get("favorite")).toBe("exclude");
      expect(restored?.bookType.values.get("manga")).toBe("include");
      expect(restored?.bookType.values.get("comic")).toBe("include");
      expect(restored?.readStatus.values.get("unread")).toBe("include");
      expect(restored?.hasError).toBe("exclude");
      expect(restored?.inReadList).toBe("include");
    });

    it("returns null when the condition uses an unknown field", () => {
      const condition: BookCondition = {
        path: { operator: "contains", value: "/manga/" },
      };
      expect(conditionToBookFilterState(condition)).toBeNull();
    });
  });
});

describe("Filter Types - Membership filters (inCollection / inReadList)", () => {
  describe("inCollection (series)", () => {
    it("maps include to inCollection isTrue", () => {
      const state = createEmptySeriesFilterState();
      state.inCollection = "include";
      expect(seriesFilterStateToCondition(state)).toEqual({
        inCollection: { operator: "isTrue" },
      });
    });

    it("maps exclude to inCollection isFalse", () => {
      const state = createEmptySeriesFilterState();
      state.inCollection = "exclude";
      expect(seriesFilterStateToCondition(state)).toEqual({
        inCollection: { operator: "isFalse" },
      });
    });

    it("omits the condition when neutral", () => {
      const state = createEmptySeriesFilterState();
      expect(seriesFilterStateToCondition(state)).toBeUndefined();
    });

    it("round-trips through the URL (icf key)", () => {
      const state = createEmptySeriesFilterState();
      state.inCollection = "include";

      const params = serializeSeriesFilters(state);
      expect(params.get("icf")).toBe("include");
      expect(parseSeriesFilters(params).inCollection).toBe("include");
    });

    it("does not serialize a neutral inCollection", () => {
      const params = serializeSeriesFilters(createEmptySeriesFilterState());
      expect(params.has("icf")).toBe(false);
    });

    it("defaults to neutral for a missing or invalid icf param", () => {
      expect(parseSeriesFilters(new URLSearchParams()).inCollection).toBe(
        "neutral",
      );
      expect(
        parseSeriesFilters(new URLSearchParams("icf=bogus")).inCollection,
      ).toBe("neutral");
    });

    it("round-trips through condition converters", () => {
      const original = createEmptySeriesFilterState();
      original.inCollection = "include";
      const restored = conditionToSeriesFilterState(
        seriesFilterStateToCondition(original),
      );
      expect(restored?.inCollection).toBe("include");
    });
  });

  describe("inReadList (books)", () => {
    it("maps include to inReadList isTrue", () => {
      const state = createEmptyBookFilterState();
      state.inReadList = "include";
      expect(bookFilterStateToCondition(state)).toEqual({
        inReadList: { operator: "isTrue" },
      });
    });

    it("maps exclude to inReadList isFalse", () => {
      const state = createEmptyBookFilterState();
      state.inReadList = "exclude";
      expect(bookFilterStateToCondition(state)).toEqual({
        inReadList: { operator: "isFalse" },
      });
    });

    it("round-trips through the URL (brlf key)", () => {
      const state = createEmptyBookFilterState();
      state.inReadList = "exclude";

      const params = serializeBookFilters(state);
      expect(params.get("brlf")).toBe("exclude");
      expect(parseBookFilters(params).inReadList).toBe("exclude");
    });

    it("defaults to neutral for a missing or invalid brlf param", () => {
      expect(parseBookFilters(new URLSearchParams()).inReadList).toBe(
        "neutral",
      );
      expect(
        parseBookFilters(new URLSearchParams("brlf=bogus")).inReadList,
      ).toBe("neutral");
    });

    it("counts as an active filter", () => {
      const state = createEmptyBookFilterState();
      expect(countBookActiveFilters(state)).toBe(0);
      state.inReadList = "include";
      expect(countBookActiveFilters(state)).toBe(1);
    });

    it("round-trips through condition converters", () => {
      const original = createEmptyBookFilterState();
      original.inReadList = "exclude";
      const restored = conditionToBookFilterState(
        bookFilterStateToCondition(original),
      );
      expect(restored?.inReadList).toBe("exclude");
    });
  });
});
