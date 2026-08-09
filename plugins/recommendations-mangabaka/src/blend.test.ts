import { describe, expect, it } from "vitest";
import {
  blendScores,
  buildBlendedReason,
  DEFAULT_CONTENT_WEIGHT,
  normalizeByMax,
  SINGLE_SIGNAL_CEILING,
} from "./scoring.js";

describe("normalizeByMax", () => {
  it("rescales so the strongest score becomes 1", () => {
    const result = normalizeByMax(
      new Map([
        [1, 10],
        [2, 5],
      ]),
    );

    expect(result.get(1)).toBe(1);
    expect(result.get(2)).toBe(0.5);
  });

  it("makes wildly different raw scales comparable", () => {
    // Live collaborative responses peaked at 10, 46, and 306 for three seeds;
    // content scores sat around 0.35. Raw values cannot be blended.
    const collaborative = normalizeByMax(new Map([[1, 306]]));
    const content = normalizeByMax(new Map([[2, 0.35]]));

    expect(collaborative.get(1)).toBe(content.get(2));
  });

  it("returns an empty map when nothing scored above zero", () => {
    expect(normalizeByMax(new Map([[1, 0]])).size).toBe(0);
    expect(normalizeByMax(new Map()).size).toBe(0);
  });

  it("drops non-positive entries", () => {
    const result = normalizeByMax(
      new Map([
        [1, 4],
        [2, 0],
      ]),
    );

    expect(result.has(2)).toBe(false);
  });
});

describe("blendScores", () => {
  it("puts the favoured signal's best single-signal match at the ceiling", () => {
    // Content is favoured by default, so a top content-only match reaches the
    // full single-signal ceiling.
    expect(blendScores({ content: 1 })).toBe(SINGLE_SIGNAL_CEILING);
  });

  it("scales the less-favoured signal's single-signal matches down", () => {
    // 0.85 * (0.4 / 0.6) when content is favoured 60/40.
    expect(blendScores({ collaborative: 1 }, { contentWeight: 0.6 })).toBe(0.57);
  });

  it("treats both signals equally at the neutral default", () => {
    expect(blendScores({ content: 1 })).toBe(SINGLE_SIGNAL_CEILING);
    expect(blendScores({ collaborative: 1 })).toBe(SINGLE_SIGNAL_CEILING);
  });

  it("moves single-signal results, not just results both signals found", () => {
    // The weight is exposed as a user setting. If it only reordered entries
    // both signals returned, which are a minority of any result set, the
    // setting would appear to do nothing.
    const contentLeaning = blendScores({ collaborative: 0.8 }, { contentWeight: 0.8 });
    const collaborativeLeaning = blendScores({ collaborative: 0.8 }, { contentWeight: 0.2 });

    expect(collaborativeLeaning).toBeGreaterThan(contentLeaning);
  });

  it("silences a signal entirely at the opposite extreme", () => {
    expect(blendScores({ collaborative: 1 }, { contentWeight: 1 })).toBe(0);
    expect(blendScores({ content: 1 }, { contentWeight: 0 })).toBe(0);
  });

  it("does not treat a missing signal as a zero score", () => {
    // Averaging in a zero would halve a perfectly good content match purely
    // because the other signal never looked at it.
    const contentOnly = blendScores({ content: 0.8 });
    const withZeroCollaborative = blendScores({ content: 0.8, collaborative: 0 });

    expect(contentOnly).toBeGreaterThan(withZeroCollaborative);
  });

  it("ranks agreement above the best either signal can reach alone", () => {
    // The key property. Per-response normalisation puts the top entry of each
    // signal at exactly 1.0, so without reserved headroom a both-signals result
    // could only ever tie with a content-only one.
    const both = blendScores({ content: 1, collaborative: 1 });

    expect(both).toBe(1);
    expect(both).toBeGreaterThan(blendScores({ content: 1 }));
    expect(both).toBeGreaterThan(blendScores({ collaborative: 1 }));
  });

  it("lets agreement lift an equally-ranked content match above a content-only one", () => {
    // Equal content standing, so the collaborative endorsement is the only
    // difference and has to be what decides it.
    expect(blendScores({ content: 1, collaborative: 1 })).toBeGreaterThan(
      blendScores({ content: 1 }),
    );
  });

  it("keeps a strong single-signal match ahead of a weak agreement", () => {
    // Agreement is evidence, not a trump card.
    expect(blendScores({ content: 1 })).toBeGreaterThan(
      blendScores({ content: 0.1, collaborative: 0.1 }),
    );
  });

  it("weights the two terms by the content weight", () => {
    expect(blendScores({ content: 1, collaborative: 0 }, { contentWeight: 0.75 })).toBe(0.75);
    expect(blendScores({ content: 0, collaborative: 1 }, { contentWeight: 0.75 })).toBe(0.25);
  });

  it("ignores the collaborative term at full content weight", () => {
    expect(blendScores({ content: 0.4, collaborative: 1 }, { contentWeight: 1 })).toBe(0.4);
  });

  it("keeps a strong single-signal match ahead of a weak agreement at any weight", () => {
    expect(blendScores({ content: 1 })).toBeGreaterThan(
      blendScores({ content: 0.1, collaborative: 0.1 }),
    );
  });

  it("ignores the content term at zero content weight", () => {
    expect(blendScores({ content: 1, collaborative: 0.5 }, { contentWeight: 0 })).toBe(0.5);
  });

  it("clamps out-of-range inputs", () => {
    expect(blendScores({ content: 5, collaborative: 5 })).toBe(1);
    expect(blendScores({ content: -1 })).toBe(0);
  });

  it("rounds to two decimals", () => {
    expect(blendScores({ content: 0.777, collaborative: 0.777 })).toBe(0.78);
  });

  it("returns zero when neither signal has a score", () => {
    expect(blendScores({})).toBe(0);
  });

  it("defaults to a neutral weight so neither signal is privileged", () => {
    expect(DEFAULT_CONTENT_WEIGHT).toBe(0.5);
  });
});

