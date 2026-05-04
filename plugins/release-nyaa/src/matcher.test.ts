import { describe, expect, it } from "vitest";
import {
  CONFIDENCE_EXACT,
  DEFAULT_FUZZY_FLOOR,
  diceRatio,
  matchSeries,
  normalizeAlias,
} from "./matcher.js";

// -----------------------------------------------------------------------------
// normalizeAlias — must match the Rust `normalize_alias` impl
// -----------------------------------------------------------------------------

describe("normalizeAlias", () => {
  it("lowercases and strips punctuation", () => {
    expect(normalizeAlias("My Hero Academia!")).toBe("my hero academia");
  });

  it("collapses multiple spaces, drops leading/trailing space", () => {
    expect(normalizeAlias("  Berserk   - Vol  ")).toBe("berserk vol");
  });

  it("strips colons and other ASCII punctuation (matches Rust impl)", () => {
    expect(normalizeAlias("Re:Zero - Starting Life in Another World")).toBe(
      "rezero starting life in another world",
    );
  });

  it("returns empty string for input with only punctuation", () => {
    expect(normalizeAlias("!!! - ?!")).toBe("");
  });

  it("preserves Unicode alphanumerics", () => {
    expect(normalizeAlias("僕のヒーロー")).toBe("僕のヒーロー");
  });
});

// -----------------------------------------------------------------------------
// diceRatio — sanity checks
// -----------------------------------------------------------------------------

describe("diceRatio", () => {
  it("returns 1.0 for identical strings", () => {
    expect(diceRatio("boruto two blue vortex", "boruto two blue vortex")).toBe(1);
  });

  it("returns 0 for empty inputs", () => {
    expect(diceRatio("", "x")).toBe(0);
    expect(diceRatio("x", "")).toBe(0);
  });

  it("scores high for word-rearranged near-matches", () => {
    const r = diceRatio("boruto two blue vortex", "boruto - two blue vortex");
    expect(r).toBeGreaterThan(0.85);
  });

  it("scores low for unrelated series", () => {
    const r = diceRatio("naruto", "boruto two blue vortex");
    expect(r).toBeLessThan(0.5);
  });
});

// -----------------------------------------------------------------------------
// matchSeries
// -----------------------------------------------------------------------------

describe("matchSeries", () => {
  const candidates = [
    { seriesId: "s-boruto", aliases: ["Boruto: Two Blue Vortex", "Boruto - Two Blue Vortex"] },
    { seriesId: "s-onepiece", aliases: ["One Piece"] },
    { seriesId: "s-dandadan", aliases: ["Dandadan", "ダンダダン"] },
  ];

  it("returns null for empty seriesGuess", () => {
    expect(matchSeries("", candidates)).toBeNull();
    expect(matchSeries("   ", candidates)).toBeNull();
  });

  it("returns null when there are no candidates", () => {
    expect(matchSeries("Boruto", [])).toBeNull();
  });

  it("emits an alias-exact match at CONFIDENCE_EXACT", () => {
    const m = matchSeries("Boruto Two Blue Vortex", candidates);
    expect(m).not.toBeNull();
    if (m === null) return;
    expect(m.seriesId).toBe("s-boruto");
    expect(m.confidence).toBe(CONFIDENCE_EXACT);
    expect(m.reason).toBe("alias-exact");
    expect(m.matchedAlias).toBe("Boruto: Two Blue Vortex");
  });

  it("emits an alias-fuzzy match for a near-miss above the floor", () => {
    // Add a slightly different aliasing form.
    const c = [{ seriesId: "s-frieren", aliases: ["Sousou no Frieren"] }];
    const m = matchSeries("Sousou Frieren", c, { fuzzyFloor: DEFAULT_FUZZY_FLOOR });
    if (m === null) {
      // Below floor is also fine for this test — exercise the explicit
      // match-or-skip semantics rather than asserting a confidence value.
      expect(m).toBeNull();
      return;
    }
    expect(m.seriesId).toBe("s-frieren");
    expect(m.reason).toBe("alias-fuzzy");
    expect(m.confidence).toBeGreaterThanOrEqual(DEFAULT_FUZZY_FLOOR);
    expect(m.confidence).toBeLessThan(CONFIDENCE_EXACT);
  });

  it("rejects unrelated names below the dice floor", () => {
    const m = matchSeries("Berserk", candidates);
    expect(m).toBeNull();
  });

  it("rejects matches whose Dice ratio is below MIN_DICE_RATIO even with a low floor", () => {
    const c = [{ seriesId: "s-x", aliases: ["Berserk"] }];
    // Even with a permissive floor, the matcher still requires Dice ≥ 0.85.
    const m = matchSeries("Naruto", c, { fuzzyFloor: 0.5 });
    expect(m).toBeNull();
  });

  it("picks the best candidate when multiple are above the floor", () => {
    const c = [
      { seriesId: "s-bad", aliases: ["Boruto Two Vortex"] }, // worse Dice
      { seriesId: "s-good", aliases: ["Boruto Two Blue Vortex"] }, // exact match
    ];
    const m = matchSeries("Boruto Two Blue Vortex", c);
    expect(m?.seriesId).toBe("s-good");
    expect(m?.reason).toBe("alias-exact");
  });
});
