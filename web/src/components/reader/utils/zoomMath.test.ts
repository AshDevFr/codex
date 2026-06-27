import { describe, expect, it } from "vitest";
import {
  clampPan,
  clampScale,
  focalZoom,
  IDENTITY,
  MAX_SCALE,
  MIN_SCALE,
} from "./zoomMath";

describe("clampScale", () => {
  it("clamps below the minimum to fit", () => {
    expect(clampScale(0.5)).toBe(MIN_SCALE);
    expect(clampScale(-2)).toBe(MIN_SCALE);
  });

  it("clamps above the maximum", () => {
    expect(clampScale(MAX_SCALE + 3)).toBe(MAX_SCALE);
  });

  it("passes through in-range values", () => {
    expect(clampScale(2)).toBe(2);
  });
});

describe("clampPan", () => {
  const vp = { width: 1000, height: 800 };

  it("pins translation to 0 at fit scale (no overflow to pan)", () => {
    expect(clampPan({ tx: 200, ty: 200 }, 1, vp)).toEqual({ tx: 0, ty: 0 });
  });

  it("allows panning up to half the overflow when zoomed", () => {
    // scale 2 → overflow = width*(2-1) = 1000; max pan = 500 / 400.
    expect(clampPan({ tx: 999, ty: 999 }, 2, vp)).toEqual({ tx: 500, ty: 400 });
    expect(clampPan({ tx: -999, ty: -999 }, 2, vp)).toEqual({
      tx: -500,
      ty: -400,
    });
  });

  it("leaves in-bounds translation untouched", () => {
    expect(clampPan({ tx: 100, ty: -50 }, 2, vp)).toEqual({ tx: 100, ty: -50 });
  });
});

describe("focalZoom", () => {
  it("keeps the focal point stationary while zooming in from center", () => {
    // Focus at center (0,0 relative to center): translation stays 0.
    const next = focalZoom(IDENTITY, { x: 0, y: 0 }, 2);
    expect(next.scale).toBe(2);
    expect(next.tx).toBeCloseTo(0);
    expect(next.ty).toBeCloseTo(0);
  });

  it("shifts translation so an off-center focus stays under the fingers", () => {
    // Focus 100px right of center, zoom 1 → 2. The content point under the focus
    // must stay put: t1 = f - (f - t0)*ratio = 100 - (100-0)*2 = -100.
    const next = focalZoom(IDENTITY, { x: 100, y: 0 }, 2);
    expect(next.scale).toBe(2);
    expect(next.tx).toBeCloseTo(-100);
    expect(next.ty).toBeCloseTo(0);
  });

  it("clamps the scale to the max", () => {
    const next = focalZoom(IDENTITY, { x: 0, y: 0 }, MAX_SCALE + 5);
    expect(next.scale).toBe(MAX_SCALE);
  });

  it("returns to fit when zooming back below 1", () => {
    const zoomedIn = focalZoom(IDENTITY, { x: 50, y: 50 }, 2);
    const backOut = focalZoom(zoomedIn, { x: 50, y: 50 }, 0.5);
    expect(backOut.scale).toBe(MIN_SCALE);
  });
});
