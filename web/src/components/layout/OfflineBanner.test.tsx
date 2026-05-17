import { act } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { renderWithProviders, screen } from "@/test/utils";
import { OfflineBanner } from "./OfflineBanner";

function setOnline(value: boolean) {
  Object.defineProperty(navigator, "onLine", {
    configurable: true,
    get: () => value,
  });
}

describe("OfflineBanner (U6)", () => {
  afterEach(() => {
    // Restore the default — JSDOM treats the property as writable but tests
    // share globals, so reset after each test.
    setOnline(true);
  });

  it("renders nothing while online", () => {
    setOnline(true);
    renderWithProviders(<OfflineBanner />);

    expect(screen.queryByText(/offline/i)).not.toBeInTheDocument();
  });

  it("shows the banner on initial mount when navigator.onLine is false", () => {
    setOnline(false);
    renderWithProviders(<OfflineBanner />);

    expect(
      screen.getByText(/you're offline\. showing cached content/i),
    ).toBeInTheDocument();
  });

  it("appears when the window dispatches an offline event", () => {
    setOnline(true);
    renderWithProviders(<OfflineBanner />);

    expect(screen.queryByText(/offline/i)).not.toBeInTheDocument();

    act(() => {
      setOnline(false);
      window.dispatchEvent(new Event("offline"));
    });

    expect(
      screen.getByText(/you're offline\. showing cached content/i),
    ).toBeInTheDocument();
  });

  it("disappears when the window dispatches an online event", () => {
    setOnline(false);
    renderWithProviders(<OfflineBanner />);
    expect(
      screen.getByText(/you're offline\. showing cached content/i),
    ).toBeInTheDocument();

    act(() => {
      setOnline(true);
      window.dispatchEvent(new Event("online"));
    });

    expect(screen.queryByText(/offline/i)).not.toBeInTheDocument();
  });
});
