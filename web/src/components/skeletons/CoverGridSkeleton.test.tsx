import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/utils";
import { CoverGridSkeleton } from "./CoverGridSkeleton";

function setMatchMedia(matches: (query: string) => boolean) {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    configurable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: matches(query),
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

const forceMobile = () =>
  setMatchMedia((query) => query.includes("max-width: 30.0625em"));

const forceDesktop = () => setMatchMedia(() => false);

beforeEach(() => {
  forceDesktop();
});

describe("CoverGridSkeleton", () => {
  it("renders the requested number of cards when `count` is set", () => {
    renderWithProviders(<CoverGridSkeleton count={5} />);
    const container = screen.getByTestId("cover-grid-skeleton");
    expect(container.children.length).toBe(5);
  });

  it("defaults to 12 cards on desktop", () => {
    renderWithProviders(<CoverGridSkeleton />);
    const container = screen.getByTestId("cover-grid-skeleton");
    expect(container.children.length).toBe(12);
  });

  it("defaults to 6 cards on mobile so the 2-column grid is filled", () => {
    forceMobile();
    renderWithProviders(<CoverGridSkeleton />);
    const container = screen.getByTestId("cover-grid-skeleton");
    expect(container.children.length).toBe(6);
  });

  it("`exactCount` ignores the mobile/desktop heuristic", () => {
    forceMobile();
    renderWithProviders(<CoverGridSkeleton count={9} exactCount />);
    const container = screen.getByTestId("cover-grid-skeleton");
    expect(container.children.length).toBe(9);
  });

  it("uses the cover aspect ratio so the layout does not shift", () => {
    renderWithProviders(<CoverGridSkeleton count={1} />);
    const container = screen.getByTestId("cover-grid-skeleton");
    const cover = container.firstElementChild?.firstElementChild as HTMLElement;
    expect(cover.style.aspectRatio).toBe("150/212.125");
  });
});
