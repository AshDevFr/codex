import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useReducedMotion } from "./useReducedMotion";

type Listener = (event: MediaQueryListEvent) => void;

interface MockMediaQueryList {
  matches: boolean;
  addEventListener: (type: "change", listener: Listener) => void;
  removeEventListener: (type: "change", listener: Listener) => void;
  trigger: (matches: boolean) => void;
}

const originalMatchMedia = window.matchMedia;

function installMatchMediaMock(initial: boolean): MockMediaQueryList {
  const listeners = new Set<Listener>();
  const mql: MockMediaQueryList = {
    matches: initial,
    addEventListener: (_type, listener) => {
      listeners.add(listener);
    },
    removeEventListener: (_type, listener) => {
      listeners.delete(listener);
    },
    trigger: (matches: boolean) => {
      mql.matches = matches;
      const event = { matches, media: "" } as unknown as MediaQueryListEvent;
      for (const listener of listeners) listener(event);
    },
  };

  // Mantine's MantineProvider also calls matchMedia for the color scheme query.
  // Only return our mock for the reduced-motion query so other consumers in
  // jsdom (if any wake up during this test) still get a sane stub.
  window.matchMedia = vi.fn((query: string) => {
    if (query === "(prefers-reduced-motion: reduce)") {
      return mql as unknown as MediaQueryList;
    }
    return {
      matches: false,
      addEventListener: () => {},
      removeEventListener: () => {},
    } as unknown as MediaQueryList;
  }) as typeof window.matchMedia;

  return mql;
}

beforeEach(() => {
  installMatchMediaMock(false);
});

afterEach(() => {
  window.matchMedia = originalMatchMedia;
});

describe("useReducedMotion", () => {
  it("returns the initial matchMedia value", () => {
    installMatchMediaMock(true);
    const { result } = renderHook(() => useReducedMotion());
    expect(result.current).toBe(true);
  });

  it("returns false when the OS is not requesting reduced motion", () => {
    installMatchMediaMock(false);
    const { result } = renderHook(() => useReducedMotion());
    expect(result.current).toBe(false);
  });

  it("updates when the media query state changes", () => {
    const mql = installMatchMediaMock(false);
    const { result } = renderHook(() => useReducedMotion());

    expect(result.current).toBe(false);

    act(() => {
      mql.trigger(true);
    });

    expect(result.current).toBe(true);

    act(() => {
      mql.trigger(false);
    });

    expect(result.current).toBe(false);
  });
});
