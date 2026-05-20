import type {
  BookCondition,
  DateOperator,
  FieldOperator,
  NumberOperator,
  SeriesCondition,
  UuidOperator,
} from "@/types/filters";
import type { FieldDef, OperatorType } from "./fieldCatalog";

/**
 * The condition tree the builder works with. Mirrors the API grammar
 * exactly so we don't have to convert back and forth.
 */
export type Condition = SeriesCondition | BookCondition;

/** A combinator node: `{ allOf: ... }` or `{ anyOf: ... }`. */
export interface GroupNode {
  mode: "allOf" | "anyOf";
  children: Condition[];
}

export function isGroup(
  c: Condition,
): c is Extract<Condition, { allOf: unknown } | { anyOf: unknown }> {
  return typeof c === "object" && c !== null && ("allOf" in c || "anyOf" in c);
}

export function asGroup(c: Condition): GroupNode | null {
  if ("allOf" in c && Array.isArray((c as { allOf: unknown[] }).allOf)) {
    return {
      mode: "allOf",
      children: (c as { allOf: Condition[] }).allOf,
    };
  }
  if ("anyOf" in c && Array.isArray((c as { anyOf: unknown[] }).anyOf)) {
    return {
      mode: "anyOf",
      children: (c as { anyOf: Condition[] }).anyOf,
    };
  }
  return null;
}

export function makeGroup(group: GroupNode): Condition {
  return group.mode === "allOf"
    ? ({ allOf: group.children } as Condition)
    : ({ anyOf: group.children } as Condition);
}

/**
 * Get the field key from a leaf condition, or null if the condition is a
 * group / malformed.
 */
export function leafFieldKey(c: Condition): string | null {
  if (isGroup(c)) return null;
  const keys = Object.keys(c);
  return keys.length === 1 ? keys[0] : null;
}

export function leafOperator(c: Condition): string | null {
  const key = leafFieldKey(c);
  if (!key) return null;
  const op = (c as Record<string, { operator?: string }>)[key];
  return op?.operator ?? null;
}

/**
 * Operator → display label, per operator family. Used to populate the
 * Operator dropdown in the LeafEditor.
 */
export const OPERATOR_LABELS: Record<OperatorType, Record<string, string>> = {
  field: {
    is: "is",
    isNot: "is not",
    contains: "contains",
    doesNotContain: "does not contain",
    beginsWith: "begins with",
    endsWith: "ends with",
    isNull: "is empty",
    isNotNull: "is not empty",
  },
  uuid: {
    is: "is",
    isNot: "is not",
  },
  bool: {
    isTrue: "is true",
    isFalse: "is false",
  },
  number: {
    eq: "equals",
    ne: "is not",
    gt: "greater than",
    gte: "≥",
    lt: "less than",
    lte: "≤",
    between: "between",
    isNull: "is empty",
    isNotNull: "is not empty",
  },
  date: {
    after: "after",
    before: "before",
    onOrAfter: "on or after",
    onOrBefore: "on or before",
    between: "between",
    isNull: "is empty",
    isNotNull: "is not empty",
  },
};

/**
 * Operators legal for a given field. For "field"-type fields that carry
 * a closed enum (e.g. `format`, `bookType`), strip operators that take a
 * free-form value — only equality + nullability survive.
 */
export function operatorsForField(field: FieldDef): string[] {
  const all = Object.keys(OPERATOR_LABELS[field.operatorType]);
  if (field.operatorType === "field" && field.enumValues) {
    return all.filter(
      (op) =>
        op === "is" || op === "isNot" || op === "isNull" || op === "isNotNull",
    );
  }
  return all;
}

/**
 * Default operator for a fresh field. Picks the most natural starting point
 * given the operator family.
 */
export function defaultOperator(field: FieldDef): string {
  switch (field.operatorType) {
    case "field":
      return field.enumValues ? "is" : "contains";
    case "uuid":
      return "is";
    case "bool":
      return "isTrue";
    case "number":
      return "eq";
    case "date":
      return "onOrAfter";
  }
}

/**
 * Build a fresh leaf condition with a default value. The shape matches the
 * backend DTOs so the builder never has to translate.
 */
export function newLeaf(field: FieldDef): Condition {
  const op = defaultOperator(field);
  let value: unknown;
  switch (field.operatorType) {
    case "field":
      value = field.enumValues
        ? { operator: op, value: field.enumValues[0]?.value ?? "" }
        : { operator: op, value: "" };
      break;
    case "uuid":
      value = { operator: op, value: "" };
      break;
    case "bool":
      value = { operator: op };
      break;
    case "number":
      value = { operator: op, value: 0 };
      break;
    case "date":
      value = { operator: op, value: new Date().toISOString() };
      break;
  }
  return { [field.key]: value } as Condition;
}

/**
 * Replace the operator/value on a leaf. Operators that take no value
 * (`isNull`, `isNotNull`, `isTrue`, `isFalse`) emit a `{ operator }` shape;
 * `between` emits `{ operator, min?, max? }` (number) or `{ operator,
 * start?, end? }` (date).
 */
