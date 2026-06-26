import { describe, expect, it } from "vitest";
import {
  classifyTapZone,
  decideSnap,
  isHorizontalDrag,
  isHorizontallyPannable,
  isTap,
  rubberBand,
  SWIPE_ACTIVATION_PX,
  SWIPE_COMMIT_FRACTION,
  SWIPE_VELOCITY_THRESHOLD,
  TAP_TOLERANCE,
} from "./swipeGesture";

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

describe("isHorizontalDrag", () => {
  it("returns false below the activation threshold", () => {
    expect(isHorizontalDrag(SWIPE_ACTIVATION_PX - 1, 0)).toBe(false);
  });

  it("returns true once horizontal movement passes activation and dominates", () => {
    expect(isHorizontalDrag(SWIPE_ACTIVATION_PX, 0)).toBe(true);
    expect(isHorizontalDrag(-40, 10)).toBe(true);
  });

  it("returns false when vertical movement dominates (scroll/back-gesture)", () => {
    expect(isHorizontalDrag(20, 40)).toBe(false);
    // Equal magnitude is not horizontal-dominant.
    expect(isHorizontalDrag(30, 30)).toBe(false);
  });

  it("honors a custom activation threshold", () => {
    expect(isHorizontalDrag(15, 0)).toBe(true);
    expect(isHorizontalDrag(15, 0, 20)).toBe(false);
  });
});

describe("isHorizontallyPannable", () => {
  const base = {
    visualViewportScale: 1,
    contentWidth: 800,
    viewportWidth: 1000,
  };

  it("is not pannable when not zoomed and content fits the viewport", () => {
    expect(isHorizontallyPannable(base)).toBe(false);
  });

  it("is pannable when pinch-zoomed in", () => {
    expect(isHorizontallyPannable({ ...base, visualViewportScale: 1.5 })).toBe(
      true,
    );
  });

  it("ignores sub-epsilon zoom jitter", () => {
    expect(
      isHorizontallyPannable({ ...base, visualViewportScale: 1.005 }),
    ).toBe(false);
  });

  it("is pannable when content is wider than the viewport", () => {
    expect(isHorizontallyPannable({ ...base, contentWidth: 1400 })).toBe(true);
  });

  it("tolerates a 1px content overshoot from rounding", () => {
    expect(isHorizontallyPannable({ ...base, contentWidth: 1000.5 })).toBe(
      false,
    );
  });
});

describe("rubberBand", () => {
  const W = 1000;

  it("returns 0 for no drag", () => {
    expect(rubberBand(0, W)).toBe(0);
  });

  it("is near-identity for small drags", () => {
    // A 20px drag should resist only slightly.
    expect(rubberBand(20, W)).toBeGreaterThan(18);
    expect(rubberBand(20, W)).toBeLessThanOrEqual(20);
  });

  it("is monotonic increasing in drag magnitude", () => {
    expect(rubberBand(200, W)).toBeGreaterThan(rubberBand(100, W));
    expect(rubberBand(800, W)).toBeGreaterThan(rubberBand(400, W));
  });

  it("is bounded by the viewport width", () => {
    expect(rubberBand(100_000, W)).toBeLessThan(W);
  });

  it("is symmetric for negative drags", () => {
    expect(rubberBand(-300, W)).toBeCloseTo(-rubberBand(300, W));
  });

  it("returns 0 for a non-positive viewport width", () => {
    expect(rubberBand(300, 0)).toBe(0);
  });
});

describe("decideSnap", () => {
  const W = 1000;
  const slow = 0; // px/ms
  const commitPx = SWIPE_COMMIT_FRACTION * W; // 250px at fraction 0.25

  const input = (over: Partial<Parameters<typeof decideSnap>[0]>) => ({
    dragPx: 0,
    velocityPxPerMs: slow,
    viewportWidth: W,
    hasPrev: true,
    hasNext: true,
    readingDirection: "ltr" as const,
    ...over,
  });

  it("stays for a small, slow drag", () => {
    expect(decideSnap(input({ dragPx: 30 }))).toBe("stay");
  });

  it("commits next on a far leftward drag (LTR)", () => {
    expect(decideSnap(input({ dragPx: -commitPx }))).toBe("next");
  });

  it("commits prev on a far rightward drag (LTR)", () => {
    expect(decideSnap(input({ dragPx: commitPx }))).toBe("prev");
  });

  it("commits via a fast flick even on a short drag", () => {
    expect(
      decideSnap(
        input({ dragPx: -30, velocityPxPerMs: -SWIPE_VELOCITY_THRESHOLD }),
      ),
    ).toBe("next");
  });

  it("flips polarity in RTL", () => {
    expect(
      decideSnap(input({ dragPx: -commitPx, readingDirection: "rtl" })),
    ).toBe("prev");
    expect(
      decideSnap(input({ dragPx: commitPx, readingDirection: "rtl" })),
    ).toBe("next");
  });

  it("stays at the next edge when there is no next page", () => {
    expect(decideSnap(input({ dragPx: -commitPx, hasNext: false }))).toBe(
      "stay",
    );
  });

  it("stays at the prev edge when there is no previous page", () => {
    expect(decideSnap(input({ dragPx: commitPx, hasPrev: false }))).toBe(
      "stay",
    );
  });

  it("uses velocity direction over drag direction on a fast reversed flick", () => {
    // Dragged far left, but flicked right at release -> prev.
    expect(
      decideSnap(
        input({ dragPx: -commitPx, velocityPxPerMs: SWIPE_VELOCITY_THRESHOLD }),
      ),
    ).toBe("prev");
  });
});
