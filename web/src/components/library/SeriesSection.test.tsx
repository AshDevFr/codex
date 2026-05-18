import { act } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders } from "@/test/utils";
import { SeriesSection } from "./SeriesSection";

// Mock the API - never resolves to keep loading state
vi.mock("@/api/series", () => ({
  seriesApi: {
    search: vi.fn(() => new Promise(() => {})),
    getAlphabeticalGroups: vi.fn(() => new Promise(() => {})),
  },
}));

// Mock useSeriesFilterState hook
vi.mock("@/hooks/useSeriesFilterState", () => ({
  useSeriesFilterState: () => ({
    condition: undefined,
    hasActiveFilters: false,
    activeFilterCount: 0,
    filters: {
      genres: { mode: "anyOf", values: new Map() },
      tags: { mode: "anyOf", values: new Map() },
      status: { mode: "anyOf", values: new Map() },
      readStatus: { mode: "anyOf", values: new Map() },
      publisher: { mode: "anyOf", values: new Map() },
      language: { mode: "anyOf", values: new Map() },
      sharingTags: { mode: "anyOf", values: new Map() },
    },
    setGenreState: vi.fn(),
    setGenreMode: vi.fn(),
    setTagState: vi.fn(),
    setTagMode: vi.fn(),
    setStatusState: vi.fn(),
    setStatusMode: vi.fn(),
    setReadStatusState: vi.fn(),
    setReadStatusMode: vi.fn(),
    setPublisherState: vi.fn(),
    setPublisherMode: vi.fn(),
    setLanguageState: vi.fn(),
    setLanguageMode: vi.fn(),
    setSharingTagState: vi.fn(),
    setSharingTagMode: vi.fn(),
    clearAll: vi.fn(),
    clearGroup: vi.fn(),
    activeFiltersByGroup: {},
  }),
}));

describe("SeriesSection", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  const renderComponent = (searchParams = new URLSearchParams()) => {
    return renderWithProviders(
      <SeriesSection libraryId="test-library-id" searchParams={searchParams} />,
    );
  };

  /** Advance past the 150ms skeleton flicker guard so the placeholder renders. */
  const advancePastSkeletonDelay = () => {
    act(() => {
      vi.advanceTimersByTime(200);
    });
  };

  describe("loading state", () => {
    it("renders the shape-matched cover grid skeleton after the 150ms gate", () => {
      const { getByTestId, queryByTestId } = renderComponent();

      // Before the gate the skeleton is intentionally hidden to avoid
      // <150ms flashes on fast loads.
      expect(queryByTestId("cover-grid-skeleton")).toBeNull();

      advancePastSkeletonDelay();
      expect(getByTestId("cover-grid-skeleton")).toBeInTheDocument();
    });

    it("renders exactly `pageSize` skeleton cards when the page is small", () => {
      const searchParams = new URLSearchParams({ pageSize: "6" });
      const { getByTestId } = renderComponent(searchParams);
      advancePastSkeletonDelay();

      const grid = getByTestId("cover-grid-skeleton");
      expect(grid.children.length).toBe(6);
    });

    it("caps the skeleton at 12 cards even when `pageSize` is larger", () => {
      const searchParams = new URLSearchParams({ pageSize: "50" });
      const { getByTestId } = renderComponent(searchParams);
      advancePastSkeletonDelay();

      const grid = getByTestId("cover-grid-skeleton");
      expect(grid.children.length).toBe(12);
    });
  });
});
