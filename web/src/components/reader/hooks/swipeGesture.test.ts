import { describe, expect, it } from "vitest";
import { classifySwipe } from "./swipeGesture";

describe("classifySwipe", () => {
  describe("tap detection", () => {
    it("classifies minimal movement as tap", () => {
      expect(classifySwipe(2, 2, 50)).toBe("tap");
    });

    it("classifies zero movement as tap even at long press", () => {
      expect(classifySwipe(0, 0, 5000)).toBe("tap");
    });

    it("does not classify >= tapTolerance movement as tap", () => {
      expect(classifySwipe(15, 0, 50)).not.toBe("tap");
    });
  });

  describe("LTR mode", () => {
    it("returns 'next' on leftward swipe", () => {
      expect(classifySwipe(-100, 5, 200)).toBe("next");
    });

    it("returns 'prev' on rightward swipe", () => {
      expect(classifySwipe(100, 5, 200)).toBe("prev");
    });

    it("returns 'none' for sub-threshold horizontal movement", () => {
      expect(classifySwipe(30, 5, 200)).toBe("none");
    });

    it("returns 'none' when swipe is too slow", () => {
      expect(classifySwipe(-100, 5, 1000)).toBe("none");
    });

    it("ignores vertical movement in LTR mode", () => {
      expect(classifySwipe(5, -100, 200)).toBe("none");
    });
  });

  describe("RTL mode", () => {
    it("returns 'prev' on leftward swipe", () => {
      expect(classifySwipe(-100, 5, 200, { readingDirection: "rtl" })).toBe(
        "prev",
      );
    });

    it("returns 'next' on rightward swipe", () => {
      expect(classifySwipe(100, 5, 200, { readingDirection: "rtl" })).toBe(
        "next",
      );
    });
  });

  describe("TTB / webtoon mode", () => {
    it("returns 'next' on upward swipe in TTB", () => {
      expect(classifySwipe(5, -100, 200, { readingDirection: "ttb" })).toBe(
        "next",
      );
    });

    it("returns 'prev' on downward swipe in TTB", () => {
      expect(classifySwipe(5, 100, 200, { readingDirection: "ttb" })).toBe(
        "prev",
      );
    });

    it("ignores horizontal movement in TTB mode", () => {
      expect(classifySwipe(-100, 5, 200, { readingDirection: "ttb" })).toBe(
        "none",
      );
    });

    it("treats webtoon the same as TTB", () => {
      expect(classifySwipe(5, -100, 200, { readingDirection: "webtoon" })).toBe(
        "next",
      );
    });
  });

  describe("custom thresholds", () => {
    it("honors a custom minSwipeDistance", () => {
      expect(classifySwipe(60, 5, 200, { minSwipeDistance: 80 })).toBe("none");
      expect(classifySwipe(90, 5, 200, { minSwipeDistance: 80 })).toBe("prev");
    });

    it("honors a custom maxSwipeTime", () => {
      expect(classifySwipe(-100, 5, 500, { maxSwipeTime: 200 })).toBe("none");
      expect(classifySwipe(-100, 5, 500, { maxSwipeTime: 1000 })).toBe("next");
    });

    it("honors a custom tapTolerance", () => {
      // 15px movement with default tapTolerance (10) is not a tap, but with
      // tapTolerance=20 it should be.
      expect(classifySwipe(15, 0, 50, { tapTolerance: 20 })).toBe("tap");
    });
  });
});
