import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PluginActionDto } from "@/api/plugins";
import { renderWithProviders, screen, waitFor } from "@/test/utils";
import { MetadataSearchModal } from "./MetadataSearchModal";

// Mock the plugins API
vi.mock("@/api/plugins", () => ({
  pluginsApi: {
    searchMetadata: vi.fn(),
  },
}));

import { pluginsApi } from "@/api/plugins";

const mockPlugin: PluginActionDto = {
  pluginId: "test-plugin-id",
  pluginName: "test-plugin",
  pluginDisplayName: "Test Plugin",
  actionType: "metadata_search",
  label: "Search Test Plugin",
};

const mockSearchResults = {
  success: true,
  result: {
    results: [
      {
        externalId: "ext-1",
        title: "Test Series",
        alternateTitles: ["Alt Title 1"],
        year: 2024,
        coverUrl: "https://example.com/cover.jpg",
        relevanceScore: 0.95,
        preview: {
          status: "Ongoing",
          genres: ["Action", "Adventure"],
        },
      },
      {
        externalId: "ext-2",
        title: "Another Series",
        alternateTitles: [],
        year: 2023,
        coverUrl: null,
        relevanceScore: 0.8,
        preview: null,
      },
    ],
  },
  latencyMs: 150,
};

describe("MetadataSearchModal", () => {
  const onClose = vi.fn();
  const onSelect = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    (pluginsApi.searchMetadata as ReturnType<typeof vi.fn>).mockResolvedValue(
      mockSearchResults,
    );
  });

  it("renders modal with plugin name in title", () => {
    renderWithProviders(
      <MetadataSearchModal
        opened={true}
        onClose={onClose}
        plugin={mockPlugin}
        onSelect={onSelect}
      />,
    );

    expect(screen.getByText("Search Test Plugin")).toBeInTheDocument();
  });

  it("shows initial query in search input", () => {
    renderWithProviders(
      <MetadataSearchModal
        opened={true}
        onClose={onClose}
        plugin={mockPlugin}
        initialQuery="Initial Search"
        onSelect={onSelect}
      />,
    );

    expect(screen.getByDisplayValue("Initial Search")).toBeInTheDocument();
  });

  it("shows hint when query is too short", () => {
    renderWithProviders(
      <MetadataSearchModal
        opened={true}
        onClose={onClose}
        plugin={mockPlugin}
        initialQuery="a"
        onSelect={onSelect}
      />,
    );

    expect(
      screen.getByText(
        "Enter at least 2 characters to search, or paste an external ID",
      ),
    ).toBeInTheDocument();
  });

  it("shows search results after searching", async () => {
    renderWithProviders(
      <MetadataSearchModal
        opened={true}
        onClose={onClose}
        plugin={mockPlugin}
        initialQuery="Test"
        onSelect={onSelect}
      />,
    );

    // Wait for debounced search and results
    await waitFor(
      () => {
        expect(screen.getByText("Test Series")).toBeInTheDocument();
      },
      { timeout: 1000 },
    );

    expect(screen.getByText("Another Series")).toBeInTheDocument();
    expect(screen.getByText("2024")).toBeInTheDocument();
    expect(screen.getByText("Ongoing")).toBeInTheDocument();
  });

  it("displays result count", async () => {
    renderWithProviders(
      <MetadataSearchModal
        opened={true}
        onClose={onClose}
        plugin={mockPlugin}
        initialQuery="Test"
        onSelect={onSelect}
      />,
    );

    await waitFor(
      () => {
        expect(screen.getByText("2 results found")).toBeInTheDocument();
      },
      { timeout: 1000 },
    );
  });

  it("shows no results message when search returns empty", async () => {
    (pluginsApi.searchMetadata as ReturnType<typeof vi.fn>).mockResolvedValue({
      success: true,
      result: { results: [] },
      latencyMs: 100,
    });

    renderWithProviders(
      <MetadataSearchModal
        opened={true}
        onClose={onClose}
        plugin={mockPlugin}
        initialQuery="NonExistent"
        onSelect={onSelect}
      />,
    );

    await waitFor(
      () => {
        expect(
          screen.getByText(/No results found for "NonExistent"/),
        ).toBeInTheDocument();
      },
      { timeout: 1000 },
    );
  });

  it("shows error message when search fails", async () => {
    (pluginsApi.searchMetadata as ReturnType<typeof vi.fn>).mockResolvedValue({
      success: false,
      error: "Network error",
      latencyMs: 100,
    });

    renderWithProviders(
      <MetadataSearchModal
        opened={true}
        onClose={onClose}
        plugin={mockPlugin}
        initialQuery="Test"
        onSelect={onSelect}
      />,
    );

    await waitFor(
      () => {
        expect(screen.getByText("Network error")).toBeInTheDocument();
      },
      { timeout: 1000 },
    );

    expect(screen.getByText("Retry")).toBeInTheDocument();
  });

  it("does not render when closed", () => {
    renderWithProviders(
      <MetadataSearchModal
        opened={false}
        onClose={onClose}
        plugin={mockPlugin}
        onSelect={onSelect}
      />,
    );

    expect(screen.queryByText("Search Test Plugin")).not.toBeInTheDocument();
  });

  it("displays book count when provided in preview", async () => {
    (pluginsApi.searchMetadata as ReturnType<typeof vi.fn>).mockResolvedValue({
      success: true,
      result: {
        results: [
          {
            externalId: "ext-1",
            title: "Series With Books",
            alternateTitles: [],
            year: 2024,
            coverUrl: null,
            preview: {
              status: "Completed",
              bookCount: 10,
              genres: ["Fantasy"],
            },
          },
          {
            externalId: "ext-2",
            title: "Single Book Series",
            alternateTitles: [],
            year: 2023,
            coverUrl: null,
            preview: {
              bookCount: 1,
              genres: [],
            },
          },
        ],
      },
      latencyMs: 100,
    });

    renderWithProviders(
      <MetadataSearchModal
        opened={true}
        onClose={onClose}
        plugin={mockPlugin}
        initialQuery="Series"
        onSelect={onSelect}
      />,
    );

    await waitFor(
      () => {
        expect(screen.getByText("10 books")).toBeInTheDocument();
      },
      { timeout: 1000 },
    );

    // Singular form for 1 book
    expect(screen.getByText("1 book")).toBeInTheDocument();
  });
});
