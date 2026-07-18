import { describe, expect, it } from "vitest";
import { CODEX_TO_TSUNDOKU_PROVIDER, manifest, TSUNDOKU_SERIES_LOOKUP_LINKS } from "./manifest.js";

describe("webLinks capability", () => {
  it("declares a search template with the title runtime placeholder", () => {
    expect(manifest.capabilities.webLinks.searchUrlTemplate).toBe(
      "{config.baseUrl}/search?q={title}",
    );
  });

  it("derives one series link per provider-map entry, in map order", () => {
    const codexSources = Object.keys(CODEX_TO_TSUNDOKU_PROVIDER);
    expect(TSUNDOKU_SERIES_LOOKUP_LINKS.map((link) => link.source)).toEqual(codexSources);
    expect(manifest.capabilities.webLinks.seriesLinks).toBe(TSUNDOKU_SERIES_LOOKUP_LINKS);
  });

  it("bakes Tsundoku's provider notation into each template", () => {
    for (const link of TSUNDOKU_SERIES_LOOKUP_LINKS) {
      const tsundokuProvider = CODEX_TO_TSUNDOKU_PROVIDER[link.source];
      expect(link.urlTemplate).toBe(
        `{config.baseUrl}/series/lookup?source=${tsundokuProvider}&id={externalId}`,
      );
    }
  });

  it("covers the same sources as requiresExternalIds, so any matched series can deep-link", () => {
    expect(TSUNDOKU_SERIES_LOOKUP_LINKS.map((link) => link.source)).toEqual([
      ...manifest.capabilities.releaseSource.requiresExternalIds,
    ]);
  });
});
