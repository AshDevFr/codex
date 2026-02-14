import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, waitFor } from "@/test/utils";
import { SeriesMetadataEditModal } from "./SeriesMetadataEditModal";

// Mock the API module
vi.mock("@/api/seriesMetadata", () => ({
  seriesMetadataApi: {
    getFullMetadata: vi.fn(),
    patchMetadata: vi.fn(),
    updateLocks: vi.fn(),
    uploadCover: vi.fn(),
    createAlternateTitle: vi.fn(),
    updateAlternateTitle: vi.fn(),
    deleteAlternateTitle: vi.fn(),
    createExternalLink: vi.fn(),
    deleteExternalLink: vi.fn(),
  },
}));

import { seriesMetadataApi } from "@/api/seriesMetadata";

const mockMetadata = {
  seriesId: "test-series-id",
  title: "Test Series",
  titleSort: "test series",
  summary: "A test summary",
  status: "ongoing",
  language: "en",
  readingDirection: "ltr",
  publisher: "Test Publisher",
  imprint: null,
  year: 2024,
  ageRating: null,
  genres: [{ id: "g1", name: "Action" }],
  tags: [{ id: "t1", name: "Superhero" }],
  alternateTitles: [],
  externalLinks: [],
  externalRatings: [],
  locks: {
    title: false,
    titleSort: false,
    summary: false,
    status: false,
    language: false,
    readingDirection: false,
    publisher: false,
    imprint: false,
    year: false,
    ageRating: false,
    genres: false,
    tags: false,
  },
  createdAt: "2024-01-01T00:00:00Z",
  updatedAt: "2024-01-01T00:00:00Z",
};

describe("SeriesMetadataEditModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (
      seriesMetadataApi.getFullMetadata as ReturnType<typeof vi.fn>
    ).mockResolvedValue(mockMetadata);
    (
      seriesMetadataApi.patchMetadata as ReturnType<typeof vi.fn>
    ).mockResolvedValue({});
    (
      seriesMetadataApi.updateLocks as ReturnType<typeof vi.fn>
    ).mockResolvedValue({});
  });

  it("renders modal with title", async () => {
    renderWithProviders(
      <SeriesMetadataEditModal
        opened={true}
        onClose={vi.fn()}
        seriesId="test-series-id"
        seriesTitle="Test Series"
      />,
    );

    expect(screen.getByText(/Edit Test Series/)).toBeInTheDocument();
  });

  it("shows loading state initially", () => {
    renderWithProviders(
      <SeriesMetadataEditModal
        opened={true}
        onClose={vi.fn()}
        seriesId="test-series-id"
      />,
    );

    // Should show loader while fetching (Mantine loader uses a span)
    const loader = document.querySelector(".mantine-Loader-root");
    expect(loader).toBeInTheDocument();
  });

  it("loads and displays metadata", async () => {
    renderWithProviders(
      <SeriesMetadataEditModal
        opened={true}
        onClose={vi.fn()}
        seriesId="test-series-id"
      />,
    );

    await waitFor(() => {
      expect(seriesMetadataApi.getFullMetadata).toHaveBeenCalledWith(
        "test-series-id",
      );
    });

    // Wait for form to load
    await waitFor(() => {
      expect(screen.getByDisplayValue("Test Series")).toBeInTheDocument();
    });
  });

  it("shows all tabs", async () => {
    renderWithProviders(
      <SeriesMetadataEditModal
        opened={true}
        onClose={vi.fn()}
        seriesId="test-series-id"
      />,
    );

    await waitFor(() => {
      expect(screen.getByRole("tab", { name: /General/i })).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: /Titles/i })).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: /Tags/i })).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: /Links/i })).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: /Cover/i })).toBeInTheDocument();
    });
  });

  it("calls onClose when cancel is clicked", async () => {
    const onClose = vi.fn();

    renderWithProviders(
      <SeriesMetadataEditModal
        opened={true}
        onClose={onClose}
        seriesId="test-series-id"
      />,
    );

    await waitFor(() => {
      expect(screen.getByDisplayValue("Test Series")).toBeInTheDocument();
    });

    const cancelButton = screen.getByRole("button", { name: /Cancel/i });
    cancelButton.click();

    expect(onClose).toHaveBeenCalled();
  });

  it("does not fetch when closed", () => {
    renderWithProviders(
      <SeriesMetadataEditModal
        opened={false}
        onClose={vi.fn()}
        seriesId="test-series-id"
      />,
    );

    expect(seriesMetadataApi.getFullMetadata).not.toHaveBeenCalled();
  });
});
