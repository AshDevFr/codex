import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders } from "@/test/utils";
import { SearchResults } from "./SearchResults";

// Mock the search API
vi.mock("@/api/search", () => ({
  searchApi: {
    search: vi.fn(),
  },
}));

// Mock the branding API (used by useDocumentTitle -> useAppName)
vi.mock("@/hooks/useAppName", () => ({
  useAppName: () => "Codex",
  useBranding: () => ({ data: { applicationName: "Codex" } }),
  brandingQueryKey: ["settings", "branding"],
  DEFAULT_APP_NAME: "Codex",
}));

// Mock the HorizontalCarousel component
vi.mock("@/components/library/HorizontalCarousel", () => ({
  HorizontalCarousel: ({
    title,
    subtitle,
    children,
  }: {
    title: string;
    subtitle?: string;
    children: React.ReactNode;
  }) => (
    <div data-testid={`carousel-${title.toLowerCase()}`}>
      <h2>{title}</h2>
      {subtitle && <span>{subtitle}</span>}
      <div>{children}</div>
    </div>
  ),
}));

// Mock the MediaCard component
vi.mock("@/components/library/MediaCard", () => ({
  MediaCard: ({
    type,
    data,
  }: {
    type: "book" | "series";
    data: { id: string; name?: string; title?: string };
  }) => (
    <div data-testid={`media-card-${type}-${data.id}`}>
      {type === "series" ? data.name : data.title}
    </div>
  ),
}));

// Import searchApi after mocking
import { searchApi } from "@/api/search";