export function updateLeafOperator(
  c: Condition,
  field: FieldDef,
  operator: string,
): Condition {
  const key = field.key;
  switch (field.operatorType) {
    case "field": {
      if (operator === "isNull" || operator === "isNotNull") {
        return { [key]: { operator } as FieldOperator } as Condition;
      }
      const prev = (c as Record<string, FieldOperator>)[key];
      const prevValue = "value" in prev ? prev.value : "";
      return {
        [key]: { operator, value: prevValue } as FieldOperator,
      } as Condition;
    }
    case "uuid": {
      const prev = (c as Record<string, UuidOperator>)[key];
      const prevValue = "value" in prev ? prev.value : "";
      return {
        [key]: { operator, value: prevValue } as UuidOperator,
      } as Condition;
    }
    case "bool":
      return { [key]: { operator } as { operator: string } } as Condition;
    case "number": {
      const prev = (c as Record<string, NumberOperator>)[key];
      if (operator === "isNull" || operator === "isNotNull") {
        return { [key]: { operator } as NumberOperator } as Condition;
      }
      if (operator === "between") {
        const prevBetween = prev as Extract<
          NumberOperator,
          { operator: "between" }
        >;
        return {
          [key]: {
            operator: "between",
            min: prevBetween?.min ?? null,
            max: prevBetween?.max ?? null,
          } as NumberOperator,
        } as Condition;
      }
      const prevValue = "value" in prev ? (prev as { value: number }).value : 0;
      return {
        [key]: { operator, value: prevValue } as NumberOperator,
      } as Condition;
    }
    case "date": {
      const prev = (c as Record<string, DateOperator>)[key];
      if (operator === "isNull" || operator === "isNotNull") {
        return { [key]: { operator } as DateOperator } as Condition;
      }
      if (operator === "between") {
        const prevBetween = prev as Extract<
          DateOperator,
          { operator: "between" }
        >;
        return {
          [key]: {
            operator: "between",
            start: prevBetween?.start ?? null,
            end: prevBetween?.end ?? null,
          } as DateOperator,
        } as Condition;
      }
      const prevValue =
        "value" in prev
          ? (prev as { value: string }).value
          : new Date().toISOString();
      return {
        [key]: { operator, value: prevValue } as DateOperator,
      } as Condition;
    }
  }
}

export function updateLeafValue<T>(
  c: Condition,
  field: FieldDef,
  patch: Record<string, T>,
): Condition {
  const key = field.key;
  const prev = (c as Record<string, Record<string, unknown>>)[key];
  return {
    [key]: { ...prev, ...patch },
  } as Condition;
}

/**
 * Walk the tree and replace the node at `path`. The path is an array of
 * child indices (from the root). An empty path replaces the root.
 */
export function replaceAtPath(
  root: Condition,
  path: number[],
  next: Condition,
): Condition {
  if (path.length === 0) return next;
  const group = asGroup(root);
  if (!group) return root;
  const [head, ...rest] = path;
  const newChildren = group.children.slice();
  newChildren[head] = replaceAtPath(newChildren[head], rest, next);
  return makeGroup({ mode: group.mode, children: newChildren });
}

export function removeAtPath(root: Condition, path: number[]): Condition {
  if (path.length === 0) {
    // Root removal — collapse to an empty allOf group.
    return makeGroup({ mode: "allOf", children: [] });
  }
  const group = asGroup(root);
  if (!group) return root;
  const [head, ...rest] = path;
  if (rest.length === 0) {
    const newChildren = group.children.filter((_, i) => i !== head);
    return makeGroup({ mode: group.mode, children: newChildren });
  }
  const newChildren = group.children.slice();
  newChildren[head] = removeAtPath(newChildren[head], rest);
  return makeGroup({ mode: group.mode, children: newChildren });
}

export function appendChildAtPath(
  root: Condition,
  path: number[],
  child: Condition,
): Condition {
  if (path.length === 0) {
    const group = asGroup(root);
    if (!group) return root;
    return makeGroup({
      mode: group.mode,
      children: [...group.children, child],
    });
  }
  const group = asGroup(root);
  if (!group) return root;
  const [head, ...rest] = path;
  const newChildren = group.children.slice();
  newChildren[head] = appendChildAtPath(newChildren[head], rest, child);
  return makeGroup({ mode: group.mode, children: newChildren });
}

/**
 * Build an empty root condition (an empty `allOf` group). Used when the
 * page mounts with no condition in the URL.
 */
export function emptyRoot(): Condition {
  return makeGroup({ mode: "allOf", children: [] });
}

/**
 * Normalize a root condition for emission to the API:
 *   - An empty group becomes `undefined` (no condition).
 *   - A group with a single leaf child unwraps to the leaf.
 *   - Otherwise the group is passed through.
 *
 * Nested groups stay nested — we don't try to flatten across levels.
 */
export function normalizeForEmit(root: Condition): Condition | undefined {
  const group = asGroup(root);
  if (!group) return root;
  if (group.children.length === 0) return undefined;
  if (group.children.length === 1) return group.children[0];
  return root;
}

/**
 * Wrap a non-group condition (e.g. a bare leaf from a saved preset) in a
 * root `allOf` group so the builder can edit it uniformly.
 */
export function ensureRoot(c: Condition | undefined): Condition {
  if (!c) return emptyRoot();
  return isGroup(c) ? c : makeGroup({ mode: "allOf", children: [c] });
}
