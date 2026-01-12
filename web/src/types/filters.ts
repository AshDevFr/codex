/**
 * Filter types for the advanced filtering system
 *
 * These types mirror the backend filter DTOs and provide a type-safe way
 * to build filter conditions for series and books.
 */

// =============================================================================
// Tri-state for filter chips
// =============================================================================

/**
 * The three states a filter value can be in:
 * - neutral: not part of the filter (ignored)
 * - include: must have this value (or any of included values in "any" mode)
 * - exclude: must NOT have this value
 */
export type TriState = "neutral" | "include" | "exclude";

// =============================================================================
// Field operators (matches backend FieldOperator)
// =============================================================================

export type FieldOperator =
	| { operator: "is"; value: string }
	| { operator: "isNot"; value: string }
	| { operator: "isNull" }
	| { operator: "isNotNull" }
	| { operator: "contains"; value: string }
	| { operator: "doesNotContain"; value: string }
	| { operator: "beginsWith"; value: string }
	| { operator: "endsWith"; value: string };

// =============================================================================
// UUID operators (matches backend UuidOperator)
// =============================================================================

export type UuidOperator = { operator: "is"; value: string } | { operator: "isNot"; value: string };

// =============================================================================
// Boolean operators (matches backend BoolOperator)
// =============================================================================

export type BoolOperator = { operator: "isTrue" } | { operator: "isFalse" };

// =============================================================================
// Series conditions (matches backend SeriesCondition)
// =============================================================================

export type SeriesCondition =
	| { allOf: SeriesCondition[] }
	| { anyOf: SeriesCondition[] }
	| { libraryId: UuidOperator }
	| { genre: FieldOperator }
	| { tag: FieldOperator }
	| { status: FieldOperator }
	| { publisher: FieldOperator }
	| { language: FieldOperator }
	| { name: FieldOperator }
	| { readStatus: FieldOperator };

// =============================================================================
// Book conditions (matches backend BookCondition)
// =============================================================================

export type BookCondition =
	| { allOf: BookCondition[] }
	| { anyOf: BookCondition[] }
	| { libraryId: UuidOperator }
	| { seriesId: UuidOperator }
	| { genre: FieldOperator }
	| { tag: FieldOperator }
	| { title: FieldOperator }
	| { readStatus: FieldOperator }
	| { hasError: BoolOperator };

// =============================================================================
// Request types (matches backend SeriesListRequest/BookListRequest)
// =============================================================================

export interface SeriesListRequest {
	condition?: SeriesCondition;
	search?: string;
	page?: number;
	pageSize?: number;
	sort?: string;
}

export interface BookListRequest {
	condition?: BookCondition;
	search?: string;
	page?: number;
	pageSize?: number;
	sort?: string;
}

// =============================================================================
// UI state types
// =============================================================================

/**
 * Mode for combining filter values within a group
 * - allOf: all included values must match (AND)
 * - anyOf: any included value can match (OR)
 */
export type FilterMode = "allOf" | "anyOf";

/**
 * State for a single filter group (e.g., genres, tags)
 */
export interface FilterGroupState {
	mode: FilterMode;
	values: Map<string, TriState>;
}

/**
 * UI state for series filters
 */
export interface SeriesFilterState {
	genres: FilterGroupState;
	tags: FilterGroupState;
	status: FilterGroupState;
	readStatus: FilterGroupState;
	publisher: FilterGroupState;
	language: FilterGroupState;
}

/**
 * UI state for book filters
 */
export interface BookFilterState {
	genres: FilterGroupState;
	tags: FilterGroupState;
	readStatus: FilterGroupState;
	hasError: TriState;
}

// =============================================================================
// Helper functions
// =============================================================================

/**
 * Create an empty filter group state
 */
export function createEmptyFilterGroup(): FilterGroupState {
	return {
		mode: "anyOf",
		values: new Map(),
	};
}

/**
 * Create empty series filter state
 */
