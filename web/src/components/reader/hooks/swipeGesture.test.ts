import { describe, expect, it } from "vitest";
import { classifyTapZone, isTap, TAP_TOLERANCE } from "./swipeGesture";

describe("isTap", () => {
  it("returns true for zero movement", () => {
    expect(isTap(0, 0)).toBe(true);
  });

  it("returns true for movement within tolerance", () => {
    expect(isTap(5, 5)).toBe(true);
    expect(isTap(-9, 9)).toBe(true);
  });

  it("returns false at the tolerance boundary", () => {
    expect(isTap(TAP_TOLERANCE, 0)).toBe(false);
  });

  it("returns false for horizontal movement above tolerance", () => {
    expect(isTap(100, 0)).toBe(false);
    expect(isTap(-100, 0)).toBe(false);
  });

  it("returns false for vertical movement above tolerance", () => {
    expect(isTap(0, 100)).toBe(false);
    expect(isTap(0, -100)).toBe(false);
  });

  it("honors a custom tapTolerance", () => {
    // 15px movement is not a tap by default (tolerance=10), but is with tolerance=20.
    expect(isTap(15, 0)).toBe(false);
    expect(isTap(15, 0, 20)).toBe(true);
  });
});

describe("classifyTapZone", () => {
  // 900x600 surface; horizontal thirds at 300/600, vertical thirds at 200/400.
  const W = 900;
  const H = 600;

  it("returns 'prev' for left third in LTR", () => {
    expect(classifyTapZone(100, 300, W, H)).toBe("prev");
  });

  it("returns 'center' for middle third in LTR", () => {
    expect(classifyTapZone(450, 300, W, H)).toBe("center");
  });

  it("returns 'next' for right third in LTR", () => {
    expect(classifyTapZone(800, 300, W, H)).toBe("next");
  });

  it("flips left/right in RTL", () => {
    expect(classifyTapZone(100, 300, W, H, { readingDirection: "rtl" })).toBe(
      "next",
    );
    expect(classifyTapZone(450, 300, W, H, { readingDirection: "rtl" })).toBe(
      "center",
    );
    expect(classifyTapZone(800, 300, W, H, { readingDirection: "rtl" })).toBe(
      "prev",
    );
  });

  it("uses vertical thirds in TTB", () => {
    expect(classifyTapZone(450, 50, W, H, { readingDirection: "ttb" })).toBe(
      "prev",
    );
    expect(classifyTapZone(450, 300, W, H, { readingDirection: "ttb" })).toBe(
      "center",
    );
    expect(classifyTapZone(450, 550, W, H, { readingDirection: "ttb" })).toBe(
      "next",
    );
  });

  it("uses vertical thirds in webtoon mode", () => {
    expect(
      classifyTapZone(450, 50, W, H, { readingDirection: "webtoon" }),
    ).toBe("prev");
    expect(
      classifyTapZone(450, 550, W, H, { readingDirection: "webtoon" }),
    ).toBe("next");
  });

  it("falls back to 'center' on a zero-sized surface", () => {
    expect(classifyTapZone(0, 0, 0, 0)).toBe("center");
  });
});
