import { describe, expect, it } from "vitest";
import type { BookCondition, SeriesCondition } from "@/types/filters";
import {
  appendChildAtPath,
  asGroup,
  emptyRoot,
  ensureRoot,
  isGroup,
  leafFieldKey,
  leafOperator,
  newLeaf,
  normalizeForEmit,
  removeAtPath,
  replaceAtPath,
  updateLeafOperator,
} from "./conditionUtils";
import { findField } from "./fieldCatalog";

describe("conditionUtils — group inspection", () => {
  it("recognizes group vs leaf", () => {
    expect(isGroup({ allOf: [] } as SeriesCondition)).toBe(true);
    expect(isGroup({ anyOf: [] } as SeriesCondition)).toBe(true);
    expect(
      isGroup({ name: { operator: "is", value: "x" } } as SeriesCondition),
    ).toBe(false);
  });

  it("extracts mode and children from a group", () => {
    const c: SeriesCondition = {
      anyOf: [
        { name: { operator: "is", value: "a" } },
        { name: { operator: "is", value: "b" } },
      ],
    };
    const g = asGroup(c);
    expect(g?.mode).toBe("anyOf");
    expect(g?.children).toHaveLength(2);
  });

  it("returns the leaf field key and operator", () => {
    const c: SeriesCondition = { name: { operator: "contains", value: "foo" } };
    expect(leafFieldKey(c)).toBe("name");
    expect(leafOperator(c)).toBe("contains");
  });
});

describe("conditionUtils — newLeaf", () => {
  it("creates a default leaf for each operator type", () => {
    const nameField = findField("series", "name");
    const yearField = findField("series", "year");
    const completionField = findField("series", "completion");
    const dateField = findField("books", "dateAdded");
    const libraryField = findField("books", "libraryId");
    expect(nameField).toBeTruthy();
    expect(yearField).toBeTruthy();
    expect(completionField).toBeTruthy();
    expect(dateField).toBeTruthy();
    expect(libraryField).toBeTruthy();

    const nameLeaf = newLeaf(nameField!);
    expect(leafOperator(nameLeaf)).toBe("contains");

    const yearLeaf = newLeaf(yearField!);
    expect(leafOperator(yearLeaf)).toBe("eq");

    const completionLeaf = newLeaf(completionField!);
    expect(leafOperator(completionLeaf)).toBe("isTrue");

    const dateLeaf = newLeaf(dateField!);
    expect(leafOperator(dateLeaf)).toBe("onOrAfter");

    const libLeaf = newLeaf(libraryField!);
    expect(leafOperator(libLeaf)).toBe("is");
  });

  it("uses `is` as default for enum-typed Field operators", () => {
    const formatField = findField("books", "format");
    expect(formatField).toBeTruthy();
    const leaf = newLeaf(formatField!) as BookCondition;
    expect(leafOperator(leaf)).toBe("is");
    expect((leaf as { format: { value: string } }).format.value).toBe("cbz");
  });
});

describe("conditionUtils — updateLeafOperator", () => {
  it("preserves the value when switching between value-bearing operators", () => {
    const titleField = findField("books", "title")!;
    const leaf: BookCondition = {
      title: { operator: "contains", value: "punch" },
    };
    const next = updateLeafOperator(leaf, titleField, "is");
    expect(next).toEqual({ title: { operator: "is", value: "punch" } });
  });

  it("drops the value when switching to a no-value operator", () => {
    const titleField = findField("books", "title")!;
    const leaf: BookCondition = {
      title: { operator: "contains", value: "punch" },
    };
    const next = updateLeafOperator(leaf, titleField, "isNull");
    expect(next).toEqual({ title: { operator: "isNull" } });
  });

  it("creates {min,max} when switching number to between", () => {
    const pageField = findField("books", "pageCount")!;
    const leaf: BookCondition = { pageCount: { operator: "eq", value: 24 } };
    const next = updateLeafOperator(leaf, pageField, "between");
    expect(next).toEqual({
      pageCount: { operator: "between", min: null, max: null },
    });
  });
});

describe("conditionUtils — tree mutations", () => {
  const root = (): SeriesCondition => ({
    allOf: [
      { name: { operator: "is", value: "a" } },
      {
        anyOf: [
          { tag: { operator: "is", value: "x" } },
          { tag: { operator: "is", value: "y" } },
        ],
      },
    ],
  });

  it("replaces at path", () => {
    const next = replaceAtPath(root(), [0], {
      name: { operator: "is", value: "b" },
    } as SeriesCondition);
    expect(asGroup(next)!.children[0]).toEqual({
      name: { operator: "is", value: "b" },
    });
  });

  it("replaces nested at path", () => {
    const next = replaceAtPath(root(), [1, 0], {
      tag: { operator: "is", value: "z" },
    } as SeriesCondition);
    const nested = asGroup(asGroup(next)!.children[1])!;
    expect(nested.children[0]).toEqual({
      tag: { operator: "is", value: "z" },
    });
  });

  it("removes at path", () => {
    const next = removeAtPath(root(), [0]);
    expect(asGroup(next)!.children).toHaveLength(1);
  });

  it("appends a child to the root", () => {
    const next = appendChildAtPath(root(), [], {
      name: { operator: "is", value: "c" },
    } as SeriesCondition);
    expect(asGroup(next)!.children).toHaveLength(3);
  });

  it("appends a child to a nested group", () => {
    const next = appendChildAtPath(root(), [1], {
      tag: { operator: "is", value: "z" },
    } as SeriesCondition);
    const nested = asGroup(asGroup(next)!.children[1])!;
    expect(nested.children).toHaveLength(3);
  });
});

describe("conditionUtils — normalizeForEmit", () => {
  it("emits undefined for an empty root", () => {
    expect(normalizeForEmit(emptyRoot())).toBeUndefined();
  });

  it("unwraps a single-leaf root", () => {
    const single: SeriesCondition = {
      allOf: [{ name: { operator: "is", value: "a" } }],
    };
    expect(normalizeForEmit(single)).toEqual({
      name: { operator: "is", value: "a" },
    });
  });

  it("passes through multi-child groups", () => {
    const multi: SeriesCondition = {
      allOf: [
        { name: { operator: "is", value: "a" } },
        { name: { operator: "is", value: "b" } },
      ],
    };
    expect(normalizeForEmit(multi)).toEqual(multi);
  });
});

describe("conditionUtils — ensureRoot", () => {
  it("wraps a bare leaf so the builder always sees a group", () => {
    const leaf: SeriesCondition = { name: { operator: "is", value: "a" } };
    const wrapped = ensureRoot(leaf);
    expect(isGroup(wrapped)).toBe(true);
    expect(asGroup(wrapped)!.children).toHaveLength(1);
  });

  it("returns the same group when given one", () => {
    const g: SeriesCondition = { anyOf: [] };
    expect(ensureRoot(g)).toEqual(g);
  });

  it("returns an empty root when given undefined", () => {
    expect(ensureRoot(undefined)).toEqual({ allOf: [] });
  });
});
