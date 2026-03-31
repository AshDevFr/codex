import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { RecommendationDto } from "@/api/recommendations";
import { renderWithProviders, userEvent } from "@/test/utils";
import {
  activeFilterCount,
  applyFilters,
  DEFAULT_FILTERS,
  extractFilterOptions,
  type RecommendationFilterState,
  RecommendationFilters,
  type TriStateGroup,
} from "./RecommendationFilters";

/** Helper to create a TriStateGroup with include values */
function includeGroup(...values: string[]): TriStateGroup {
  const map = new Map<string, "include" | "exclude" | "neutral">();
  for (const v of values) map.set(v, "include");
  return { values: map };
}

/** Helper to create a TriStateGroup with exclude values */
function excludeGroup(...values: string[]): TriStateGroup {
  const map = new Map<string, "include" | "exclude" | "neutral">();
  for (const v of values) map.set(v, "exclude");
  return { values: map };
}

/** Helper to create a mixed TriStateGroup */
function mixedGroup(includes: string[], excludes: string[]): TriStateGroup {
  const map = new Map<string, "include" | "exclude" | "neutral">();
  for (const v of includes) map.set(v, "include");
  for (const v of excludes) map.set(v, "exclude");
  return { values: map };
}

// =============================================================================
// Test Data
// =============================================================================

function makeRec(overrides: Partial<RecommendationDto>): RecommendationDto {
  return {
    externalId: "1",
    title: "Test",
    score: 0.5,
    reason: "test",
    inLibrary: false,
    ...overrides,
  };
}

const sampleRecs: RecommendationDto[] = [
  makeRec({
    externalId: "1",
    title: "Berserk",
    score: 0.95,
    status: "ongoing",
    genres: ["Action", "Dark Fantasy"],
    tags: [
      { name: "Revenge", rank: 1, category: "Theme" },
      { name: "Gore", rank: 2, category: "Theme" },
    ],
    format: "MANGA",
    countryOfOrigin: "JP",
    basedOn: ["Vinland Saga"],
  }),
  makeRec({
    externalId: "2",
    title: "Solo Leveling",
    score: 0.8,
    status: "ended",
    genres: ["Action", "Fantasy"],
    tags: [
      { name: "Overpowered", rank: 1, category: "Theme" },
      { name: "Revenge", rank: 2, category: "Theme" },
    ],
    format: "MANGA",
    countryOfOrigin: "KR",
    basedOn: ["Tower of God"],
  }),
  makeRec({
    externalId: "3",
    title: "Light Novel X",
    score: 0.3,
    status: "ongoing",
    genres: ["Romance"],
    tags: [{ name: "School Life", rank: 1, category: "Setting" }],
    format: "NOVEL",
    countryOfOrigin: "JP",
    basedOn: ["Vinland Saga"],
  }),
];

// =============================================================================
// extractFilterOptions Tests
// =============================================================================

describe("extractFilterOptions", () => {
  it("extracts unique values across all dimensions", () => {
    const opts = extractFilterOptions(sampleRecs);
    expect(opts.statuses).toEqual(new Set(["ongoing", "ended"]));
    expect(opts.genres).toEqual(
      new Set(["Action", "Dark Fantasy", "Fantasy", "Romance"]),
    );
    expect(opts.tags).toEqual(
      new Set(["Gore", "Overpowered", "Revenge", "School Life"]),
    );
    expect(opts.formats).toEqual(new Set(["MANGA", "NOVEL"]));
    expect(opts.countries).toEqual(new Set(["JP", "KR"]));
    expect(opts.seeds).toEqual(new Set(["Vinland Saga", "Tower of God"]));
  });

  it("handles empty list", () => {
    const opts = extractFilterOptions([]);
    expect(opts.statuses.size).toBe(0);
    expect(opts.genres.size).toBe(0);
  });
});

// =============================================================================
// applyFilters Tests
// =============================================================================

