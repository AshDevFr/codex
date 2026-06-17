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

export type UuidOperator =
  | { operator: "is"; value: string }
  | { operator: "isNot"; value: string };

// =============================================================================
// Boolean operators (matches backend BoolOperator)
// =============================================================================

export type BoolOperator = { operator: "isTrue" } | { operator: "isFalse" };

// =============================================================================
// Number operators (matches backend NumberOperator)
// =============================================================================

export type NumberOperator =
  | { operator: "eq"; value: number }
  | { operator: "ne"; value: number }
  | { operator: "gt"; value: number }
  | { operator: "gte"; value: number }
  | { operator: "lt"; value: number }
  | { operator: "lte"; value: number }
  | { operator: "between"; min?: number | null; max?: number | null }
  | { operator: "isNull" }
  | { operator: "isNotNull" };

// =============================================================================
// Date operators (matches backend DateOperator)
//
// Values are ISO-8601 UTC strings (the same shape that comes back over the
// wire). Open-ended ranges set `start`/`end` to null.
// =============================================================================

export type DateOperator =
  | { operator: "after"; value: string }
  | { operator: "before"; value: string }
  | { operator: "onOrAfter"; value: string }
  | { operator: "onOrBefore"; value: string }
  | { operator: "between"; start?: string | null; end?: string | null }
  | { operator: "isNull" }
  | { operator: "isNotNull" };

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
  | { title: FieldOperator }
  | { titleSort: FieldOperator }
  | { readStatus: FieldOperator }
  | { sharingTag: FieldOperator }
  | { completion: BoolOperator }
  | { hasExternalSourceId: BoolOperator }
  | { hasUserRating: BoolOperator }
  | { isTracked: BoolOperator }
  | { inCollection: BoolOperator }
  | { year: NumberOperator }
  | { author: FieldOperator }
  | { path: FieldOperator }
  | { dateAdded: DateOperator };

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
  | { titleSort: FieldOperator }
  | { readStatus: FieldOperator }
  | { hasError: BoolOperator }
  | { inReadList: BoolOperator }
  | { bookType: FieldOperator }
  | { path: FieldOperator }
  | { format: FieldOperator }
  | { pageCount: NumberOperator }
  | { dateAdded: DateOperator };

// =============================================================================
// Request types (matches backend SeriesListRequest/BookListRequest)
// =============================================================================

export interface SeriesListRequest {
  condition?: SeriesCondition;
  fullTextSearch?: string;
  page?: number;
  pageSize?: number;
  sort?: string;
}

export interface BookListRequest {
  condition?: BookCondition;
  fullTextSearch?: string;
  page?: number;
  pageSize?: number;
  sort?: string;
  includeDeleted?: boolean;
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
  sharingTags: FilterGroupState;
  completion: TriState;
  hasExternalSourceId: TriState;
  hasUserRating: TriState;
  isTracked: TriState;
  inCollection: TriState;
}

/**
 * UI state for book filters
 */
