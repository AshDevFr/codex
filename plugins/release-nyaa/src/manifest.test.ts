import { describe, expect, it } from "vitest";
import { manifest } from "./manifest.js";

describe("webLinks capability", () => {
  it("builds the search URL from the configurable base URL", () => {
    expect(manifest.capabilities.webLinks.searchUrlTemplate).toBe("{config.baseUrl}/?q={title}");
  });

  it("relies on a configSchema default for baseUrl so the link resolves without stored config", () => {
    const baseUrlField = manifest.configSchema.fields.find((field) => field.key === "baseUrl");
    expect(baseUrlField?.default).toBe("https://nyaa.si");
  });

  it("declares no series links (Nyaa has no per-series pages)", () => {
    expect(
      (manifest.capabilities.webLinks as { seriesLinks?: unknown }).seriesLinks,
    ).toBeUndefined();
  });
});
