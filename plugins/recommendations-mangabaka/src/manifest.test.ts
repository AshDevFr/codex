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

describe("userConfigSchema", () => {
  const fields = manifest.userConfigSchema.fields;
  const keys = fields.map((f) => f.key);

  it("exposes every filter and tuning setting", () => {
    expect(keys).toEqual([
      "contentRating",
      "includedTypes",
      "excludedTypes",
      "excludedGenres",
      "excludedTags",
      "minimumRating",
      "contentWeight",
      "collaborativeSeeds",
      "excludeSameAuthor",
    ]);
  });

  it("uses only field types the settings UI can render", () => {
    // Anything other than boolean, number, or json falls back to a plain text
    // input, so an array type would render as an unexplained text box.
    for (const field of fields) {
      expect(["string", "number", "boolean"]).toContain(field.type);
    }
  });

  it("describes tag exclusion in terms of names, not IDs", () => {
    const tags = fields.find((f) => f.key === "excludedTags");

    expect(tags?.description).toMatch(/name/i);
    expect(tags?.description).not.toMatch(/\bID\b/);
  });

  it("defaults every field, so an untouched plugin is fully configured", () => {
    for (const field of fields) {
      expect(field.default).toBeDefined();
      expect(field.required ?? false).toBe(false);
    }
  });

  it("keeps same-author results by default", () => {
    expect(fields.find((f) => f.key === "excludeSameAuthor")?.default).toBe(false);
  });

  it("documents that zero seeds disables reader overlap", () => {
    expect(fields.find((f) => f.key === "collaborativeSeeds")?.description).toMatch(/0/);
  });
});

describe("configSchema", () => {
  it("exposes the admin-level client settings", () => {
    expect(manifest.configSchema.fields.map((f) => f.key)).toEqual(["timeout", "base_url"]);
  });

  it("offers a base URL override as an escape hatch for the beta endpoints", () => {
    const baseUrl = manifest.configSchema.fields.find((f) => f.key === "base_url");

    expect(baseUrl?.default).toBe("https://api.mangabaka.org");
  });
});
