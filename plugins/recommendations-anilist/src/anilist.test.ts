import { describe, expect, it } from "vitest";
import { getBestTitle, stripHtml } from "./anilist.js";

describe("getBestTitle", () => {
  it("prefers English title", () => {
    expect(getBestTitle({ romaji: "Shingeki no Kyojin", english: "Attack on Titan" })).toBe(
      "Attack on Titan",
    );
  });

  it("falls back to romaji", () => {
    expect(getBestTitle({ romaji: "Berserk" })).toBe("Berserk");
  });

  it("falls back to romaji when english is empty", () => {
    expect(getBestTitle({ romaji: "Berserk", english: "" })).toBe("Berserk");
  });

  it("returns Unknown when neither is set", () => {
    expect(getBestTitle({})).toBe("Unknown");
  });
});

describe("stripHtml", () => {
  it("strips basic tags", () => {
    expect(stripHtml("<p>Hello <b>world</b></p>")).toBe("Hello world");
  });

  it("converts br to newlines", () => {
    expect(stripHtml("Line 1<br>Line 2<br/>Line 3")).toBe("Line 1\nLine 2\nLine 3");
  });

  it("returns undefined for null", () => {
    expect(stripHtml(null)).toBeUndefined();
  });

  it("returns undefined for empty string after trim", () => {
    expect(stripHtml("   ")).toBe("");
  });

  it("handles complex HTML", () => {
    expect(stripHtml('<i>A story about <a href="#">heroes</a></i>')).toBe("A story about heroes");
  });

  it("decodes named HTML entities", () => {
    expect(stripHtml("Tom &amp; Jerry")).toBe("Tom & Jerry");
    expect(stripHtml("a &lt; b &gt; c")).toBe("a < b > c");
    expect(stripHtml("&quot;quoted&quot;")).toBe('"quoted"');
    expect(stripHtml("it&#39;s")).toBe("it's");
  });

  it("decodes numeric HTML entities", () => {
    expect(stripHtml("&#169; 2026")).toBe("\u00A9 2026");
    expect(stripHtml("&#x2764;")).toBe("\u2764");
  });

  it("decodes entities inside HTML", () => {
    expect(stripHtml("<p>Rock &amp; Roll</p>")).toBe("Rock & Roll");
  });

  it("preserves unknown entities as-is", () => {
    expect(stripHtml("&unknown;")).toBe("&unknown;");
  });
});
