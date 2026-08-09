import { describe, expect, it } from "vitest";
import { normalizeTitle } from "./titles.js";

describe("normalizeTitle", () => {
  it("replaces punctuation with a space and lowercases", () => {
    expect(normalizeTitle("Re:ZERO -Starting Life-")).toBe("re zero starting life");
  });

  it("treats a separator and a space as equivalent", () => {
    expect(normalizeTitle("Re:Zero")).toBe(normalizeTitle("Re: Zero"));
  });

  it("collapses runs of whitespace", () => {
    expect(normalizeTitle("  Solo    Leveling  ")).toBe("solo leveling");
  });

  it("strips diacritics so romanisations agree", () => {
    expect(normalizeTitle("Ōkami")).toBe(normalizeTitle("Okami"));
  });

  it("treats different dash characters alike", () => {
    expect(normalizeTitle("Dai-5 Shou—Mizu")).toBe(normalizeTitle("Dai 5 Shou Mizu"));
  });

  describe("volume and chapter suffixes", () => {
    it("cuts a comma-separated chapter suffix", () => {
      expect(normalizeTitle("Re:ZERO -Starting Life in Another World-, Chapter 1: A Day")).toBe(
        "re zero starting life in another world",
      );
    });

    it("cuts a bare chapter suffix", () => {
      expect(
        normalizeTitle("Re:ZERO -Starting Life in Another World- Chapter 4: The Sanctuary"),
      ).toBe("re zero starting life in another world");
    });

    it("groups the chapter volumes of one franchise under the same key", () => {
      const one = normalizeTitle("Re:ZERO -Starting Life in Another World-, Chapter 1: A Day");
      const four = normalizeTitle("Re:ZERO -Starting Life in Another World- Chapter 4: The Witch");

      expect(one).toBe(four);
    });

    it.each([
      "Berserk Vol. 3",
      "Berserk Volume 3",
      "Berserk, Vol 3",
      "Berserk Part 3",
      "Berserk Season 3",
      "Berserk Book 3",
      "Berserk Arc 3",
    ])("cuts the trailing volume marker in %s", (title) => {
      expect(normalizeTitle(title)).toBe("berserk");
    });

    it("does not cut a marker word that carries no number", () => {
      // "Chapter" is part of the actual title here, not a volume suffix.
      expect(normalizeTitle("The Final Chapter")).toBe("the final chapter");
    });

    it("does not cut a number that is part of the title", () => {
      expect(normalizeTitle("20th Century Boys")).toBe("20th century boys");
      expect(normalizeTitle("100")).toBe("100");
    });

    it("never reduces a title to nothing", () => {
      // Cutting must not leave an empty key, which would collapse unrelated
      // series into one bucket.
      expect(normalizeTitle("Chapter 1")).toBe("chapter 1");
      expect(normalizeTitle("Vol. 5")).toBe("vol 5");
    });
  });

  it("returns an empty string for empty or whitespace input", () => {
    expect(normalizeTitle("")).toBe("");
    expect(normalizeTitle("   ")).toBe("");
  });

  it("tolerates non-string input", () => {
    expect(normalizeTitle(undefined)).toBe("");
    expect(normalizeTitle(null)).toBe("");
  });

  it("keeps genuinely different series distinct", () => {
    expect(normalizeTitle("Solo Leveling")).not.toBe(normalizeTitle("Solo Leveling Ragnarok"));
    expect(normalizeTitle("Berserk")).not.toBe(normalizeTitle("Berserk of Gluttony"));
  });

  it("preserves CJK titles rather than stripping them to nothing", () => {
    expect(normalizeTitle("나 혼자만 레벨업")).toBe("나 혼자만 레벨업");
  });
});
