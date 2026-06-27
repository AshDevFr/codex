import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { usePinchZoom } from "./usePinchZoom";

const makeRefs = (w = 1000, h = 800) => {
  const viewport = document.createElement("div");
  const content = document.createElement("div");
  vi.spyOn(viewport, "getBoundingClientRect").mockReturnValue({
    left: 0,
    top: 0,
    right: w,
    bottom: h,
    width: w,
    height: h,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });
  return {
    viewportRef: { current: viewport },
    contentRef: { current: content },
    content,
  };
};

describe("usePinchZoom", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it("zooms in on pinch and reports zoomed state", () => {
    const { viewportRef, contentRef, content } = makeRefs();
    const { result } = renderHook(() =>
      usePinchZoom({ viewportRef, contentRef }),
    );

    expect(result.current.isZoomedNow()).toBe(false);

    act(() => result.current.pinch(2, { x: 0, y: 0 }));

    expect(result.current.isZoomedNow()).toBe(true);
    expect(content.style.transform).toContain("scale(2)");
  });

  it("ignores pan while at fit scale", () => {
    const { viewportRef, contentRef, content } = makeRefs();
    const { result } = renderHook(() =>
      usePinchZoom({ viewportRef, contentRef }),
    );

    act(() => result.current.panBy(50, 50));
    expect(content.style.transform).toBe("");
  });

  it("pans within bounds once zoomed and clamps at the edge", () => {
    const { viewportRef, contentRef, content } = makeRefs();
    const { result } = renderHook(() =>
      usePinchZoom({ viewportRef, contentRef }),
    );

    act(() => result.current.pinch(2, { x: 0, y: 0 }));
    // At scale 2 the max pan is width*(2-1)/2 = 500, height = 400.
    act(() => result.current.panBy(10_000, 10_000));
    expect(content.style.transform).toContain("translate3d(500px, 400px, 0)");
  });

  it("double-tap zooms in at fit and resets when already zoomed", () => {
    const { viewportRef, contentRef, content } = makeRefs();
    const { result } = renderHook(() =>
      usePinchZoom({ viewportRef, contentRef }),
    );

    // At fit: double-tap zooms in (animated).
    act(() => result.current.doubleTap({ x: 0, y: 0 }));
    expect(result.current.isZoomedNow()).toBe(true);
    expect(content.style.transform).toContain("scale(2.5)");
    expect(content.style.transition).toContain("transform");

    // Already zoomed: double-tap returns to fit.
    act(() => result.current.doubleTap({ x: 0, y: 0 }));
    expect(result.current.isZoomedNow()).toBe(false);
    expect(content.style.transform).toContain("scale(1)");
  });

  it("resets to fit", () => {
    const { viewportRef, contentRef, content } = makeRefs();
    const { result } = renderHook(() =>
      usePinchZoom({ viewportRef, contentRef }),
    );

    act(() => result.current.pinch(3, { x: 0, y: 0 }));
    expect(result.current.isZoomedNow()).toBe(true);

    act(() => result.current.reset());
    expect(result.current.isZoomedNow()).toBe(false);
    expect(content.style.transform).toContain("scale(1)");
  });
});