export interface BookFilterState {
  genres: FilterGroupState;
  tags: FilterGroupState;
  readStatus: FilterGroupState;
  bookType: FilterGroupState;
  hasError: TriState;
  inReadList: TriState;
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
    sharingTags: createEmptyFilterGroup(),
    completion: "neutral",
    hasExternalSourceId: "neutral",
    hasUserRating: "neutral",
    isTracked: "neutral",
    inCollection: "neutral",
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
    bookType: createEmptyFilterGroup(),
    hasError: "neutral",
    inReadList: "neutral",
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
export function filterGroupToConditions<
  T extends
    | "genre"
    | "tag"
    | "status"
    | "readStatus"
    | "publisher"
    | "language"
    | "sharingTag",
>(group: FilterGroupState, field: T): SeriesCondition[] {
  const includes = getIncludedValues(group);
  const excludes = getExcludedValues(group);

  // Build all conditions for this group (includes and excludes)
  const includeConditions = includes.map((value) => ({
    [field]: { operator: "is" as const, value },
  })) as SeriesCondition[];

  const excludeConditions = excludes.map((value) => ({
    [field]: { operator: "isNot" as const, value },
  })) as SeriesCondition[];

  const allGroupConditions = [...includeConditions, ...excludeConditions];

  // If no conditions, return empty array
  if (allGroupConditions.length === 0) {
    return [];
  }

  // If only one condition, return it directly
  if (allGroupConditions.length === 1) {
    return allGroupConditions;
  }

  // Wrap all conditions in the group's mode (allOf or anyOf)
  if (group.mode === "allOf") {
    return [{ allOf: allGroupConditions }];
  } else {
    return [{ anyOf: allGroupConditions }];
  }
}

/**
 * Convert UI filter state to API condition
 */
export function seriesFilterStateToCondition(
  state: SeriesFilterState,
): SeriesCondition | undefined {
  const allConditions: SeriesCondition[] = [];

  // Add genre conditions
  allConditions.push(...filterGroupToConditions(state.genres, "genre"));

  // Add tag conditions
  allConditions.push(...filterGroupToConditions(state.tags, "tag"));

  // Add status conditions
  allConditions.push(...filterGroupToConditions(state.status, "status"));

  // Add read status conditions
  allConditions.push(
    ...filterGroupToConditions(state.readStatus, "readStatus"),
  );

  // Add publisher conditions
  allConditions.push(...filterGroupToConditions(state.publisher, "publisher"));

  // Add language conditions
  allConditions.push(...filterGroupToConditions(state.language, "language"));

  // Add sharing tag conditions
  allConditions.push(
    ...filterGroupToConditions(state.sharingTags, "sharingTag"),
  );

  // Add completion condition
  if (state.completion === "include") {
    allConditions.push({ completion: { operator: "isTrue" } });
  } else if (state.completion === "exclude") {
    allConditions.push({ completion: { operator: "isFalse" } });
  }

  // Add hasExternalSourceId condition
  if (state.hasExternalSourceId === "include") {
    allConditions.push({ hasExternalSourceId: { operator: "isTrue" } });
  } else if (state.hasExternalSourceId === "exclude") {
    allConditions.push({ hasExternalSourceId: { operator: "isFalse" } });
  }

  // Add hasUserRating condition
  if (state.hasUserRating === "include") {
    allConditions.push({ hasUserRating: { operator: "isTrue" } });
  } else if (state.hasUserRating === "exclude") {
    allConditions.push({ hasUserRating: { operator: "isFalse" } });
  }

  // Add isTracked condition
  if (state.isTracked === "include") {
    allConditions.push({ isTracked: { operator: "isTrue" } });
  } else if (state.isTracked === "exclude") {
    allConditions.push({ isTracked: { operator: "isFalse" } });
  }

  // Add inCollection condition
  if (state.inCollection === "include") {
    allConditions.push({ inCollection: { operator: "isTrue" } });
  } else if (state.inCollection === "exclude") {
    allConditions.push({ inCollection: { operator: "isFalse" } });
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
 * Convert a book filter group to API conditions
 */
export function bookFilterGroupToConditions<
  T extends "genre" | "tag" | "readStatus" | "bookType",
>(group: FilterGroupState, field: T): BookCondition[] {
  const includes = getIncludedValues(group);
  const excludes = getExcludedValues(group);

  // Build all conditions for this group (includes and excludes)
  const includeConditions = includes.map((value) => ({
    [field]: { operator: "is" as const, value },
  })) as BookCondition[];

  const excludeConditions = excludes.map((value) => ({
    [field]: { operator: "isNot" as const, value },
  })) as BookCondition[];

  const allGroupConditions = [...includeConditions, ...excludeConditions];

  // If no conditions, return empty array
  if (allGroupConditions.length === 0) {
    return [];
  }

  // If only one condition, return it directly
  if (allGroupConditions.length === 1) {
    return allGroupConditions;
  }

  // Wrap all conditions in the group's mode (allOf or anyOf)
  if (group.mode === "allOf") {
    return [{ allOf: allGroupConditions }];
  } else {
    return [{ anyOf: allGroupConditions }];
  }
}

/**
 * Convert UI book filter state to API condition
 */
export function bookFilterStateToCondition(
  state: BookFilterState,
): BookCondition | undefined {
  const allConditions: BookCondition[] = [];

  // Add genre conditions
  allConditions.push(...bookFilterGroupToConditions(state.genres, "genre"));

  // Add tag conditions
  allConditions.push(...bookFilterGroupToConditions(state.tags, "tag"));

  // Add read status conditions
  allConditions.push(
    ...bookFilterGroupToConditions(state.readStatus, "readStatus"),
  );

  // Add book type conditions
  allConditions.push(
    ...bookFilterGroupToConditions(state.bookType, "bookType"),
  );

  // Add hasError condition
  if (state.hasError === "include") {
    allConditions.push({ hasError: { operator: "isTrue" } });
  } else if (state.hasError === "exclude") {
    allConditions.push({ hasError: { operator: "isFalse" } });
  }

  // Add inReadList condition
  if (state.inReadList === "include") {
    allConditions.push({ inReadList: { operator: "isTrue" } });
  } else if (state.inReadList === "exclude") {
    allConditions.push({ inReadList: { operator: "isFalse" } });
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

// =============================================================================
// Condition → UI state (reverse converters used by list-page preset apply)
//
// The chip-based list-page UIs serialize their state through
// `seriesFilterStateToCondition` / `bookFilterStateToCondition`. The shape they
// produce is a strict subset of the full grammar:
//
//   * A top-level `allOf` of group conditions, or a single group condition.
//   * Each group is either a single leaf or a `allOf`/`anyOf` of same-field
//     leaves with `is`/`isNot` (or a bool leaf for the four boolean toggles).
//
// These reverse converters round-trip that subset. Conditions that fall
// outside it (nested groups, fields not exposed by the chip UI, custom
// operators) cause the visitor to bail and the caller can refuse to apply
// the preset.
// =============================================================================

const SERIES_FIELD_GROUPS: Record<
  string,
  keyof Omit<
    SeriesFilterState,
    | "completion"
    | "hasExternalSourceId"
    | "hasUserRating"
    | "isTracked"
    | "inCollection"
  >
> = {
  genre: "genres",
  tag: "tags",
  status: "status",
  readStatus: "readStatus",
  publisher: "publisher",
  language: "language",
  sharingTag: "sharingTags",
};

const SERIES_BOOL_FIELDS = new Set([
  "completion",
  "hasExternalSourceId",
  "hasUserRating",
  "isTracked",
  "inCollection",
]);

function applySeriesLeaf(
  state: SeriesFilterState,
  leaf: Record<string, unknown>,
  groupMode?: FilterMode,
): boolean {
  const fieldKeys = Object.keys(leaf);
  if (fieldKeys.length !== 1) return false;
  const field = fieldKeys[0];
  const op = leaf[field] as { operator?: string; value?: string };
  if (!op || typeof op !== "object" || typeof op.operator !== "string") {
    return false;
  }

  // Bool fields land directly on the state.
  if (SERIES_BOOL_FIELDS.has(field)) {
    const tri: TriState | null =
      op.operator === "isTrue"
        ? "include"
        : op.operator === "isFalse"
          ? "exclude"
          : null;
    if (tri === null) return false;
    (state as unknown as Record<string, TriState>)[field] = tri;
    return true;
  }

  // Field-operator backed group filters.
  const groupKey = SERIES_FIELD_GROUPS[field];
  if (!groupKey) return false;

  const group = state[groupKey];
  if (groupMode) group.mode = groupMode;
  if (op.operator === "is" && typeof op.value === "string") {
    group.values.set(op.value, "include");
    return true;
  }
  if (op.operator === "isNot" && typeof op.value === "string") {
    group.values.set(op.value, "exclude");
    return true;
  }
  return false;
}

function applySeriesGroup(
  state: SeriesFilterState,
  items: SeriesCondition[],
  groupMode: FilterMode,
): boolean {
  if (items.length === 0) return true;
  for (const item of items) {
    if (typeof item !== "object" || item === null) return false;
    if (!applySeriesLeaf(state, item as Record<string, unknown>, groupMode)) {
      return false;
    }
  }
  return true;
}

function applySeriesItem(
  state: SeriesFilterState,
  item: SeriesCondition,
): boolean {
  const record = item as Record<string, unknown>;
  if (Array.isArray(record.anyOf)) {
    return applySeriesGroup(state, record.anyOf as SeriesCondition[], "anyOf");
  }
  if (Array.isArray(record.allOf)) {
    return applySeriesGroup(state, record.allOf as SeriesCondition[], "allOf");
  }
  return applySeriesLeaf(state, record);
}

// Returns the shared field name when every item is a single-field leaf that
// references the same field (and not allOf/anyOf). Used to distinguish a
// "single group with multiple values" top-level wrapper from a "multi-group"
// wrapper, since the forward converter unwraps both into the same `allOf`
// shape when no other group is present.
function sharedLeafField(items: readonly unknown[]): string | null {
  if (items.length === 0) return null;
  let field: string | null = null;
  for (const item of items) {
    if (typeof item !== "object" || item === null) return null;
    const keys = Object.keys(item as Record<string, unknown>);
    if (keys.length !== 1) return null;
    const key = keys[0];
    if (key === "allOf" || key === "anyOf") return null;
    if (field === null) field = key;
    else if (field !== key) return null;
  }
  return field;
}

/**
 * Convert a saved condition back into the chip-based SeriesFilterState used
 * by the library list page. Returns `null` when the condition uses fields or
 * shapes that the chip UI can't represent — the caller should surface an
 * error instead of silently dropping filters.
 */
export function conditionToSeriesFilterState(
  condition: SeriesCondition | undefined | null,
): SeriesFilterState | null {
  const state = createEmptySeriesFilterState();
  if (!condition) return state;

  const record = condition as Record<string, unknown>;

  if (Array.isArray(record.allOf)) {
    const items = record.allOf as SeriesCondition[];
    // Single-group wrapper (e.g. genres in allOf mode is the only active
    // group): forward conversion produces `{allOf: [<leaf>, <leaf>]}` with no
    // outer wrapper, so we reapply the mode here.
    if (sharedLeafField(items)) {
      return applySeriesGroup(state, items, "allOf") ? state : null;
    }
    for (const item of items) {
      if (!applySeriesItem(state, item)) return null;
    }
    return state;
  }

  if (Array.isArray(record.anyOf)) {
    const items = record.anyOf as SeriesCondition[];
    // Top-level anyOf is only emitted for a single anyOf group; mixed-field
    // anyOf at the top isn't expressible in the chip UI.
    if (items.length > 0 && !sharedLeafField(items)) return null;
    return applySeriesGroup(state, items, "anyOf") ? state : null;
  }

  return applySeriesItem(state, condition) ? state : null;
}

const BOOK_FIELD_GROUPS: Record<
  string,
  keyof Omit<BookFilterState, "hasError" | "inReadList">
> = {
  genre: "genres",
  tag: "tags",
  readStatus: "readStatus",
  bookType: "bookType",
};

function applyBookLeaf(
  state: BookFilterState,
  leaf: Record<string, unknown>,
  groupMode?: FilterMode,
): boolean {
  const fieldKeys = Object.keys(leaf);
  if (fieldKeys.length !== 1) return false;
  const field = fieldKeys[0];
  const op = leaf[field] as { operator?: string; value?: string };
  if (!op || typeof op !== "object" || typeof op.operator !== "string") {
    return false;
  }

  if (field === "hasError") {
    if (op.operator === "isTrue") {
      state.hasError = "include";
      return true;
    }
    if (op.operator === "isFalse") {
      state.hasError = "exclude";
      return true;
    }
    return false;
  }

  if (field === "inReadList") {
    if (op.operator === "isTrue") {
      state.inReadList = "include";
      return true;
    }
    if (op.operator === "isFalse") {
      state.inReadList = "exclude";
      return true;
    }
    return false;
  }

  const groupKey = BOOK_FIELD_GROUPS[field];
  if (!groupKey) return false;

  const group = state[groupKey];
  if (groupMode) group.mode = groupMode;
  if (op.operator === "is" && typeof op.value === "string") {
    group.values.set(op.value, "include");
    return true;
  }
  if (op.operator === "isNot" && typeof op.value === "string") {
    group.values.set(op.value, "exclude");
    return true;
  }
  return false;
}

function applyBookGroup(
  state: BookFilterState,
  items: BookCondition[],
  groupMode: FilterMode,
): boolean {
  if (items.length === 0) return true;
  for (const item of items) {
    if (typeof item !== "object" || item === null) return false;
    if (!applyBookLeaf(state, item as Record<string, unknown>, groupMode)) {
      return false;
    }
  }
  return true;
}

function applyBookItem(state: BookFilterState, item: BookCondition): boolean {
  const record = item as Record<string, unknown>;
  if (Array.isArray(record.anyOf)) {
    return applyBookGroup(state, record.anyOf as BookCondition[], "anyOf");
  }
  if (Array.isArray(record.allOf)) {
    return applyBookGroup(state, record.allOf as BookCondition[], "allOf");
  }
  return applyBookLeaf(state, record);
}

/**
 * Convert a saved condition back into the chip-based BookFilterState. See
 * conditionToSeriesFilterState for caveats.
 */
export function conditionToBookFilterState(
  condition: BookCondition | undefined | null,
): BookFilterState | null {
  const state = createEmptyBookFilterState();
  if (!condition) return state;

  const record = condition as Record<string, unknown>;

  if (Array.isArray(record.allOf)) {
    const items = record.allOf as BookCondition[];
    if (sharedLeafField(items)) {
      return applyBookGroup(state, items, "allOf") ? state : null;
    }
    for (const item of items) {
      if (!applyBookItem(state, item)) return null;
    }
    return state;
  }

  if (Array.isArray(record.anyOf)) {
    const items = record.anyOf as BookCondition[];
    if (items.length > 0 && !sharedLeafField(items)) return null;
    return applyBookGroup(state, items, "anyOf") ? state : null;
  }

  return applyBookItem(state, condition) ? state : null;
}

/**
 * Count active filters in book filter state
 */
export function countBookActiveFilters(state: BookFilterState): number {
  let count = 0;
  count += countActiveFilters(state.genres);
  count += countActiveFilters(state.tags);
  count += countActiveFilters(state.readStatus);
  count += countActiveFilters(state.bookType);
  if (state.hasError !== "neutral") count++;
  if (state.inReadList !== "neutral") count++;
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
  sharingTags: "stf",
  completion: "cf",
  hasExternalSourceId: "esf",
  hasUserRating: "urf",
  isTracked: "trf",
  inCollection: "icf",
} as const;

/**
 * Serialize series filter state to URL search params
 */
export function serializeSeriesFilters(
  state: SeriesFilterState,
): URLSearchParams {
  const params = new URLSearchParams();

  const genreParam = serializeFilterGroup(state.genres);
  if (genreParam) params.set(FILTER_PARAM_KEYS.genres, genreParam);

  const tagParam = serializeFilterGroup(state.tags);
  if (tagParam) params.set(FILTER_PARAM_KEYS.tags, tagParam);

  const statusParam = serializeFilterGroup(state.status);
  if (statusParam) params.set(FILTER_PARAM_KEYS.status, statusParam);

  const readStatusParam = serializeFilterGroup(state.readStatus);
  if (readStatusParam)
    params.set(FILTER_PARAM_KEYS.readStatus, readStatusParam);

  const publisherParam = serializeFilterGroup(state.publisher);
  if (publisherParam) params.set(FILTER_PARAM_KEYS.publisher, publisherParam);

  const languageParam = serializeFilterGroup(state.language);
  if (languageParam) params.set(FILTER_PARAM_KEYS.language, languageParam);

  const sharingTagParam = serializeFilterGroup(state.sharingTags);
  if (sharingTagParam)
    params.set(FILTER_PARAM_KEYS.sharingTags, sharingTagParam);

  if (state.completion !== "neutral") {
    params.set(FILTER_PARAM_KEYS.completion, state.completion);
  }

  if (state.hasExternalSourceId !== "neutral") {
    params.set(
      FILTER_PARAM_KEYS.hasExternalSourceId,
      state.hasExternalSourceId,
    );
  }

  if (state.hasUserRating !== "neutral") {
    params.set(FILTER_PARAM_KEYS.hasUserRating, state.hasUserRating);
  }

  if (state.isTracked !== "neutral") {
    params.set(FILTER_PARAM_KEYS.isTracked, state.isTracked);
  }

  if (state.inCollection !== "neutral") {
    params.set(FILTER_PARAM_KEYS.inCollection, state.inCollection);
  }

  return params;
}

/**
 * Parse series filter state from URL search params
 */
export function parseSeriesFilters(params: URLSearchParams): SeriesFilterState {
  const completionParam = params.get(FILTER_PARAM_KEYS.completion);
  const hasExternalSourceIdParam = params.get(
    FILTER_PARAM_KEYS.hasExternalSourceId,
  );
  const hasUserRatingParam = params.get(FILTER_PARAM_KEYS.hasUserRating);
  const isTrackedParam = params.get(FILTER_PARAM_KEYS.isTracked);
  const inCollectionParam = params.get(FILTER_PARAM_KEYS.inCollection);
  return {
    genres: parseFilterGroup(params.get(FILTER_PARAM_KEYS.genres)),
    tags: parseFilterGroup(params.get(FILTER_PARAM_KEYS.tags)),
    status: parseFilterGroup(params.get(FILTER_PARAM_KEYS.status)),
    readStatus: parseFilterGroup(params.get(FILTER_PARAM_KEYS.readStatus)),
    publisher: parseFilterGroup(params.get(FILTER_PARAM_KEYS.publisher)),
    language: parseFilterGroup(params.get(FILTER_PARAM_KEYS.language)),
    sharingTags: parseFilterGroup(params.get(FILTER_PARAM_KEYS.sharingTags)),
    completion:
      completionParam === "include" || completionParam === "exclude"
        ? completionParam
        : "neutral",
    hasExternalSourceId:
      hasExternalSourceIdParam === "include" ||
      hasExternalSourceIdParam === "exclude"
        ? hasExternalSourceIdParam
        : "neutral",
    hasUserRating:
      hasUserRatingParam === "include" || hasUserRatingParam === "exclude"
        ? hasUserRatingParam
        : "neutral",
    isTracked:
      isTrackedParam === "include" || isTrackedParam === "exclude"
        ? isTrackedParam
        : "neutral",
    inCollection:
      inCollectionParam === "include" || inCollectionParam === "exclude"
        ? inCollectionParam
        : "neutral",
  };
}

/**
 * URL parameter keys for book filter groups
 */
export const BOOK_FILTER_PARAM_KEYS = {
  genres: "bgf",
  tags: "btf",
  readStatus: "brf",
  bookType: "bbt",
  hasError: "bef",
  inReadList: "brlf",
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
  if (readStatusParam)
    params.set(BOOK_FILTER_PARAM_KEYS.readStatus, readStatusParam);

  const bookTypeParam = serializeFilterGroup(state.bookType);
  if (bookTypeParam) params.set(BOOK_FILTER_PARAM_KEYS.bookType, bookTypeParam);

  if (state.hasError !== "neutral") {
    params.set(BOOK_FILTER_PARAM_KEYS.hasError, state.hasError);
  }

  if (state.inReadList !== "neutral") {
    params.set(BOOK_FILTER_PARAM_KEYS.inReadList, state.inReadList);
  }

  return params;
}

/**
 * Parse book filter state from URL search params
 */
export function parseBookFilters(params: URLSearchParams): BookFilterState {
  const hasErrorParam = params.get(BOOK_FILTER_PARAM_KEYS.hasError);
  const inReadListParam = params.get(BOOK_FILTER_PARAM_KEYS.inReadList);
  return {
    genres: parseFilterGroup(params.get(BOOK_FILTER_PARAM_KEYS.genres)),
    tags: parseFilterGroup(params.get(BOOK_FILTER_PARAM_KEYS.tags)),
    readStatus: parseFilterGroup(params.get(BOOK_FILTER_PARAM_KEYS.readStatus)),
    bookType: parseFilterGroup(params.get(BOOK_FILTER_PARAM_KEYS.bookType)),
    hasError:
      hasErrorParam === "include" || hasErrorParam === "exclude"
        ? hasErrorParam
        : "neutral",
    inReadList:
      inReadListParam === "include" || inReadListParam === "exclude"
        ? inReadListParam
        : "neutral",
  };
}
