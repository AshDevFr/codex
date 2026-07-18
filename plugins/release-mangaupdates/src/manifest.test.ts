import { describe, expect, it } from "vitest";
import { manifest } from "./manifest.js";

describe("webLinks capability", () => {
  it("declares a series-scoped search template", () => {
    expect(manifest.capabilities.webLinks.searchUrlTemplate).toBe(
      "https://www.mangaupdates.com/series?search={title}",
    );
  });

  it("declares no series links (stored IDs mix numeric and base36 slug forms)", () => {
    expect(
      (manifest.capabilities.webLinks as { seriesLinks?: unknown }).seriesLinks,
    ).toBeUndefined();
  });
});
