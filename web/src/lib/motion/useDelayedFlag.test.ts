import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useDelayedFlag } from "./useDelayedFlag";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useDelayedFlag", () => {
  it("returns false when the source is never true", () => {
    const { result } = renderHook(({ source }) => useDelayedFlag(source, 150), {
      initialProps: { source: false },
    });

    expect(result.current).toBe(false);

    act(() => {
      vi.advanceTimersByTime(500);
    });

    expect(result.current).toBe(false);
  });

  it("does not flip true if the source turns false before the delay", () => {
    const { result, rerender } = renderHook(
      ({ source }) => useDelayedFlag(source, 150),
      { initialProps: { source: true } },
    );

    expect(result.current).toBe(false);

    act(() => {
      vi.advanceTimersByTime(100);
    });

    rerender({ source: false });

    act(() => {
      vi.advanceTimersByTime(200);
    });

    expect(result.current).toBe(false);
  });

  it("flips true once the source stays true for at least delayMs", () => {
    const { result } = renderHook(({ source }) => useDelayedFlag(source, 150), {
      initialProps: { source: true },
    });

    expect(result.current).toBe(false);

    act(() => {
      vi.advanceTimersByTime(149);
    });
    expect(result.current).toBe(false);

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(result.current).toBe(true);
  });

  it("resets to false immediately when the source flips false after the delay", () => {
    const { result, rerender } = renderHook(
      ({ source }) => useDelayedFlag(source, 150),
      { initialProps: { source: true } },
    );

    act(() => {
      vi.advanceTimersByTime(200);
    });
    expect(result.current).toBe(true);

    rerender({ source: false });
    expect(result.current).toBe(false);
  });

  it("cleans up the pending timer on unmount", () => {
    const { result, unmount } = renderHook(
      ({ source }) => useDelayedFlag(source, 150),
      { initialProps: { source: true } },
    );

    unmount();

    act(() => {
      vi.advanceTimersByTime(500);
    });

    // Hook is gone; the last observed value remains false because the timer
    // was cleared before it could fire.
    expect(result.current).toBe(false);
  });

  it("defaults to a 150ms delay when none is provided", () => {
    const { result } = renderHook(() => useDelayedFlag(true));

    act(() => {
      vi.advanceTimersByTime(149);
    });
    expect(result.current).toBe(false);

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(result.current).toBe(true);
  });
});
