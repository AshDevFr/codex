import { describe, expect, it } from "vitest";
import { hasWebLinks, type PluginManifest } from "./manifest.js";

const baseManifest: PluginManifest = {
  name: "test-plugin",
  displayName: "Test Plugin",
  version: "1.0.0",
  description: "A test plugin",
  author: "Codex",
  protocolVersion: "1.1",
  capabilities: {},
};

describe("hasWebLinks", () => {
  it("returns false when the capability is absent", () => {
    expect(hasWebLinks(baseManifest)).toBe(false);
  });

  it("returns true and narrows when the capability is declared", () => {
    const manifest: PluginManifest = {
      ...baseManifest,
      capabilities: {
        webLinks: {
          searchUrlTemplate: "{config.baseUrl}/search?q={title}",
          seriesLinks: [
            {
              source: "myanimelist",
              urlTemplate: "{config.baseUrl}/series/lookup?source=mal&id={externalId}",
            },
          ],
        },
      },
    };

    expect(hasWebLinks(manifest)).toBe(true);
    if (hasWebLinks(manifest)) {
      expect(manifest.capabilities.webLinks.searchUrlTemplate).toContain("{title}");
      expect(manifest.capabilities.webLinks.seriesLinks?.[0]?.source).toBe("myanimelist");
    }
  });

  it("accepts a webLinks capability without seriesLinks (search-only plugins)", () => {
    const manifest: PluginManifest = {
      ...baseManifest,
      capabilities: {
        webLinks: { searchUrlTemplate: "https://nyaa.si/?q={title}" },
      },
    };

    expect(hasWebLinks(manifest)).toBe(true);
  });
});
