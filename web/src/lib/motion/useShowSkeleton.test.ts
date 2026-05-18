import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SKELETON_DELAY_MS, useShowSkeleton } from "./useShowSkeleton";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useShowSkeleton", () => {
  it("exposes the 150ms shared constant", () => {
    expect(SKELETON_DELAY_MS).toBe(150);
  });

  it("returns false immediately when loading is false", () => {
    const { result } = renderHook(
      ({ isLoading }) => useShowSkeleton(isLoading),
      { initialProps: { isLoading: false } },
    );
    expect(result.current).toBe(false);
  });

  it("stays false until the 150ms gate elapses", () => {
    const { result } = renderHook(
      ({ isLoading }) => useShowSkeleton(isLoading),
      { initialProps: { isLoading: true } },
    );

    expect(result.current).toBe(false);

    act(() => {
      vi.advanceTimersByTime(SKELETON_DELAY_MS - 1);
    });
    expect(result.current).toBe(false);

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(result.current).toBe(true);
  });

  it("never flips true if loading finishes before the gate elapses", () => {
    const { result, rerender } = renderHook(
      ({ isLoading }) => useShowSkeleton(isLoading),
      { initialProps: { isLoading: true } },
    );

    act(() => {
      vi.advanceTimersByTime(100);
    });
    rerender({ isLoading: false });

    act(() => {
      vi.advanceTimersByTime(500);
    });
    expect(result.current).toBe(false);
  });
});
