import { describe, expect, it } from "vitest";
import { withContentKeys } from "./contentKeys";

describe("withContentKeys", () => {
  it("keys each item by its identity", () => {
    const keyed = withContentKeys(["a", "b"], (item) => item);

    expect(keyed).toEqual([
      { key: "a", item: "a" },
      { key: "b", item: "b" },
    ]);
  });

  it("suffixes repeated identities with their occurrence number", () => {
    const keyed = withContentKeys(["a", "b", "a", "a"], (item) => item);

    expect(keyed.map(({ key }) => key)).toEqual(["a", "b", "a#2", "a#3"]);
  });

  it("keeps keys stable when an unrelated item is removed", () => {
    const before = withContentKeys(["x", "a", "b"], (item) => item);
    const after = withContentKeys(["a", "b"], (item) => item);

    expect(after.map(({ key }) => key)).toEqual(
      before.slice(1).map(({ key }) => key),
    );
  });

  it("cannot collide with an identity that already looks like a suffix", () => {
    const keyed = withContentKeys(["a#2", "a", "a"], (item) => item);
    const keys = keyed.map(({ key }) => key);

    expect(new Set(keys).size).toBe(keys.length);
  });

  it("returns an empty list for no items", () => {
    expect(withContentKeys([], (item: string) => item)).toEqual([]);
  });
});
