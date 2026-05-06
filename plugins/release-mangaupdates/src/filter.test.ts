import { describe, expect, it } from "vitest";
import { parseCommaList, passesFilters, resolveFilters } from "./filter.js";
import type { ParsedRssItem } from "./parser.js";
import { UNKNOWN_LANGUAGE } from "./parser.js";

function item(overrides: Partial<ParsedRssItem> = {}): ParsedRssItem {
  return {
    externalReleaseId: "abc",
    title: "c.143 by G (en)",
    chapter: 143,
    volume: null,
    group: "G",
    language: "en",
    link: "https://example.com",
    observedAt: new Date().toISOString(),
    ...overrides,
  };
}

describe("resolveFilters", () => {
  it("normalizes languages: trim, lowercase, dedup", () => {
    const f = resolveFilters({
      languages: ["EN", " es ", "en", ""],
      blockedGroups: [],
    });
    expect(f.languages).toEqual(["en", "es"]);
  });

  it("normalizes blocked groups (case-insensitive set)", () => {
    const f = resolveFilters({
      languages: ["en"],
      blockedGroups: ["LowQuality", "  MTL Group  "],
    });
    expect(f.blockedGroups.has("lowquality")).toBe(true);
    expect(f.blockedGroups.has("mtl group")).toBe(true);
  });

  it("defaults includeUnknownLanguage to false", () => {
    const f = resolveFilters({ languages: ["en"], blockedGroups: [] });
    expect(f.includeUnknownLanguage).toBe(false);
  });
});

describe("parseCommaList", () => {
  it("splits, trims, drops empties", () => {
    expect(parseCommaList(" a , b ,  , c")).toEqual(["a", "b", "c"]);
  });

  it("returns [] for non-string input", () => {
    expect(parseCommaList(undefined)).toEqual([]);
    expect(parseCommaList(null)).toEqual([]);
    expect(parseCommaList(42)).toEqual([]);
  });
});

describe("passesFilters", () => {
  it("passes English item when languages = ['en']", () => {
    const f = resolveFilters({ languages: ["en"], blockedGroups: [] });
    expect(passesFilters(item({ language: "en" }), f)).toBe(true);
  });

  it("rejects Spanish item when languages = ['en']", () => {
    const f = resolveFilters({ languages: ["en"], blockedGroups: [] });
    expect(passesFilters(item({ language: "es" }), f)).toBe(false);
  });

  it("passes Spanish when languages = ['en', 'es']", () => {
    const f = resolveFilters({ languages: ["en", "es"], blockedGroups: [] });
    expect(passesFilters(item({ language: "es" }), f)).toBe(true);
  });

  it("rejects unknown-language item by default", () => {
    const f = resolveFilters({ languages: ["en"], blockedGroups: [] });
    expect(passesFilters(item({ language: UNKNOWN_LANGUAGE }), f)).toBe(false);
  });

  it("admits unknown-language item when includeUnknownLanguage = true", () => {
    const f = resolveFilters({
      languages: ["en"],
      blockedGroups: [],
      includeUnknownLanguage: true,
    });
    expect(passesFilters(item({ language: UNKNOWN_LANGUAGE }), f)).toBe(true);
  });

  it("passes everything (including unknown) when languages list is empty", () => {
    const f = resolveFilters({ languages: [], blockedGroups: [] });
    expect(passesFilters(item({ language: "en" }), f)).toBe(true);
    expect(passesFilters(item({ language: "es" }), f)).toBe(true);
    // Unknown language is *still* gated by includeUnknownLanguage
    // (defaults to false); an empty `languages` list means "no language
    // restriction on known codes," not "include unknown."
    expect(passesFilters(item({ language: UNKNOWN_LANGUAGE }), f)).toBe(false);
  });

  it("rejects items from a blocked group", () => {
    const f = resolveFilters({
      languages: ["en"],
      blockedGroups: ["MTL Group"],
    });
    expect(passesFilters(item({ group: "MTL Group" }), f)).toBe(false);
  });

  it("group blocklist is case-insensitive", () => {
    const f = resolveFilters({ languages: ["en"], blockedGroups: ["mtl group"] });
    expect(passesFilters(item({ group: "MTL Group" }), f)).toBe(false);
    expect(passesFilters(item({ group: "MTL GROUP" }), f)).toBe(false);
    expect(passesFilters(item({ group: "Other Group" }), f)).toBe(true);
  });

  it("admits items with no group regardless of blocklist", () => {
    const f = resolveFilters({ languages: ["en"], blockedGroups: ["MTL"] });
    expect(passesFilters(item({ group: null }), f)).toBe(true);
  });
});
