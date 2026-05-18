import { describe, expect, it } from "vitest";
import { renderWithProviders, screen } from "@/test/utils";
import { SeriesDetailSkeleton } from "./SeriesDetailSkeleton";

describe("SeriesDetailSkeleton", () => {
  it("renders the breadcrumbs, header, metadata rows, and books grid placeholders", () => {
    renderWithProviders(<SeriesDetailSkeleton />);

    expect(screen.getByTestId("series-detail-skeleton")).toBeInTheDocument();
    // Books grid placeholder still uses the shared CoverGridSkeleton.
    expect(
      screen.getByTestId("series-detail-books-skeleton"),
    ).toBeInTheDocument();
  });
});
