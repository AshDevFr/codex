import { renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { useViewportZoomLock } from "./useViewportZoomLock";

const ORIGINAL = "width=device-width, initial-scale=1.0, viewport-fit=cover";

const getViewport = () =>
  document.querySelector<HTMLMetaElement>('meta[name="viewport"]');

describe("useViewportZoomLock", () => {
  beforeEach(() => {
    document.head.innerHTML = `<meta name="viewport" content="${ORIGINAL}">`;
  });

  afterEach(() => {
    document.head.innerHTML = "";
  });

  it("locks the viewport while mounted and restores it on unmount", () => {
    const { unmount } = renderHook(() => useViewportZoomLock());

    const locked = getViewport()?.getAttribute("content") ?? "";
    expect(locked).toContain("maximum-scale=1");
    expect(locked).toContain("user-scalable=no");
    // Existing tokens are preserved.
    expect(locked).toContain("width=device-width");
    expect(locked).toContain("viewport-fit=cover");

    unmount();
    expect(getViewport()?.getAttribute("content")).toBe(ORIGINAL);
  });

  it("does not duplicate tokens already present", () => {
    document.head.innerHTML =
      '<meta name="viewport" content="width=device-width, user-scalable=yes">';

    renderHook(() => useViewportZoomLock());

    const locked = getViewport()?.getAttribute("content") ?? "";
    expect(locked.match(/user-scalable/g)).toHaveLength(1);
    expect(locked).toContain("user-scalable=no");
  });

  it("prevents the iOS gesturestart pinch-zoom while mounted", () => {
    const { unmount } = renderHook(() => useViewportZoomLock());

    const event = new Event("gesturestart", { cancelable: true });
    document.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);

    unmount();

    const after = new Event("gesturestart", { cancelable: true });
    document.dispatchEvent(after);
    expect(after.defaultPrevented).toBe(false);
  });

  it("creates a viewport meta when none exists and removes it on unmount", () => {
    document.head.innerHTML = "";

    const { unmount } = renderHook(() => useViewportZoomLock());

    const created = getViewport();
    expect(created).not.toBeNull();
    expect(created?.getAttribute("content")).toContain("user-scalable=no");

    unmount();
    expect(getViewport()).toBeNull();
  });

  it("does nothing when inactive", () => {
    renderHook(() => useViewportZoomLock(false));
    expect(getViewport()?.getAttribute("content")).toBe(ORIGINAL);
  });
});
