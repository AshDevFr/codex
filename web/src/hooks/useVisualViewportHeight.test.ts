import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useVisualViewportHeight } from "./useVisualViewportHeight";

interface MockVisualViewport {
  height: number;
  addEventListener: ReturnType<typeof vi.fn>;
  removeEventListener: ReturnType<typeof vi.fn>;
  dispatchResize: () => void;
}

function installMockVisualViewport(height: number): MockVisualViewport {
  const listeners = new Set<() => void>();
  const viewport: MockVisualViewport = {
    height,
    addEventListener: vi.fn((event: string, handler: () => void) => {
      if (event === "resize") listeners.add(handler);
    }),
    removeEventListener: vi.fn((event: string, handler: () => void) => {
      if (event === "resize") listeners.delete(handler);
    }),
    dispatchResize: () => {
      for (const handler of listeners) handler();
    },
  };
  Object.defineProperty(window, "visualViewport", {
    configurable: true,
    value: viewport,
  });
  return viewport;
}

function removeMockVisualViewport() {
  Object.defineProperty(window, "visualViewport", {
    configurable: true,
    value: undefined,
  });
}

describe("useVisualViewportHeight", () => {
  afterEach(() => {
    removeMockVisualViewport();
    vi.restoreAllMocks();
  });

  it("returns null when the Visual Viewport API is unavailable", () => {
    removeMockVisualViewport();
    const { result } = renderHook(() => useVisualViewportHeight(true));
    expect(result.current).toBeNull();
  });

  it("returns the current viewport height when active", () => {
    installMockVisualViewport(800);
    const { result } = renderHook(() => useVisualViewportHeight(true));
    expect(result.current).toBe(800);
  });

  it("updates when the visual viewport resizes (keyboard opens)", () => {
    const viewport = installMockVisualViewport(800);
    const { result } = renderHook(() => useVisualViewportHeight(true));

    act(() => {
      viewport.height = 450;
      viewport.dispatchResize();
    });

    expect(result.current).toBe(450);
  });

  it("returns null and does not subscribe when inactive", () => {
    const viewport = installMockVisualViewport(800);
    const { result } = renderHook(() => useVisualViewportHeight(false));
    expect(result.current).toBeNull();
    expect(viewport.addEventListener).not.toHaveBeenCalled();
  });

  it("unsubscribes on unmount", () => {
    const viewport = installMockVisualViewport(800);
    const { unmount } = renderHook(() => useVisualViewportHeight(true));
    unmount();
    expect(viewport.removeEventListener).toHaveBeenCalledWith(
      "resize",
      expect.any(Function),
    );
  });

  it("resets to null when deactivated", () => {
    installMockVisualViewport(800);
    const { result, rerender } = renderHook(
      ({ active }) => useVisualViewportHeight(active),
      { initialProps: { active: true } },
    );
    expect(result.current).toBe(800);

    rerender({ active: false });
    expect(result.current).toBeNull();
  });
});
