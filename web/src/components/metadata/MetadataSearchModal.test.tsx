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

  it("shows 'Search on' button when plugin has searchUriTemplate and query is valid", () => {
    const pluginWithTemplate: PluginActionDto = {
      ...mockPlugin,
      searchUriTemplate: "https://mangabaka.org/search?q=<title>",
    };

    renderWithProviders(
      <MetadataSearchModal
        opened={true}
        onClose={onClose}
        plugin={pluginWithTemplate}
        initialQuery="One Piece"
        onSelect={onSelect}
      />,
    );

    const link = screen.getByRole("link", {
      name: /Search on Test Plugin/,
    });
    expect(link).toBeInTheDocument();
    expect(link).toHaveAttribute(
      "href",
      "https://mangabaka.org/search?q=One%20Piece",
    );
    expect(link).toHaveAttribute("target", "_blank");
    expect(link).toHaveAttribute("rel", "noopener noreferrer");
  });

  it("does not show 'Search on' button when plugin has no searchUriTemplate", () => {
    renderWithProviders(
      <MetadataSearchModal
        opened={true}
        onClose={onClose}
        plugin={mockPlugin}
        initialQuery="One Piece"
        onSelect={onSelect}
      />,
    );

    expect(
      screen.queryByRole("link", { name: /Search on/ }),
    ).not.toBeInTheDocument();
  });

  it("does not show 'Search on' button when query is too short", () => {
    const pluginWithTemplate: PluginActionDto = {
      ...mockPlugin,
      searchUriTemplate: "https://mangabaka.org/search?q=<title>",
    };

    renderWithProviders(
      <MetadataSearchModal
        opened={true}
        onClose={onClose}
        plugin={pluginWithTemplate}
        initialQuery="a"
        onSelect={onSelect}
      />,
    );

    expect(
      screen.queryByRole("link", { name: /Search on/ }),
    ).not.toBeInTheDocument();
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

  it("renders distinct format badges for manga vs novel results", async () => {
    (pluginsApi.searchMetadata as ReturnType<typeof vi.fn>).mockResolvedValue({
      success: true,
      result: {
        results: [
          {
            externalId: "ext-manga",
            title: "A Wild Last Boss Appeared!",
            alternateTitles: [],
            year: 2017,
            coverUrl: null,
            preview: {
              status: "Releasing",
              genres: [],
              format: "manga",
            },
          },
          {
            externalId: "ext-novel",
            title: "A Wild Last Boss Appeared!",
            alternateTitles: [],
            year: 2016,
            coverUrl: null,
            preview: {
              status: "Releasing",
              genres: [],
              format: "novel",
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
        initialQuery="A Wild Last Boss"
        onSelect={onSelect}
      />,
    );

    await waitFor(
      () => {
        expect(screen.getByText("Manga")).toBeInTheDocument();
      },
      { timeout: 1000 },
    );

    const mangaBadge = screen.getByText("Manga");
    const novelBadge = screen.getByText("Novel");

    expect(mangaBadge).toBeInTheDocument();
    expect(novelBadge).toBeInTheDocument();

    // Mantine Badge sets data-variant + the resolved color via CSS variables on
    // the root element. The badge text node lives inside a label span; walk up
    // to find the styled root.
    const mangaRoot = mangaBadge.closest("[data-variant]");
    const novelRoot = novelBadge.closest("[data-variant]");

    expect(mangaRoot).not.toBeNull();
    expect(novelRoot).not.toBeNull();
    // Distinct colors → distinct inline style for the Mantine color variable.
    expect(mangaRoot?.getAttribute("style")).not.toBe(
      novelRoot?.getAttribute("style"),
    );
  });

  it("renders a fallback gray badge for unknown format values", async () => {
    (pluginsApi.searchMetadata as ReturnType<typeof vi.fn>).mockResolvedValue({
      success: true,
      result: {
        results: [
          {
            externalId: "ext-oel",
            title: "Original English",
            alternateTitles: [],
            year: 2020,
            coverUrl: null,
            preview: {
              genres: [],
              format: "oel",
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
        initialQuery="Original"
        onSelect={onSelect}
      />,
    );

    await waitFor(
      () => {
        expect(screen.getByText("Oel")).toBeInTheDocument();
      },
      { timeout: 1000 },
    );
  });

  it("omits the format badge when format is missing", async () => {
    renderWithProviders(
      <MetadataSearchModal
        opened={true}
        onClose={onClose}
        plugin={mockPlugin}
        initialQuery="Test"
        onSelect={onSelect}
      />,
    );

    // Use the existing default mock (no `format` set on either preview).
    await waitFor(
      () => {
        expect(screen.getByText("Test Series")).toBeInTheDocument();
      },
      { timeout: 1000 },
    );

    expect(screen.queryByText("Manga")).not.toBeInTheDocument();
    expect(screen.queryByText("Novel")).not.toBeInTheDocument();
  });

  it("displays description when provided in preview", async () => {
    (pluginsApi.searchMetadata as ReturnType<typeof vi.fn>).mockResolvedValue({
      success: true,
      result: {
        results: [
          {
            externalId: "ext-1",
            title: "Series With Description",
            alternateTitles: [],
            year: 2024,
            coverUrl: null,
            preview: {
              description: "A thrilling adventure through distant lands.",
              status: "Ongoing",
              genres: [],
            },
          },
          {
            externalId: "ext-2",
            title: "Series Without Description",
            alternateTitles: [],
            year: 2023,
            coverUrl: null,
            preview: {
              status: "Completed",
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
        expect(
          screen.getByText("A thrilling adventure through distant lands."),
        ).toBeInTheDocument();
      },
      { timeout: 1000 },
    );
  });
});
