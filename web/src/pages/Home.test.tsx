import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders } from "@/test/utils";
import { Home } from "./Home";

// Mock the RecommendedSection component
vi.mock("@/components/library/RecommendedSection", () => ({
  RecommendedSection: ({ libraryId }: { libraryId: string }) => (
    <div data-testid="recommended-section">Recommended: {libraryId}</div>
  ),
}));

describe("Home Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  it("should render the Home page title", async () => {
    renderWithProviders(<Home />);

    await waitFor(() => {
      expect(screen.getByText("Home")).toBeInTheDocument();
    });
  });

  it("should render RecommendedSection with 'all' libraryId", async () => {
    renderWithProviders(<Home />);

    await waitFor(() => {
      const recommendedSection = screen.getByTestId("recommended-section");
      expect(recommendedSection).toBeInTheDocument();
      expect(recommendedSection).toHaveTextContent("Recommended: all");
    });
  });
});