export function createEmptySeriesFilterState(): SeriesFilterState {
	return {
		genres: createEmptyFilterGroup(),
		tags: createEmptyFilterGroup(),
		status: createEmptyFilterGroup(),
		readStatus: createEmptyFilterGroup(),
		publisher: createEmptyFilterGroup(),
		language: createEmptyFilterGroup(),
	};
}

/**
 * Create empty book filter state
 */
export function createEmptyBookFilterState(): BookFilterState {
	return {
		genres: createEmptyFilterGroup(),
		tags: createEmptyFilterGroup(),
		readStatus: createEmptyFilterGroup(),
		hasError: "neutral",
	};
}

/**
 * Check if a filter group has any active filters
 */
export function hasActiveFilters(group: FilterGroupState): boolean {
	for (const state of group.values.values()) {
		if (state !== "neutral") {
			return true;
		}
	}
	return false;
}

/**
 * Count the number of active filters in a group
 */
export function countActiveFilters(group: FilterGroupState): number {
	let count = 0;
	for (const state of group.values.values()) {
		if (state !== "neutral") {
			count++;
		}
	}
	return count;
}

/**
 * Get included values from a filter group
 */
export function getIncludedValues(group: FilterGroupState): string[] {
	const values: string[] = [];
	for (const [value, state] of group.values) {
		if (state === "include") {
			values.push(value);
		}
	}
	return values;
}

/**
 * Get excluded values from a filter group
 */
export function getExcludedValues(group: FilterGroupState): string[] {
	const values: string[] = [];
	for (const [value, state] of group.values) {
		if (state === "exclude") {
			values.push(value);
		}
	}
	return values;
}

/**
 * Convert a filter group to API conditions
 */
export function filterGroupToConditions<T extends "genre" | "tag" | "status" | "readStatus" | "publisher" | "language">(
	group: FilterGroupState,
	field: T,
): SeriesCondition[] {
	const conditions: SeriesCondition[] = [];
	const includes = getIncludedValues(group);
	const excludes = getExcludedValues(group);

	// Build include conditions
	if (includes.length > 0) {
		const includeConditions = includes.map((value) => ({
			[field]: { operator: "is" as const, value },
		})) as SeriesCondition[];

		if (group.mode === "allOf") {
			// All must match - add each as separate condition
			conditions.push(...includeConditions);
		} else {
			// Any can match - wrap in anyOf
			if (includeConditions.length === 1) {
				conditions.push(includeConditions[0]);
			} else {
				conditions.push({ anyOf: includeConditions });
			}
		}
	}

	// Build exclude conditions (always AND - must not have ANY of them)
	for (const value of excludes) {
		conditions.push({ [field]: { operator: "isNot" as const, value } } as SeriesCondition);
	}

	return conditions;
}

/**
 * Convert UI filter state to API condition
 */
export function seriesFilterStateToCondition(state: SeriesFilterState): SeriesCondition | undefined {
	const allConditions: SeriesCondition[] = [];

	// Add genre conditions
	allConditions.push(...filterGroupToConditions(state.genres, "genre"));

	// Add tag conditions
	allConditions.push(...filterGroupToConditions(state.tags, "tag"));

	// Add status conditions
	allConditions.push(...filterGroupToConditions(state.status, "status"));

	// Add read status conditions
	allConditions.push(...filterGroupToConditions(state.readStatus, "readStatus"));

	// Add publisher conditions
	allConditions.push(...filterGroupToConditions(state.publisher, "publisher"));

	// Add language conditions
	allConditions.push(...filterGroupToConditions(state.language, "language"));

	// Return combined condition
	if (allConditions.length === 0) {
		return undefined;
	}
	if (allConditions.length === 1) {
		return allConditions[0];
	}
	return { allOf: allConditions };
}

/**
 * Convert a book filter group to API conditions
 */
