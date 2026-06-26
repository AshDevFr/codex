import { describe, expect, it } from "vitest";
import { reservedPageHeight } from "./pageHeight";

describe("reservedPageHeight", () => {
  const tall = { width: 800, height: 2400 }; // 1:3 webtoon strip
  const wide = { width: 2000, height: 1000 }; // 2:1 landscape

  it("returns null for invalid dimensions", () => {
    expect(
      reservedPageHeight({
        fitMode: "width",
        contentWidth: 600,
        viewportHeight: 900,
        dimension: { width: 0, height: 100 },
      }),
    ).toBeNull();
    expect(
      reservedPageHeight({
        fitMode: "width",
        contentWidth: 600,
        viewportHeight: 900,
        dimension: { width: 100, height: 0 },
      }),
    ).toBeNull();
  });

  describe("width", () => {
    it("scales the image to the full content width (preserving aspect)", () => {
      // 600px wide → 600 * (2400/800) = 1800px tall
      expect(
        reservedPageHeight({
          fitMode: "width",
          contentWidth: 600,
          viewportHeight: 900,
          dimension: tall,
        }),
      ).toBe(1800);
    });

    it("scales up a narrow image to fill the width", () => {
      // image is only 400 wide, content is 600 → scaled up to 600
      expect(
        reservedPageHeight({
          fitMode: "width",
          contentWidth: 600,
          viewportHeight: 900,
          dimension: { width: 400, height: 800 },
        }),
      ).toBe(1200);
    });

    it("returns null when the content width isn't known yet", () => {
      expect(
        reservedPageHeight({
          fitMode: "width",
          contentWidth: 0,
          viewportHeight: 900,
          dimension: tall,
        }),
      ).toBeNull();
    });
  });

  describe("width-shrink", () => {
    it("caps the width at content width for an oversized image", () => {
      expect(
        reservedPageHeight({
          fitMode: "width-shrink",
          contentWidth: 600,
          viewportHeight: 900,
          dimension: tall,
        }),
      ).toBe(1800);
    });

    it("keeps natural size for an image narrower than the content width", () => {
      // 400 < 600 → natural 800px height
      expect(
        reservedPageHeight({
          fitMode: "width-shrink",
          contentWidth: 600,
          viewportHeight: 900,
          dimension: { width: 400, height: 800 },
        }),
      ).toBe(800);
    });
  });

  describe("original", () => {
    it("returns the natural pixel height regardless of layout", () => {
      expect(
        reservedPageHeight({
          fitMode: "original",
          contentWidth: 600,
          viewportHeight: 900,
          dimension: tall,
        }),
      ).toBe(2400);
    });
  });

  describe("height", () => {
    it("pins the height to the viewport", () => {
      expect(
        reservedPageHeight({
          fitMode: "height",
          contentWidth: 600,
          viewportHeight: 900,
          dimension: tall,
        }),
      ).toBe(900);
    });
  });

  describe("screen", () => {
    it("fits a tall strip by height", () => {
      // scale = min(1, 600/800, 900/2400) = min(1, 0.75, 0.375) = 0.375
      // height = 2400 * 0.375 = 900
      expect(
        reservedPageHeight({
          fitMode: "screen",
          contentWidth: 600,
          viewportHeight: 900,
          dimension: tall,
        }),
      ).toBe(900);
    });

    it("fits a wide page by width", () => {
      // scale = min(1, 600/2000, 900/1000) = min(1, 0.3, 0.9) = 0.3
      // height = 1000 * 0.3 = 300
      expect(
        reservedPageHeight({
          fitMode: "screen",
          contentWidth: 600,
          viewportHeight: 900,
          dimension: wide,
        }),
      ).toBe(300);
    });

    it("never scales an image up", () => {
      // small image, large viewport → scale capped at 1 → natural height
      expect(
        reservedPageHeight({
          fitMode: "screen",
          contentWidth: 4000,
          viewportHeight: 4000,
          dimension: { width: 400, height: 600 },
        }),
      ).toBe(600);
    });
  });
});
