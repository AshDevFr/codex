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
});
