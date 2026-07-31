import { describe, expect, it } from "vitest";
import type { BookCondition, SeriesCondition } from "@/types/filters";
import {
  appendChildAtPath,
  applyDragMove,
  asGroup,
  dragId,
  emptyRoot,
  ensureRoot,
  isGroup,
  isLeafComplete,
  leafFieldKey,
  leafOperator,
  moveAtPath,
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
      isGroup({ title: { operator: "is", value: "x" } } as SeriesCondition),
    ).toBe(false);
  });

  it("extracts mode and children from a group", () => {
    const c: SeriesCondition = {
      anyOf: [
        { title: { operator: "is", value: "a" } },
        { title: { operator: "is", value: "b" } },
      ],
    };
    const g = asGroup(c);
    expect(g?.mode).toBe("anyOf");
    expect(g?.children).toHaveLength(2);
  });

  it("returns the leaf field key and operator", () => {
    const c: SeriesCondition = {
      title: { operator: "contains", value: "foo" },
    };
    expect(leafFieldKey(c)).toBe("title");
    expect(leafOperator(c)).toBe("contains");
  });
});

describe("conditionUtils — newLeaf", () => {
  it("creates a default leaf for each operator type", () => {
    const titleField = findField("series", "title");
    const yearField = findField("series", "year");
    const completionField = findField("series", "completion");
    const dateField = findField("books", "dateAdded");
    const libraryField = findField("books", "libraryId");
    expect(titleField).toBeTruthy();
    expect(yearField).toBeTruthy();
    expect(completionField).toBeTruthy();
    expect(dateField).toBeTruthy();
    expect(libraryField).toBeTruthy();

    const titleLeaf = newLeaf(titleField!);
    expect(leafOperator(titleLeaf)).toBe("contains");

    const yearLeaf = newLeaf(yearField!);
    expect(leafOperator(yearLeaf)).toBe("eq");

    const completionLeaf = newLeaf(completionField!);
    expect(leafOperator(completionLeaf)).toBe("isTrue");

    const dateLeaf = newLeaf(dateField!);
    expect(leafOperator(dateLeaf)).toBe("onOrAfter");

    const libLeaf = newLeaf(libraryField!);
    expect(leafOperator(libLeaf)).toBe("is");
  });

  it("exposes summary as a shared free-text field", () => {
    const seriesSummary = findField("series", "summary");
    const bookSummary = findField("books", "summary");
    expect(seriesSummary).toBeTruthy();
    expect(bookSummary).toBeTruthy();
    expect(seriesSummary!.operatorType).toBe("field");
    expect(seriesSummary!.hint).toBe("text");

    const leaf = newLeaf(seriesSummary!);
    expect(leafFieldKey(leaf)).toBe("summary");
    expect(leafOperator(leaf)).toBe("contains");
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
      { title: { operator: "is", value: "a" } },
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
      title: { operator: "is", value: "b" },
    } as SeriesCondition);
    expect(asGroup(next)!.children[0]).toEqual({
      title: { operator: "is", value: "b" },
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
      title: { operator: "is", value: "c" },
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

describe("conditionUtils — moveAtPath", () => {
  const root = (): SeriesCondition => ({
    allOf: [
      { title: { operator: "is", value: "a" } },
      {
        anyOf: [
          { tag: { operator: "is", value: "x" } },
          { tag: { operator: "is", value: "y" } },
          { tag: { operator: "is", value: "z" } },
        ],
      },
      { genre: { operator: "is", value: "fantasy" } },
    ],
  });

  it("moves a child down within the root group", () => {
    const children = asGroup(moveAtPath(root(), [], 0, 2))!.children;
    expect(children[0]).toHaveProperty("anyOf");
    expect(children[1]).toEqual({
      genre: { operator: "is", value: "fantasy" },
    });
    expect(children[2]).toEqual({ title: { operator: "is", value: "a" } });
  });

  it("moves a child up within the root group", () => {
    const children = asGroup(moveAtPath(root(), [], 2, 0))!.children;
    expect(children[0]).toEqual({
      genre: { operator: "is", value: "fantasy" },
    });
    expect(children[1]).toEqual({ title: { operator: "is", value: "a" } });
    expect(children[2]).toHaveProperty("anyOf");
  });

  it("moves a child within a nested group", () => {
    const next = moveAtPath(root(), [1], 2, 0);
    const nested = asGroup(asGroup(next)!.children[1])!;
    expect(nested.children.map((c) => leafOperatorValue(c))).toEqual([
      "z",
      "x",
      "y",
    ]);
  });

  it("preserves the combinator mode of the group it reorders", () => {
    const next = moveAtPath(root(), [1], 0, 1);
    expect(asGroup(asGroup(next)!.children[1])!.mode).toBe("anyOf");
  });

  it("is a no-op when the indices are equal", () => {
    const before = root();
    expect(moveAtPath(before, [], 1, 1)).toEqual(before);
  });

  it("is a no-op when an index is out of range", () => {
    const before = root();
    expect(moveAtPath(before, [], 0, 9)).toEqual(before);
    expect(moveAtPath(before, [], -1, 0)).toEqual(before);
    expect(moveAtPath(before, [], 5, 0)).toEqual(before);
  });

  it("is a no-op when the path does not point at a group", () => {
    const before = root();
    // Path [0] is a leaf, not a group, so there is nothing to reorder inside it.
    expect(moveAtPath(before, [0], 0, 1)).toEqual(before);
  });

  it("does not mutate the input tree", () => {
    const before = root();
    const snapshot = structuredClone(before);
    moveAtPath(before, [1], 0, 2);
    expect(before).toEqual(snapshot);
  });
});

describe("conditionUtils — applyDragMove", () => {
  const root = (): SeriesCondition => ({
    allOf: [
      { title: { operator: "is", value: "a" } },
      {
        anyOf: [
          { tag: { operator: "is", value: "x" } },
          { tag: { operator: "is", value: "y" } },
        ],
      },
    ],
  });

  it("builds ids that encode the parent path", () => {
    expect(dragId([], 0)).toBe("#0");
    expect(dragId([1], 2)).toBe("1#2");
    expect(dragId([1, 0], 3)).toBe("1.0#3");
  });

  it("reorders siblings at the root", () => {
    const next = applyDragMove(root(), dragId([], 0), dragId([], 1));
    expect(asGroup(next as SeriesCondition)!.children[0]).toHaveProperty(
      "anyOf",
    );
  });

  it("reorders siblings inside a nested group", () => {
    const next = applyDragMove(root(), dragId([1], 1), dragId([1], 0));
    const nested = asGroup(asGroup(next as SeriesCondition)!.children[1])!;
    expect(nested.children[0]).toEqual({ tag: { operator: "is", value: "y" } });
  });

  it("rejects a drop that crossed into another group", () => {
    // Dragging a root row into the nested anyOf would change what matches, so
    // the drop is ignored rather than applied.
    expect(applyDragMove(root(), dragId([], 0), dragId([1], 0))).toBeNull();
    expect(applyDragMove(root(), dragId([1], 0), dragId([], 0))).toBeNull();
  });

  it("rejects a drop onto itself", () => {
    expect(applyDragMove(root(), dragId([], 1), dragId([], 1))).toBeNull();
  });

  it("rejects malformed ids", () => {
    expect(applyDragMove(root(), "nonsense", dragId([], 1))).toBeNull();
    expect(applyDragMove(root(), dragId([], 0), "#notanumber")).toBeNull();
  });

  it("rejects an index that is out of range", () => {
    expect(applyDragMove(root(), dragId([], 0), dragId([], 9))).toBeNull();
  });
});

/** Pull the `value` off a single-key leaf, for terse ordering assertions. */
function leafOperatorValue(c: SeriesCondition): unknown {
  const key = Object.keys(c)[0]!;
  return (c as Record<string, { value?: unknown }>)[key]?.value;
}

describe("conditionUtils — normalizeForEmit", () => {
  it("emits undefined for an empty root", () => {
    expect(normalizeForEmit(emptyRoot())).toBeUndefined();
  });

  it("unwraps a single-leaf root", () => {
    const single: SeriesCondition = {
      allOf: [{ title: { operator: "is", value: "a" } }],
    };
    expect(normalizeForEmit(single)).toEqual({
      title: { operator: "is", value: "a" },
    });
  });

  it("passes through multi-child groups", () => {
    const multi: SeriesCondition = {
      allOf: [
        { title: { operator: "is", value: "a" } },
        { title: { operator: "is", value: "b" } },
      ],
    };
    expect(normalizeForEmit(multi)).toEqual(multi);
  });
});

describe("conditionUtils — isLeafComplete", () => {
  it("treats no-value operators as complete", () => {
    expect(
      isLeafComplete({ title: { operator: "isNull" } } as BookCondition),
    ).toBe(true);
    expect(
      isLeafComplete({ completion: { operator: "isTrue" } } as SeriesCondition),
    ).toBe(true);
  });

  it("rejects empty UUIDs and blank strings", () => {
    expect(
      isLeafComplete({
        libraryId: { operator: "is", value: "" },
      } as SeriesCondition),
    ).toBe(false);
    expect(
      isLeafComplete({
        title: { operator: "contains", value: "   " },
      } as BookCondition),
    ).toBe(false);
  });

  it("accepts populated string and number values", () => {
    expect(
      isLeafComplete({
        title: { operator: "contains", value: "punch" },
      } as BookCondition),
    ).toBe(true);
    expect(
      isLeafComplete({
        year: { operator: "eq", value: 1999 },
      } as SeriesCondition),
    ).toBe(true);
  });

  it("requires at least one bound on between ranges", () => {
    expect(
      isLeafComplete({
        pageCount: { operator: "between", min: null, max: null },
      } as BookCondition),
    ).toBe(false);
    expect(
      isLeafComplete({
        pageCount: { operator: "between", min: 100, max: null },
      } as BookCondition),
    ).toBe(true);
  });
});

describe("conditionUtils — normalizeForEmit prunes leaves not on the active target", () => {
  it("drops a series-only leaf when emitting for books", () => {
    const c: SeriesCondition = {
      allOf: [
        {
          libraryId: {
            operator: "is",
            value: "83197543-5435-4a35-983a-abae4ff77884",
          },
        },
        { author: { operator: "contains", value: "Toriyama" } },
      ],
    };
    // `author` is series-only; BookCondition has no such variant. Without
    // target pruning the backend would 422 on the books tab.
    expect(normalizeForEmit(c, "books")).toEqual({
      libraryId: {
        operator: "is",
        value: "83197543-5435-4a35-983a-abae4ff77884",
      },
    });
  });

  it("drops a books-only leaf when emitting for series", () => {
    const c: BookCondition = {
      allOf: [
        { format: { operator: "is", value: "cbz" } },
        { genre: { operator: "is", value: "Action" } },
      ],
    };
    expect(normalizeForEmit(c, "series")).toEqual({
      genre: { operator: "is", value: "Action" },
    });
  });

  it("emits undefined when no leaves match the target", () => {
    const c: SeriesCondition = {
      allOf: [{ author: { operator: "contains", value: "x" } }],
    };
    expect(normalizeForEmit(c, "books")).toBeUndefined();
  });
});

describe("conditionUtils — normalizeForEmit prunes incomplete leaves", () => {
  it("drops a single incomplete leaf, emitting undefined", () => {
    const c: SeriesCondition = {
      allOf: [{ libraryId: { operator: "is", value: "" } }],
    };
    expect(normalizeForEmit(c)).toBeUndefined();
  });

  it("keeps complete siblings when one leaf is incomplete", () => {
    const c: SeriesCondition = {
      allOf: [
        { title: { operator: "contains", value: "foo" } },
        { libraryId: { operator: "is", value: "" } },
      ],
    };
    expect(normalizeForEmit(c)).toEqual({
      title: { operator: "contains", value: "foo" },
    });
  });

  it("collapses a nested group when all its children are incomplete", () => {
    const c: SeriesCondition = {
      allOf: [
        { title: { operator: "contains", value: "foo" } },
        {
          anyOf: [
            { libraryId: { operator: "is", value: "" } },
            { tag: { operator: "is", value: "" } },
          ],
        },
      ],
    };
    expect(normalizeForEmit(c)).toEqual({
      title: { operator: "contains", value: "foo" },
    });
  });
});

describe("conditionUtils — ensureRoot", () => {
  it("wraps a bare leaf so the builder always sees a group", () => {
    const leaf: SeriesCondition = { title: { operator: "is", value: "a" } };
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
