import { beforeEach, describe, expect, it, vi } from "vitest";
import { ratingsApi } from "@/api/ratings";
import { renderWithProviders, screen, waitFor } from "@/test/utils";
import { CommunityRating } from "./CommunityRating";

// Mock the API
vi.mock("@/api/ratings", () => ({
  ratingsApi: {
    getSeriesAverageRating: vi.fn(),
    getUserRating: vi.fn(),
    setUserRating: vi.fn(),
    deleteUserRating: vi.fn(),
    getAllUserRatings: vi.fn(),
  },
  storageToDisplayRating: (rating: number) => rating / 10,
  displayToStorageRating: (rating: number) => rating * 10,
}));

const mockGetSeriesAverageRating = vi.mocked(ratingsApi.getSeriesAverageRating);

describe("CommunityRating", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("displays community average rating with count", async () => {
    mockGetSeriesAverageRating.mockResolvedValue({
      average: 85,
      count: 25,
    });

    renderWithProviders(<CommunityRating seriesId="test-series-1" />);

    await waitFor(() => {
      expect(screen.getByText("Community: 8.5")).toBeInTheDocument();
    });

    expect(screen.getByText("(25)")).toBeInTheDocument();
  });

  it("renders nothing when no community ratings exist", async () => {
    mockGetSeriesAverageRating.mockResolvedValue({
      average: null,
      count: 0,
    });

    renderWithProviders(<CommunityRating seriesId="test-series-no-ratings" />);

    await waitFor(() => {
      // Wait for query to complete
      expect(mockGetSeriesAverageRating).toHaveBeenCalledWith(
        "test-series-no-ratings",
      );
    });

    // Component should not display the Community rating text
    expect(screen.queryByText(/Community:/)).not.toBeInTheDocument();
  });

  it("renders nothing when count is zero", async () => {
    mockGetSeriesAverageRating.mockResolvedValue({
      average: null,
      count: 0,
    });

    renderWithProviders(<CommunityRating seriesId="test-series-zero" />);

    await waitFor(() => {
      expect(mockGetSeriesAverageRating).toHaveBeenCalled();
    });

    // Component should not display the Community rating text
    expect(screen.queryByText(/Community:/)).not.toBeInTheDocument();
  });

  it("handles single user rating correctly", async () => {
    mockGetSeriesAverageRating.mockResolvedValue({
      average: 90,
      count: 1,
    });

    renderWithProviders(<CommunityRating seriesId="test-series-single" />);

    await waitFor(() => {
      expect(screen.getByText("Community: 9.0")).toBeInTheDocument();
    });

    expect(screen.getByText("(1)")).toBeInTheDocument();
  });

  it("displays decimal ratings correctly", async () => {
    mockGetSeriesAverageRating.mockResolvedValue({
      average: 78.5,
      count: 15,
    });

    renderWithProviders(<CommunityRating seriesId="test-series-decimal" />);

    await waitFor(() => {
      // 78.5 / 10 = 7.85, displayed as 7.8 (toFixed(1) rounds down)
      expect(screen.getByText("Community: 7.8")).toBeInTheDocument();
    });
  });

  it("calls API with correct series ID", async () => {
    mockGetSeriesAverageRating.mockResolvedValue({
      average: 75,
      count: 10,
    });

    renderWithProviders(<CommunityRating seriesId="specific-series-id-123" />);

    await waitFor(() => {
      expect(mockGetSeriesAverageRating).toHaveBeenCalledWith(
        "specific-series-id-123",
      );
    });
  });
});
