import { describe, expect, it } from "vitest";
import { resolveFormatBadge } from "./formatBadge";

describe("resolveFormatBadge", () => {
  it("returns null for null/undefined input", () => {
    expect(resolveFormatBadge(null)).toBeNull();
    expect(resolveFormatBadge(undefined)).toBeNull();
  });

  it("returns null for empty/whitespace input", () => {
    expect(resolveFormatBadge("")).toBeNull();
    expect(resolveFormatBadge("   ")).toBeNull();
  });

  it("maps known manga-family formats to grape", () => {
    expect(resolveFormatBadge("manga")).toEqual({
      color: "grape",
      label: "Manga",
    });
    expect(resolveFormatBadge("manhwa")).toEqual({
      color: "grape",
      label: "Manhwa",
    });
    expect(resolveFormatBadge("manhua")).toEqual({
      color: "grape",
      label: "Manhua",
    });
    expect(resolveFormatBadge("webtoon")).toEqual({
      color: "grape",
      label: "Webtoon",
    });
    expect(resolveFormatBadge("one_shot")).toEqual({
      color: "grape",
      label: "One Shot",
    });
  });

  it("maps known novel-family formats to teal", () => {
    expect(resolveFormatBadge("novel")).toEqual({
      color: "teal",
      label: "Novel",
    });
    expect(resolveFormatBadge("light_novel")).toEqual({
      color: "teal",
      label: "Light Novel",
    });
  });

  it("maps comic to orange", () => {
    expect(resolveFormatBadge("comic")).toEqual({
      color: "orange",
      label: "Comic",
    });
  });

  it("falls back to gray for unknown values with title-cased label", () => {
    expect(resolveFormatBadge("oel")).toEqual({ color: "gray", label: "Oel" });
    expect(resolveFormatBadge("doujin")).toEqual({
      color: "gray",
      label: "Doujin",
    });
    expect(resolveFormatBadge("artbook")).toEqual({
      color: "gray",
      label: "Artbook",
    });
  });

  it("looks up known values case-insensitively", () => {
    expect(resolveFormatBadge("MANGA")).toEqual({
      color: "grape",
      label: "Manga",
    });
    expect(resolveFormatBadge("Light_Novel")).toEqual({
      color: "teal",
      label: "Light Novel",
    });
  });

  it("title-cases multi-word fallback values", () => {
    expect(resolveFormatBadge("graphic_novel")).toEqual({
      color: "gray",
      label: "Graphic Novel",
    });
  });
});
