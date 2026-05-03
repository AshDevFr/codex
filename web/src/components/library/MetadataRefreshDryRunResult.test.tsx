import { describe, expect, it, vi } from "vitest";
import type { DryRunResponse } from "@/api/metadataRefresh";
import { renderWithProviders, screen } from "@/test/utils";
import { MetadataRefreshDryRunResult } from "./MetadataRefreshDryRunResult";

function makeResponse(overrides: Partial<DryRunResponse> = {}): DryRunResponse {
  return {
    sample: [],
    totalEligible: 0,
    estSkippedNoId: 0,
    estSkippedRecentlySynced: 0,
    ...overrides,
  };
}

describe("MetadataRefreshDryRunResult", () => {
  it("renders loading state", () => {
    renderWithProviders(
      <MetadataRefreshDryRunResult
        opened
        onClose={vi.fn()}
        result={null}
        loading
      />,
    );
    expect(screen.getByText(/Computing preview/i)).toBeInTheDocument();
  });

  it("renders empty sample message when nothing eligible", () => {
    renderWithProviders(
      <MetadataRefreshDryRunResult
        opened
        onClose={vi.fn()}
        result={makeResponse()}
      />,
    );
    expect(
      screen.getByText(/No series eligible for refresh/i),
    ).toBeInTheDocument();
  });

  it("renders changes and skipped fields", () => {
    const result = makeResponse({
      totalEligible: 12,
      estSkippedNoId: 3,
      estSkippedRecentlySynced: 1,
      sample: [
        {
          seriesId: "11111111-1111-1111-1111-111111111111",
          seriesTitle: "Test Series",
          provider: "plugin:mangabaka",
          changes: [
            { field: "rating", before: 80, after: 82 },
            { field: "status", before: "ongoing", after: "completed" },
          ],
          skipped: [{ field: "summary", reason: "field locked" }],
        },
      ],
    });
    renderWithProviders(
      <MetadataRefreshDryRunResult opened onClose={vi.fn()} result={result} />,
    );

    expect(screen.getByText("Test Series")).toBeInTheDocument();
    expect(screen.getByText("plugin:mangabaka")).toBeInTheDocument();
    expect(screen.getByText("rating")).toBeInTheDocument();
    expect(screen.getByText("80")).toBeInTheDocument();
    expect(screen.getByText("82")).toBeInTheDocument();
    expect(screen.getByText("status")).toBeInTheDocument();
    expect(screen.getByText("summary")).toBeInTheDocument();
    expect(screen.getByText(/field locked/i)).toBeInTheDocument();
    expect(screen.getByText("12")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByText("1")).toBeInTheDocument();
  });

  it("warns about unresolved providers", () => {
    const result = makeResponse({
      totalEligible: 0,
      unresolvedProviders: ["plugin:typo"],
    });
    renderWithProviders(
      <MetadataRefreshDryRunResult opened onClose={vi.fn()} result={result} />,
    );
    expect(screen.getByText(/Unresolved providers/i)).toBeInTheDocument();
    expect(screen.getByText("plugin:typo")).toBeInTheDocument();
  });
});