describe("SearchResults Page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("should set document title to 'Search' when query is empty", async () => {
    renderWithProviders(<SearchResults />, {
      initialEntries: ["/search"],
    });

    await waitFor(() => {
      expect(document.title).toBe("Search - Codex");
    });
  });

  it("should set document title with search query", async () => {
    vi.mocked(searchApi.search).mockResolvedValueOnce({
      series: [],
      books: [],
    });

    renderWithProviders(<SearchResults />, {
      initialEntries: ["/search?q=batman"],
    });

    await waitFor(() => {
      expect(document.title).toBe("Search: batman - Codex");
    });
  });

  it("should show message when query is empty", async () => {
    renderWithProviders(<SearchResults />, {
      initialEntries: ["/search"],
    });

    await waitFor(() => {
      expect(screen.getByText("Search")).toBeInTheDocument();
      expect(screen.getByText("Enter a search term")).toBeInTheDocument();
    });
  });

  it("should show message when query is too short", async () => {
    renderWithProviders(<SearchResults />, {
      initialEntries: ["/search?q=a"],
    });

    await waitFor(() => {
      expect(screen.getByText("Enter a search term")).toBeInTheDocument();
    });
  });

  it("should display search results title with query", async () => {
    vi.mocked(searchApi.search).mockResolvedValueOnce({
      series: [],
      books: [],
    });

    renderWithProviders(<SearchResults />, {
      initialEntries: ["/search?q=batman"],
    });

    await waitFor(() => {
      expect(
        screen.getByText('Search results for "batman"'),
      ).toBeInTheDocument();
    });
  });

  it("should show loading state while searching", async () => {
    // Make the search take a while
    vi.mocked(searchApi.search).mockImplementationOnce(
      () => new Promise(() => {}), // Never resolves
    );

    renderWithProviders(<SearchResults />, {
      initialEntries: ["/search?q=batman"],
    });

    await waitFor(() => {
      expect(screen.getByText("Searching...")).toBeInTheDocument();
    });
  });

  it("should display no results message when search returns empty", async () => {
    vi.mocked(searchApi.search).mockResolvedValueOnce({
      series: [],
      books: [],
    });

    renderWithProviders(<SearchResults />, {
      initialEntries: ["/search?q=nonexistent"],
    });

    await waitFor(() => {
      expect(screen.getByText("No results found")).toBeInTheDocument();
    });
  });

  it("should display series results in carousel", async () => {
    vi.mocked(searchApi.search).mockResolvedValueOnce({
      series: [
        {
          id: "series-1",
          title: "Batman",
          bookCount: 10,
          libraryId: "lib-1",
          libraryName: "Comics",
          path: "/path",
          createdAt: "2024-01-01",
          updatedAt: "2024-01-01",
        },
        {
          id: "series-2",
          title: "Batman Beyond",
          bookCount: 5,
          libraryId: "lib-1",
          libraryName: "Comics",
          path: "/path",
          createdAt: "2024-01-01",
          updatedAt: "2024-01-01",
        },
      ],
      books: [],
    });

    renderWithProviders(<SearchResults />, {
      initialEntries: ["/search?q=batman"],
    });

    await waitFor(() => {
      expect(screen.getByTestId("carousel-series")).toBeInTheDocument();
      expect(
        screen.getByTestId("media-card-series-series-1"),
      ).toBeInTheDocument();
      expect(
        screen.getByTestId("media-card-series-series-2"),
      ).toBeInTheDocument();
    });
  });

  it("should display book results in carousel", async () => {
    vi.mocked(searchApi.search).mockResolvedValueOnce({
      series: [],
      books: [
        {
          id: "book-1",
          title: "Batman Year One",
          libraryId: "lib-1",
          libraryName: "Comics",
          seriesName: "Batman",
          pageCount: 200,
          seriesId: "series-1",
          filePath: "/path",
          fileSize: 1000,
          fileHash: "hash",
          fileFormat: "cbz",
          analyzed: true,
          createdAt: "2024-01-01",
          updatedAt: "2024-01-01",
          deleted: false,
        },
      ],
    });

    renderWithProviders(<SearchResults />, {
      initialEntries: ["/search?q=batman"],
    });

    await waitFor(() => {
      expect(screen.getByTestId("carousel-books")).toBeInTheDocument();
      expect(screen.getByTestId("media-card-book-book-1")).toBeInTheDocument();
    });
  });

  it("should display both series and book carousels when both have results", async () => {
    vi.mocked(searchApi.search).mockResolvedValueOnce({
      series: [
        {
          id: "series-1",
          title: "Batman",
          bookCount: 10,
          libraryId: "lib-1",
          libraryName: "Comics",
          createdAt: "2024-01-01",
          updatedAt: "2024-01-01",
        },
      ],
      books: [
        {
          id: "book-1",
          title: "Batman Year One",
          libraryId: "lib-1",
          libraryName: "Comics",
          seriesName: "Batman",
          pageCount: 200,
          seriesId: "series-1",
          filePath: "/path",
          fileSize: 1000,
          fileHash: "hash",
          fileFormat: "cbz",
          analyzed: true,
          createdAt: "2024-01-01",
          updatedAt: "2024-01-01",
          deleted: false,
        },
      ],
    });

    renderWithProviders(<SearchResults />, {
      initialEntries: ["/search?q=batman"],
    });

    await waitFor(() => {
      expect(screen.getByTestId("carousel-series")).toBeInTheDocument();
      expect(screen.getByTestId("carousel-books")).toBeInTheDocument();
    });
  });

  it("should show result count in subtitle", async () => {
    vi.mocked(searchApi.search).mockResolvedValueOnce({
      series: [
        {
          id: "series-1",
          title: "Batman",
          bookCount: 10,
          libraryId: "lib-1",
          libraryName: "Comics",
          path: "/path",
          createdAt: "2024-01-01",
          updatedAt: "2024-01-01",
        },
        {
          id: "series-2",
          title: "Batman Beyond",
          bookCount: 5,
          libraryId: "lib-1",
          libraryName: "Comics",
          path: "/path",
          createdAt: "2024-01-01",
          updatedAt: "2024-01-01",
        },
      ],
      books: [],
    });

    renderWithProviders(<SearchResults />, {
      initialEntries: ["/search?q=batman"],
    });

    await waitFor(() => {
      expect(screen.getByText("2 results")).toBeInTheDocument();
    });
  });
});