export function bookFilterGroupToConditions<T extends "genre" | "tag" | "readStatus">(
	group: FilterGroupState,
	field: T,
): BookCondition[] {
	const conditions: BookCondition[] = [];
	const includes = getIncludedValues(group);
	const excludes = getExcludedValues(group);

	// Build include conditions
	if (includes.length > 0) {
		const includeConditions = includes.map((value) => ({
			[field]: { operator: "is" as const, value },
		})) as BookCondition[];

		if (group.mode === "allOf") {
			// All must match - add each as separate condition
			conditions.push(...includeConditions);
		} else {
			// Any can match - wrap in anyOf
			if (includeConditions.length === 1) {
				conditions.push(includeConditions[0]);
			} else {
				conditions.push({ anyOf: includeConditions });
			}
		}
	}

	// Build exclude conditions (always AND - must not have ANY of them)
	for (const value of excludes) {
		conditions.push({ [field]: { operator: "isNot" as const, value } } as BookCondition);
	}

	return conditions;
}

/**
 * Convert UI book filter state to API condition
 */
export function bookFilterStateToCondition(state: BookFilterState): BookCondition | undefined {
	const allConditions: BookCondition[] = [];

	// Add genre conditions
	allConditions.push(...bookFilterGroupToConditions(state.genres, "genre"));

	// Add tag conditions
	allConditions.push(...bookFilterGroupToConditions(state.tags, "tag"));

	// Add read status conditions
	allConditions.push(...bookFilterGroupToConditions(state.readStatus, "readStatus"));

	// Add hasError condition
	if (state.hasError === "include") {
		allConditions.push({ hasError: { operator: "isTrue" } });
	} else if (state.hasError === "exclude") {
		allConditions.push({ hasError: { operator: "isFalse" } });
	}

	// Return combined condition
	if (allConditions.length === 0) {
		return undefined;
	}
	if (allConditions.length === 1) {
		return allConditions[0];
	}
	return { allOf: allConditions };
}

/**
 * Count active filters in book filter state
 */
export function countBookActiveFilters(state: BookFilterState): number {
	let count = 0;
	count += countActiveFilters(state.genres);
	count += countActiveFilters(state.tags);
	count += countActiveFilters(state.readStatus);
	if (state.hasError !== "neutral") count++;
	return count;
}

/**
 * Check if book filter state has any active filters
 */
export function hasBookActiveFilters(state: BookFilterState): boolean {
	return countBookActiveFilters(state) > 0;
}

// =============================================================================
// URL serialization
// =============================================================================

/**
 * Serialize a filter group to URL parameter format
 * Format: mode:include1,include2:-exclude1,exclude2
 * Example: any:Action,Comedy:-Horror
 */
export function serializeFilterGroup(group: FilterGroupState): string | null {
	const includes = getIncludedValues(group);
	const excludes = getExcludedValues(group);

	if (includes.length === 0 && excludes.length === 0) {
		return null;
	}

	const mode = group.mode === "allOf" ? "all" : "any";
	const includeStr = includes.join(",");
	const excludeStr = excludes.length > 0 ? `:-${excludes.join(",")}` : "";

	return `${mode}:${includeStr}${excludeStr}`;
}

/**
 * Parse a filter group from URL parameter format
 */
export function parseFilterGroup(param: string | null): FilterGroupState {
	const group = createEmptyFilterGroup();

	if (!param) {
		return group;
	}

	// Split mode from rest
	const colonIndex = param.indexOf(":");
	if (colonIndex === -1) {
		return group;
	}

	const modeStr = param.slice(0, colonIndex);
	const rest = param.slice(colonIndex + 1);

	group.mode = modeStr === "all" ? "allOf" : "anyOf";

	// Split includes from excludes
	const parts = rest.split(":-");
	const includesStr = parts[0] || "";
	const excludesStr = parts[1] || "";

	// Parse includes
	if (includesStr) {
		for (const value of includesStr.split(",")) {
			if (value) {
				group.values.set(value, "include");
			}
		}
	}

	// Parse excludes
	if (excludesStr) {
		for (const value of excludesStr.split(",")) {
			if (value) {
				group.values.set(value, "exclude");
			}
		}
	}

	return group;
}