describe("applyFilters", () => {
  it("returns all when no filters active", () => {
    const result = applyFilters(sampleRecs, { ...DEFAULT_FILTERS });
    expect(result).toHaveLength(3);
  });

  it("includes by status", () => {
    const result = applyFilters(sampleRecs, {
      ...DEFAULT_FILTERS,
      statuses: includeGroup("ongoing"),
    });
    expect(result).toHaveLength(2);
    expect(result.map((r) => r.externalId)).toEqual(["1", "3"]);
  });

  it("excludes by status", () => {
    const result = applyFilters(sampleRecs, {
      ...DEFAULT_FILTERS,
      statuses: excludeGroup("ongoing"),
    });
    expect(result).toHaveLength(1);
    expect(result[0].externalId).toBe("2");
  });

  it("includes by genre (OR logic)", () => {
    const result = applyFilters(sampleRecs, {
      ...DEFAULT_FILTERS,
      genres: includeGroup("Romance"),
    });
    expect(result).toHaveLength(1);
    expect(result[0].externalId).toBe("3");
  });

  it("excludes by genre", () => {
    const result = applyFilters(sampleRecs, {
      ...DEFAULT_FILTERS,
      genres: excludeGroup("Action"),
    });
    expect(result).toHaveLength(1);
    expect(result[0].externalId).toBe("3");
  });

  it("includes by tag (OR logic)", () => {
    const result = applyFilters(sampleRecs, {
      ...DEFAULT_FILTERS,
      tags: includeGroup("Overpowered"),
    });
    expect(result).toHaveLength(1);
    expect(result[0].externalId).toBe("2");
  });

  it("excludes by tag", () => {
    const result = applyFilters(sampleRecs, {
      ...DEFAULT_FILTERS,
      tags: excludeGroup("Revenge"),
    });
    // Only Light Novel X has no "Revenge" tag
    expect(result).toHaveLength(1);
    expect(result[0].externalId).toBe("3");
  });

  it("includes by format", () => {
    const result = applyFilters(sampleRecs, {
      ...DEFAULT_FILTERS,
      formats: includeGroup("NOVEL"),
    });
    expect(result).toHaveLength(1);
    expect(result[0].externalId).toBe("3");
  });

  it("excludes by format", () => {
    const result = applyFilters(sampleRecs, {
      ...DEFAULT_FILTERS,
      formats: excludeGroup("NOVEL"),
    });
    expect(result).toHaveLength(2);
    expect(result.map((r) => r.externalId)).toEqual(["1", "2"]);
  });

  it("includes by country", () => {
    const result = applyFilters(sampleRecs, {
      ...DEFAULT_FILTERS,
      countries: includeGroup("KR"),
    });
    expect(result).toHaveLength(1);
    expect(result[0].externalId).toBe("2");
  });

  it("excludes by country", () => {
    const result = applyFilters(sampleRecs, {
      ...DEFAULT_FILTERS,
      countries: excludeGroup("KR"),
    });
    expect(result).toHaveLength(2);
    expect(result.map((r) => r.externalId)).toEqual(["1", "3"]);
  });

  it("includes by seed (basedOn)", () => {
    const result = applyFilters(sampleRecs, {
      ...DEFAULT_FILTERS,
      seeds: includeGroup("Tower of God"),
    });
    expect(result).toHaveLength(1);
    expect(result[0].externalId).toBe("2");
  });

  it("excludes by seed (basedOn)", () => {
    const result = applyFilters(sampleRecs, {
      ...DEFAULT_FILTERS,
      seeds: excludeGroup("Vinland Saga"),
    });
    expect(result).toHaveLength(1);
    expect(result[0].externalId).toBe("2");
  });

  it("filters by score range", () => {
    const result = applyFilters(sampleRecs, {
      ...DEFAULT_FILTERS,
      scoreRange: [50, 100],
    });
    expect(result).toHaveLength(2);
    expect(result.map((r) => r.externalId)).toEqual(["1", "2"]);
  });

  it("combines include and exclude in same dimension", () => {
    const result = applyFilters(sampleRecs, {
      ...DEFAULT_FILTERS,
      genres: mixedGroup(["Action"], ["Dark Fantasy"]),
    });
    // Solo Leveling has Action but not Dark Fantasy
    expect(result).toHaveLength(1);
    expect(result[0].externalId).toBe("2");
  });

  it("combines multiple filter dimensions", () => {
    const result = applyFilters(sampleRecs, {
      ...DEFAULT_FILTERS,
      countries: includeGroup("JP"),
      formats: includeGroup("MANGA"),
    });
    expect(result).toHaveLength(1);
    expect(result[0].externalId).toBe("1");
  });

  it("returns empty when nothing matches", () => {
    const result = applyFilters(sampleRecs, {
      ...DEFAULT_FILTERS,
      countries: includeGroup("CN"),
    });
    expect(result).toHaveLength(0);
  });
});

