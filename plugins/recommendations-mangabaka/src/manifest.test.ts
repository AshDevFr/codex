import { describe, expect, it } from "vitest";
import { EXTERNAL_ID_SOURCE_MANGABAKA, manifest, seriesUrl } from "./manifest.js";

describe("manifest", () => {
  it("declares the external ID source metadata-mangabaka writes", () => {
    // The host derives `excludeIds` by matching this string against the
    // `source` field of each library entry's external IDs. `metadata-mangabaka`
    // writes "api:mangabaka" (see its mappers.ts). If these ever diverge,
    // exclusion silently stops working rather than failing loudly, so pin it.
    expect(EXTERNAL_ID_SOURCE_MANGABAKA).toBe("api:mangabaka");
    expect(manifest.capabilities.externalIdSource).toBe("api:mangabaka");
  });

  it("advertises itself as a user recommendation provider", () => {
    expect(manifest.capabilities.userRecommendationProvider).toBe(true);
  });

  it("requires no credentials and no OAuth", () => {
    // Enabling with zero configuration is this plugin's reason to exist.
    // A stray credential field would gate it behind a setup step.
    expect(manifest).not.toHaveProperty("requiredCredentials");
    expect(manifest).not.toHaveProperty("oauth");
  });

  it("warns that coverage is manga-only", () => {
    // Users with comics or ebook libraries would otherwise enable this and get
    // an empty list with no explanation.
    expect(manifest.userDescription.toLowerCase()).toContain("manhwa");
    expect(manifest.userDescription.toLowerCase()).toContain("manhua");
  });

  it("uses the plugin naming convention", () => {
    expect(manifest.name).toBe("recommendations-mangabaka");
    expect(manifest.protocolVersion).toBe("1.1");
  });
});

describe("seriesUrl", () => {
  it("builds a public series URL", () => {
    expect(seriesUrl(57372)).toBe("https://mangabaka.org/57372");
  });
});