/**
 * URL parameter keys for filter groups
 */
export const FILTER_PARAM_KEYS = {
	genres: "gf",
	tags: "tf",
	status: "sf",
	readStatus: "rf",
	publisher: "pf",
	language: "lf",
} as const;

/**
 * Serialize series filter state to URL search params
 */
export function serializeSeriesFilters(state: SeriesFilterState): URLSearchParams {
	const params = new URLSearchParams();

	const genreParam = serializeFilterGroup(state.genres);
	if (genreParam) params.set(FILTER_PARAM_KEYS.genres, genreParam);

	const tagParam = serializeFilterGroup(state.tags);
	if (tagParam) params.set(FILTER_PARAM_KEYS.tags, tagParam);

	const statusParam = serializeFilterGroup(state.status);
	if (statusParam) params.set(FILTER_PARAM_KEYS.status, statusParam);

	const readStatusParam = serializeFilterGroup(state.readStatus);
	if (readStatusParam) params.set(FILTER_PARAM_KEYS.readStatus, readStatusParam);

	const publisherParam = serializeFilterGroup(state.publisher);
	if (publisherParam) params.set(FILTER_PARAM_KEYS.publisher, publisherParam);

	const languageParam = serializeFilterGroup(state.language);
	if (languageParam) params.set(FILTER_PARAM_KEYS.language, languageParam);

	return params;
}

/**
 * Parse series filter state from URL search params
 */
export function parseSeriesFilters(params: URLSearchParams): SeriesFilterState {
	return {
		genres: parseFilterGroup(params.get(FILTER_PARAM_KEYS.genres)),
		tags: parseFilterGroup(params.get(FILTER_PARAM_KEYS.tags)),
		status: parseFilterGroup(params.get(FILTER_PARAM_KEYS.status)),
		readStatus: parseFilterGroup(params.get(FILTER_PARAM_KEYS.readStatus)),
		publisher: parseFilterGroup(params.get(FILTER_PARAM_KEYS.publisher)),
		language: parseFilterGroup(params.get(FILTER_PARAM_KEYS.language)),
	};
}

/**
 * URL parameter keys for book filter groups
 */
export const BOOK_FILTER_PARAM_KEYS = {
	genres: "bgf",
	tags: "btf",
	readStatus: "brf",
	hasError: "bef",
} as const;

/**
 * Serialize book filter state to URL search params
 */
export function serializeBookFilters(state: BookFilterState): URLSearchParams {
	const params = new URLSearchParams();

	const genreParam = serializeFilterGroup(state.genres);
	if (genreParam) params.set(BOOK_FILTER_PARAM_KEYS.genres, genreParam);

	const tagParam = serializeFilterGroup(state.tags);
	if (tagParam) params.set(BOOK_FILTER_PARAM_KEYS.tags, tagParam);

	const readStatusParam = serializeFilterGroup(state.readStatus);
	if (readStatusParam) params.set(BOOK_FILTER_PARAM_KEYS.readStatus, readStatusParam);

	if (state.hasError !== "neutral") {
		params.set(BOOK_FILTER_PARAM_KEYS.hasError, state.hasError);
	}

	return params;
}

/**
 * Parse book filter state from URL search params
 */
export function parseBookFilters(params: URLSearchParams): BookFilterState {
	const hasErrorParam = params.get(BOOK_FILTER_PARAM_KEYS.hasError);
	return {
		genres: parseFilterGroup(params.get(BOOK_FILTER_PARAM_KEYS.genres)),
		tags: parseFilterGroup(params.get(BOOK_FILTER_PARAM_KEYS.tags)),
		readStatus: parseFilterGroup(params.get(BOOK_FILTER_PARAM_KEYS.readStatus)),
		hasError: hasErrorParam === "include" || hasErrorParam === "exclude" ? hasErrorParam : "neutral",
	};
}
