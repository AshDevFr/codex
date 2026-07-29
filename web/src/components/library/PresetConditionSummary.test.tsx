import { describe, expect, it } from "vitest";
import type { FilterPresetDto } from "@/api/filterPresets";
import { renderWithProviders, screen } from "@/test/utils";
import type { SeriesCondition } from "@/types/filters";
import { PresetConditionSummary } from "./PresetConditionSummary";

const LIB_A = "11111111-1111-1111-1111-111111111111";

function preset(condition: unknown, target = "series"): FilterPresetDto {
  return {
    id: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
    name: "Preset",
    scope: "library",
    target,
    libraryId: null,
    query: null,
    sort: null,
    condition: condition as FilterPresetDto["condition"],
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
  };
}

function render(condition: unknown) {
  renderWithProviders(<PresetConditionSummary preset={preset(condition)} />);
}

describe("PresetConditionSummary rating rows", () => {
  // The stored scale is 1-100; showing 85 to a user reading a 10-point scale
  // would be a factor-of-ten lie.
  it("renders a userRating threshold on the 1-10 scale", () => {
    render({ userRating: { operator: "gte", value: 85 } });
    expect(screen.getByText("My Rating")).toBeInTheDocument();
    expect(screen.getByText("≥ 8.5")).toBeInTheDocument();
    expect(screen.queryByText(/85/)).not.toBeInTheDocument();
  });

  it("renders a communityRating threshold on the 1-10 scale", () => {
    render({ communityRating: { operator: "gt", value: 78 } });
    expect(screen.getByText("Community Rating")).toBeInTheDocument();
    expect(screen.getByText("> 7.8")).toBeInTheDocument();
  });

  it("renders a closed range with both bounds converted", () => {
    render({ userRating: { operator: "between", min: 70, max: 90 } });
    expect(screen.getByText("7.0 to 9.0")).toBeInTheDocument();
  });

  it("renders an open-ended range as a single bound", () => {
    render({ userRating: { operator: "between", min: 85 } });
    expect(screen.getByText("≥ 8.5")).toBeInTheDocument();
  });

  it("describes the nullability operators in words", () => {
    render({ userRating: { operator: "isNull" } });
    expect(screen.getByText("not rated")).toBeInTheDocument();
  });

  // Rating filters can't be represented in the chip state. Before this split
  // they took the whole preset down to the bare "advanced filter" notice.
  it("shows the rating alongside chip-representable filters", () => {
    render({
      allOf: [
        { userRating: { operator: "gte", value: 85 } },
        { genre: { operator: "is", value: "Action" } },
      ],
    } satisfies SeriesCondition);

    expect(screen.getByText("≥ 8.5")).toBeInTheDocument();
    expect(screen.getByText("+ Action")).toBeInTheDocument();
    expect(screen.queryByText(/advanced filter/i)).not.toBeInTheDocument();
  });

  it("shows the rating plus a notice when the remainder is advanced", () => {
    render({
      allOf: [
        { userRating: { operator: "gte", value: 85 } },
        {
          anyOf: [
            { genre: { operator: "is", value: "Action" } },
            { year: { operator: "gte", value: 2015 } },
          ],
        },
      ],
    });

    expect(screen.getByText("≥ 8.5")).toBeInTheDocument();
    expect(screen.getByText(/advanced filter/i)).toBeInTheDocument();
  });

  it("does not claim the preset is empty when it only holds a rating", () => {
    render({ userRating: { operator: "gte", value: 85 } });
    expect(screen.queryByText(/no filters in this preset/i)).toBeNull();
  });
});

describe("PresetConditionSummary libraries row", () => {
  it("renders included libraries from an `in` condition", () => {
    render({ libraryId: { operator: "in", values: [LIB_A] } });
    expect(screen.getByText("Libraries")).toBeInTheDocument();
  });

  // Library values are UUIDs. Until names resolve, show a short prefix rather
  // than a 36-character blob in a chip.
  it("shortens an unresolved library UUID", () => {
    render({ libraryId: { operator: "in", values: [LIB_A] } });
    expect(screen.getByText(`+ ${LIB_A.slice(0, 8)}…`)).toBeInTheDocument();
  });

  it("renders excluded libraries from a `notIn` condition", () => {
    render({ libraryId: { operator: "notIn", values: [LIB_A] } });
    expect(screen.getByText(`− ${LIB_A.slice(0, 8)}…`)).toBeInTheDocument();
  });
});

describe("PresetConditionSummary existing behaviour", () => {
  it("still reports an empty preset", () => {
    render({ allOf: [] });
    expect(screen.getByText(/no filters in this preset/i)).toBeInTheDocument();
  });

  it("still renders plain chip groups", () => {
    render({
      allOf: [
        { genre: { operator: "is", value: "Action" } },
        { tag: { operator: "isNot", value: "Ecchi" } },
      ],
    } satisfies SeriesCondition);
    expect(screen.getByText("+ Action")).toBeInTheDocument();
    expect(screen.getByText("− Ecchi")).toBeInTheDocument();
  });
});
