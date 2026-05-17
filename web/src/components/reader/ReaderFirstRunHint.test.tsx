import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, renderWithProviders, screen, waitFor } from "@/test/utils";
import {
  __resetReaderFirstRunHintForTests,
  ReaderFirstRunHint,
} from "./ReaderFirstRunHint";

function forceMobileViewport() {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    configurable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: query.includes("max-width"),
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
}

function forceDesktopViewport() {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    configurable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
}

describe("ReaderFirstRunHint", () => {
  beforeEach(() => {
    __resetReaderFirstRunHintForTests();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("shows on first mount on a phone viewport", () => {
    forceMobileViewport();
    renderWithProviders(<ReaderFirstRunHint />);

    expect(
      screen.getByText(/tap the center to show controls/i),
    ).toBeInTheDocument();
  });

  it("does not show on the next mount in the same session", () => {
    forceMobileViewport();
    const { unmount } = renderWithProviders(<ReaderFirstRunHint />);
    expect(
      screen.getByText(/tap the center to show controls/i),
    ).toBeInTheDocument();
    unmount();

    renderWithProviders(<ReaderFirstRunHint />);
    expect(
      screen.queryByText(/tap the center to show controls/i),
    ).not.toBeInTheDocument();
  });

  it("does not show on desktop viewports", () => {
    forceDesktopViewport();
    renderWithProviders(<ReaderFirstRunHint />);

    expect(
      screen.queryByText(/tap the center to show controls/i),
    ).not.toBeInTheDocument();
  });

  it("dismisses when the hint is clicked", async () => {
    forceMobileViewport();
    vi.useRealTimers();
    renderWithProviders(<ReaderFirstRunHint />);

    fireEvent.click(
      screen.getByRole("button", { name: /dismiss reader hint/i }),
    );

    // Mantine `Transition` plays a fade-out, then removes the node. Use
    // waitFor (with real timers) rather than fake-timer advancement because
    // the fade is driven by requestAnimationFrame, which doesn't move under
    // vi.advanceTimersByTime in jsdom.
    await waitFor(() => {
      expect(
        screen.queryByText(/tap the center to show controls/i),
      ).not.toBeInTheDocument();
    });
  });

  it("schedules an auto-dismiss timer on mount", () => {
    forceMobileViewport();
    const setTimeoutSpy = vi.spyOn(window, "setTimeout");
    renderWithProviders(<ReaderFirstRunHint />);

    // useEffect schedules a setTimeout with the auto-hide delay. We can't
    // reliably observe the unmount under jsdom because Mantine's Transition
    // exit is driven by requestAnimationFrame, which doesn't advance with
    // vi.advanceTimersByTime. Asserting the schedule is sufficient; the
    // dismiss path itself is covered by the click test above.
    const found = setTimeoutSpy.mock.calls.find(([, delay]) => delay === 4000);
    expect(found).toBeDefined();
  });

  it("respects the enabled prop", () => {
    forceMobileViewport();
    renderWithProviders(<ReaderFirstRunHint enabled={false} />);

    expect(
      screen.queryByText(/tap the center to show controls/i),
    ).not.toBeInTheDocument();
  });
});
