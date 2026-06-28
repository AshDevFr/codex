import { describe, expect, it } from "vitest";
import { downscaleWidth } from "./pageDownscale";

describe("downscaleWidth", () => {
  it("scales by device pixel ratio and buckets to 256", () => {
    // 390 * 3 = 1170 -> next 256 bucket = 1280.
    expect(downscaleWidth(390, 3, false)).toBe(1280);
  });

  it("halves the per-page width in double-page mode", () => {
    // (1024 / 2) * 2 = 1024 -> already a bucket.
    expect(downscaleWidth(1024, 2, true)).toBe(1024);
  });

  it("clamps to the 640px minimum for small viewports", () => {
    expect(downscaleWidth(200, 1, false)).toBe(640);
  });

  it("clamps to the 2560px maximum for large viewports", () => {
    expect(downscaleWidth(3000, 3, false)).toBe(2560);
  });

  it("treats a missing/zero devicePixelRatio as 1", () => {
    // 800 -> next 256 bucket = 1024.
    expect(downscaleWidth(800, 0, false)).toBe(1024);
  });
});
