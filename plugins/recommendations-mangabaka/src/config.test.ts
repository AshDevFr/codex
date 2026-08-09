import { describe, expect, it } from "vitest";
import { DEFAULT_COLLABORATIVE_SEEDS } from "./collaborative.js";
import { parseAdminConfig, parseUserConfig } from "./config.js";
import { DEFAULT_CONTENT_WEIGHT } from "./scoring.js";

describe("parseUserConfig", () => {
  it("returns usable defaults for an absent config", () => {
    const settings = parseUserConfig(undefined);

    expect(settings.contentRating).toEqual([]);
    expect(settings.excludedTagNames).toEqual([]);
    expect(settings.contentWeight).toBe(DEFAULT_CONTENT_WEIGHT);
    expect(settings.collaborativeSeeds).toBe(DEFAULT_COLLABORATIVE_SEEDS);
    expect(settings.excludeSameAuthor).toBe(false);
  });

  describe("comma-separated lists", () => {
    it("splits and trims values", () => {
      const settings = parseUserConfig({ excludedGenres: " hentai , ecchi " });

      expect(settings.excludedGenres).toEqual(["hentai", "ecchi"]);
    });

    it("drops empty segments from sloppy input", () => {
      const settings = parseUserConfig({ excludedGenres: "hentai,,ecchi," });

      expect(settings.excludedGenres).toEqual(["hentai", "ecchi"]);
    });

    it("treats an empty string as no filter", () => {
      expect(parseUserConfig({ excludedGenres: "" }).excludedGenres).toEqual([]);
      expect(parseUserConfig({ excludedGenres: "   " }).excludedGenres).toEqual([]);
    });

    it("ignores a non-string value", () => {
      expect(parseUserConfig({ excludedGenres: 42 }).excludedGenres).toEqual([]);
    });

    it("preserves tag names verbatim for later resolution", () => {
      // These are resolved against the catalogue, which is case-insensitive but
      // needs the words intact.
      const settings = parseUserConfig({ excludedTags: "Death Game, Gore" });

      expect(settings.excludedTagNames).toEqual(["Death Game", "Gore"]);
    });
  });

  describe("content rating", () => {
    it("accepts the documented ratings, lowercased", () => {
      const settings = parseUserConfig({ contentRating: "Safe, SUGGESTIVE" });

      expect(settings.contentRating).toEqual(["safe", "suggestive"]);
    });

    it("drops values outside the upstream enum", () => {
      // Upstream rejects the whole request on an unknown value, so a typo here
      // would cost the user every recommendation.
      const settings = parseUserConfig({ contentRating: "safe, wholesome" });

      expect(settings.contentRating).toEqual(["safe"]);
    });
  });

  describe("series types", () => {
    it("accepts the documented types", () => {
      const settings = parseUserConfig({ includedTypes: "manga,manhwa", excludedTypes: "novel" });

      expect(settings.includedTypes).toEqual(["manga", "manhwa"]);
      expect(settings.excludedTypes).toEqual(["novel"]);
    });

    it("drops values outside the upstream enum", () => {
      expect(parseUserConfig({ includedTypes: "manga,comic" }).includedTypes).toEqual(["manga"]);
    });
  });

  describe("minimum rating", () => {
    it("accepts a value in range", () => {
      expect(parseUserConfig({ minimumRating: 70 }).minimumRating).toBe(70);
    });

    it("treats zero as no filter", () => {
      expect(parseUserConfig({ minimumRating: 0 }).minimumRating).toBeUndefined();
    });

    it("clamps out-of-range values", () => {
      expect(parseUserConfig({ minimumRating: 500 }).minimumRating).toBe(100);
      expect(parseUserConfig({ minimumRating: -5 }).minimumRating).toBeUndefined();
    });

    it("ignores a non-numeric value", () => {
      expect(parseUserConfig({ minimumRating: "high" }).minimumRating).toBeUndefined();
    });
  });

  describe("blend weight", () => {
    it("accepts a value in range", () => {
      expect(parseUserConfig({ contentWeight: 0.25 }).contentWeight).toBe(0.25);
    });

    it("accepts both extremes", () => {
      expect(parseUserConfig({ contentWeight: 0 }).contentWeight).toBe(0);
      expect(parseUserConfig({ contentWeight: 1 }).contentWeight).toBe(1);
    });

    it("clamps out-of-range values instead of rejecting them", () => {
      expect(parseUserConfig({ contentWeight: 5 }).contentWeight).toBe(1);
      expect(parseUserConfig({ contentWeight: -1 }).contentWeight).toBe(0);
    });

    it("falls back to the default for a non-numeric value", () => {
      expect(parseUserConfig({ contentWeight: "half" }).contentWeight).toBe(DEFAULT_CONTENT_WEIGHT);
    });
  });

  describe("collaborative seed count", () => {
    it("accepts a value in range", () => {
      expect(parseUserConfig({ collaborativeSeeds: 3 }).collaborativeSeeds).toBe(3);
    });

    it("accepts zero, which disables the signal", () => {
      expect(parseUserConfig({ collaborativeSeeds: 0 }).collaborativeSeeds).toBe(0);
    });

    it("caps the fan-out so one user cannot hammer the API", () => {
      expect(parseUserConfig({ collaborativeSeeds: 1000 }).collaborativeSeeds).toBe(10);
    });

    it("rounds a fractional value", () => {
      expect(parseUserConfig({ collaborativeSeeds: 2.7 }).collaborativeSeeds).toBe(3);
    });
  });

  describe("exclude same author", () => {
    it("defaults to keeping same-author results", () => {
      expect(parseUserConfig({}).excludeSameAuthor).toBe(false);
    });

    it("honours an explicit opt-in", () => {
      expect(parseUserConfig({ excludeSameAuthor: true }).excludeSameAuthor).toBe(true);
    });

    it("ignores a non-boolean value", () => {
      expect(parseUserConfig({ excludeSameAuthor: "yes" }).excludeSameAuthor).toBe(false);
    });
  });
});

describe("parseAdminConfig", () => {
  it("returns empty options for an absent config", () => {
    expect(parseAdminConfig(undefined)).toEqual({});
  });

  it("reads the request timeout", () => {
    expect(parseAdminConfig({ timeout: 45 }).timeout).toBe(45);
  });

  it("ignores a non-positive or non-numeric timeout", () => {
    expect(parseAdminConfig({ timeout: 0 }).timeout).toBeUndefined();
    expect(parseAdminConfig({ timeout: "soon" }).timeout).toBeUndefined();
  });

  it("reads a base URL override", () => {
    // The escape hatch if the beta endpoints move hosts.
    expect(parseAdminConfig({ base_url: "https://mb.example.test" }).baseUrl).toBe(
      "https://mb.example.test",
    );
  });

  it("ignores a blank base URL", () => {
    expect(parseAdminConfig({ base_url: "   " }).baseUrl).toBeUndefined();
  });
});