// =============================================================================
// activeFilterCount Tests
// =============================================================================

describe("activeFilterCount", () => {
  it("returns 0 for default filters", () => {
    expect(activeFilterCount({ ...DEFAULT_FILTERS })).toBe(0);
  });

  it("counts each active dimension", () => {
    expect(
      activeFilterCount({
        ...DEFAULT_FILTERS,
        statuses: includeGroup("ongoing"),
        genres: excludeGroup("Action"),
        scoreRange: [20, 100],
      }),
    ).toBe(3);
  });

  it("counts score range as active when min > 0", () => {
    expect(
      activeFilterCount({
        ...DEFAULT_FILTERS,
        scoreRange: [10, 100],
      }),
    ).toBe(1);
  });

  it("counts score range as active when max < 100", () => {
    expect(
      activeFilterCount({
        ...DEFAULT_FILTERS,
        scoreRange: [0, 90],
      }),
    ).toBe(1);
  });
});

// =============================================================================
// Component Tests
// =============================================================================

describe("RecommendationFilters component", () => {
  const defaultProps = {
    recommendations: sampleRecs,
    filters: { ...DEFAULT_FILTERS } as RecommendationFilterState,
    onChange: vi.fn(),
  };

  it("renders the filter toggle button", () => {
    renderWithProviders(<RecommendationFilters {...defaultProps} />);
    expect(screen.getByText("Filters")).toBeInTheDocument();
  });

  it("shows active filter count badge", () => {
    renderWithProviders(
      <RecommendationFilters
        {...defaultProps}
        filters={{ ...DEFAULT_FILTERS, statuses: includeGroup("ongoing") }}
      />,
    );
    expect(screen.getByText("1")).toBeInTheDocument();
  });

  it("expands filter panel on click", async () => {
    const user = userEvent.setup();
    renderWithProviders(<RecommendationFilters {...defaultProps} />);

    await user.click(screen.getByTestId("filter-toggle"));

    expect(screen.getByText("Match Score")).toBeInTheDocument();
    expect(screen.getByText("Origin")).toBeInTheDocument();
    expect(screen.getByText("Format")).toBeInTheDocument();
    expect(screen.getByText("Status")).toBeInTheDocument();
    expect(screen.getByText("Genres")).toBeInTheDocument();
    expect(screen.getByText("Based On")).toBeInTheDocument();
  });

  it("shows clear all when filters are active", () => {
    renderWithProviders(
      <RecommendationFilters
        {...defaultProps}
        filters={{ ...DEFAULT_FILTERS, statuses: includeGroup("ongoing") }}
      />,
    );
    expect(screen.getByText("Clear all")).toBeInTheDocument();
  });

  it("calls onChange with cleared filters on clear all", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(
      <RecommendationFilters
        {...defaultProps}
        filters={{ ...DEFAULT_FILTERS, statuses: includeGroup("ongoing") }}
        onChange={onChange}
      />,
    );

    await user.click(screen.getByTestId("filter-clear"));

    const call = onChange.mock.calls[0][0] as RecommendationFilterState;
    expect(call.statuses.values.size).toBe(0);
    expect(call.genres.values.size).toBe(0);
    expect(call.tags.values.size).toBe(0);
    expect(call.formats.values.size).toBe(0);
    expect(call.countries.values.size).toBe(0);
    expect(call.seeds.values.size).toBe(0);
  });
});
