import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAutoAdvanceCountdown } from "./useAutoAdvanceCountdown";

describe("useAutoAdvanceCountdown", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("counts down and fires onElapsed once when active", () => {
    const onElapsed = vi.fn();
    const { result } = renderHook(() =>
      useAutoAdvanceCountdown({ active: true, seconds: 3, onElapsed }),
    );

    expect(result.current.remaining).toBe(3);

    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(result.current.remaining).toBe(2);

    act(() => {
      vi.advanceTimersByTime(2000);
    });
    expect(result.current.remaining).toBe(0);
    expect(onElapsed).toHaveBeenCalledTimes(1);

    // Staying mounted at 0 must not fire again.
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(onElapsed).toHaveBeenCalledTimes(1);
  });

  it("does not fire onElapsed after cancel()", () => {
    const onElapsed = vi.fn();
    const { result } = renderHook(() =>
      useAutoAdvanceCountdown({ active: true, seconds: 3, onElapsed }),
    );

    act(() => {
      vi.advanceTimersByTime(1000);
    });
    act(() => {
      result.current.cancel();
    });
    expect(result.current.cancelled).toBe(true);

    act(() => {
      vi.advanceTimersByTime(10000);
    });
    expect(onElapsed).not.toHaveBeenCalled();
  });

  it("stays inert when not active", () => {
    const onElapsed = vi.fn();
    const { result } = renderHook(() =>
      useAutoAdvanceCountdown({ active: false, seconds: 3, onElapsed }),
    );

    expect(result.current.remaining).toBe(3);
    act(() => {
      vi.advanceTimersByTime(10000);
    });
    expect(onElapsed).not.toHaveBeenCalled();
    expect(result.current.remaining).toBe(3);
  });

  it("defaults to 5 seconds when seconds is omitted", () => {
    const onElapsed = vi.fn();
    const { result } = renderHook(() =>
      useAutoAdvanceCountdown({ active: true, onElapsed }),
    );

    expect(result.current.remaining).toBe(5);
  });

  it("restarts cleanly when re-activated after deactivation", () => {
    const onElapsed = vi.fn();
    const { result, rerender } = renderHook(
      ({ active }: { active: boolean }) =>
        useAutoAdvanceCountdown({ active, seconds: 3, onElapsed }),
      { initialProps: { active: true } },
    );

    act(() => {
      result.current.cancel();
    });
    expect(result.current.cancelled).toBe(true);

    // Deactivate, then re-activate: cancelled resets, countdown runs again.
    rerender({ active: false });
    expect(result.current.cancelled).toBe(false);
    expect(result.current.remaining).toBe(3);

    rerender({ active: true });
    act(() => {
      vi.advanceTimersByTime(3000);
    });
    expect(onElapsed).toHaveBeenCalledTimes(1);
  });
});