describe("buildBlendedReason", () => {
  it("keeps the shared-tag phrasing for a content-only match", () => {
    const reason = buildBlendedReason({
      sharedTags: [{ id: 1, name: "Isekai", weight: "core" }],
      contentSeedTitles: ["Berserk"],
      collaborativeSeedTitles: [],
    });

    expect(reason).toBe("Shares Isekai with Berserk");
  });

  it("attributes a collaborative-only match to other readers", () => {
    const reason = buildBlendedReason({
      sharedTags: null,
      contentSeedTitles: [],
      collaborativeSeedTitles: ["Solo Leveling"],
    });

    expect(reason).toBe("Readers of Solo Leveling also read this");
  });

  it("names several endorsing seeds", () => {
    const reason = buildBlendedReason({
      sharedTags: null,
      contentSeedTitles: [],
      collaborativeSeedTitles: ["Solo Leveling", "Berserk"],
    });

    expect(reason).toBe("Readers of Solo Leveling and Berserk also read this");
  });

  it("summarises rather than listing every endorsing seed", () => {
    const reason = buildBlendedReason({
      sharedTags: null,
      contentSeedTitles: [],
      collaborativeSeedTitles: ["A", "B", "C", "D"],
    });

    expect(reason).toBe("Readers of A, B and 2 more also read this");
  });

  it("combines both provenances when both signals matched", () => {
    const reason = buildBlendedReason({
      sharedTags: [{ id: 1, name: "Time Loop", weight: "core" }],
      contentSeedTitles: ["Berserk"],
      collaborativeSeedTitles: ["Solo Leveling"],
    });

    expect(reason).toBe("Shares Time Loop with Berserk, and readers of Solo Leveling also read it");
  });

  it("falls back to the generic phrasing with nothing to attribute", () => {
    const reason = buildBlendedReason({
      sharedTags: null,
      contentSeedTitles: [],
      collaborativeSeedTitles: [],
    });

    expect(reason).toBe("Matches your library's taste profile");
  });

  it("names the same seed in both clauses when both signals point at it", () => {
    // Not a duplicate to be removed: content similarity and reader overlap are
    // independent pieces of evidence that happen to share a source.
    const reason = buildBlendedReason({
      sharedTags: null,
      contentSeedTitles: ["Berserk"],
      collaborativeSeedTitles: ["Berserk"],
    });

    expect(reason).toBe("Similar to Berserk, and readers of Berserk also read it");
  });
});
