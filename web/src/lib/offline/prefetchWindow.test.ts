import { describe, expect, it } from "vitest";
import {
  getEffectivePreloadWindow,
  MAX_PREFETCH_PAGES,
  MIN_PREFETCH_DOWNLOADED,
  MIN_PREFETCH_NOT_DOWNLOADED,
} from "./prefetchWindow";

describe("getEffectivePreloadWindow", () => {
  it("respects the user setting when above the not-downloaded floor", () => {
    expect(getEffectivePreloadWindow(7, false)).toBe(7);
  });

  it("raises a low user setting to the not-downloaded floor", () => {
    expect(getEffectivePreloadWindow(1, false)).toBe(
      MIN_PREFETCH_NOT_DOWNLOADED,
    );
  });

  it("widens to the downloaded floor when the book is in the cache", () => {
    expect(getEffectivePreloadWindow(1, true)).toBe(MIN_PREFETCH_DOWNLOADED);
  });

  it("clamps user settings above the max", () => {
    expect(getEffectivePreloadWindow(99, false)).toBe(MAX_PREFETCH_PAGES);
    expect(getEffectivePreloadWindow(99, true)).toBe(MAX_PREFETCH_PAGES);
  });

  it("clamps negative user settings to the floor (never below 0)", () => {
    expect(getEffectivePreloadWindow(-5, false)).toBe(
      MIN_PREFETCH_NOT_DOWNLOADED,
    );
  });
});
