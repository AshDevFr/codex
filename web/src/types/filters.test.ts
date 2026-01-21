import { describe, expect, it } from "vitest";
import type { FilterGroupState, SeriesCondition } from "./filters";
import {
	countActiveFilters,
	createEmptyFilterGroup,
	createEmptySeriesFilterState,
	filterGroupToConditions,
	getExcludedValues,
	getIncludedValues,
	hasActiveFilters,
	parseFilterGroup,
	parseSeriesFilters,
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
	});
});
