import { describe, expect, it } from "vitest";
import { manifest } from "./manifest.js";

function schemaDefault(key: string): unknown {
  return manifest.configSchema.fields.find((field) => field.key === key)?.default;
}

describe("webLinks capability", () => {
  it("builds the search URL from the configurable base URL, filter, and category", () => {
    expect(manifest.capabilities.webLinks.searchUrlTemplate).toBe(
      "{config.baseUrl}/?f={config.searchFilter}&c={config.searchCategory}&q={title}",
    );
  });

  it("declares a configSchema default for every referenced config field, so the link resolves without stored config", () => {
    const referenced = [
      ...manifest.capabilities.webLinks.searchUrlTemplate.matchAll(/\{config\.([^}]+)\}/g),
    ].map((match) => match[1]);
    expect(referenced).toEqual(["baseUrl", "searchFilter", "searchCategory"]);
    for (const key of referenced) {
      expect(schemaDefault(key), `configSchema default for ${key}`).toBeDefined();
    }
  });

  it("defaults to an unrestricted nyaa.si search", () => {
    expect(schemaDefault("baseUrl")).toBe("https://nyaa.si");
    expect(schemaDefault("searchCategory")).toBe("0_0");
    expect(schemaDefault("searchFilter")).toBe("0");
  });

  it("declares no series links (Nyaa has no per-series pages)", () => {
    expect(
      (manifest.capabilities.webLinks as { seriesLinks?: unknown }).seriesLinks,
    ).toBeUndefined();
  });
});
