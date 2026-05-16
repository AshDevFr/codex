import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { MobileSearchSheet } from "./MobileSearchSheet";

vi.mock("@/hooks/useSearch", () => ({
  useSearch: vi.fn(),
}));

const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return {
    ...actual,
    useNavigate: () => mockNavigate,
  };
});

import { useSearch } from "@/hooks/useSearch";

const mockResults = {
  series: [
    {
      id: "s1",
      title: "Alpha Series",
      bookCount: 3,
      createdAt: "2024-01-01T00:00:00Z",
      libraryId: "lib-1",
      libraryName: "Comics",
      updatedAt: "2024-01-01T00:00:00Z",
    },
  ],
  books: [
    {
      id: "b1",
      title: "First Book",
      libraryId: "lib-1",
      libraryName: "Comics",
      seriesName: "Gamma Series",
      seriesId: "s1",
      filePath: "/path/first.cbz",
      fileSize: 1000,
      fileHash: "hash1",
      fileFormat: "cbz",
      pageCount: 100,
      analyzed: true,
      createdAt: "2024-01-01T00:00:00Z",
      updatedAt: "2024-01-01T00:00:00Z",
      deleted: false,
    },
  ],
};

describe("MobileSearchSheet", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(useSearch).mockReturnValue({
      results: { series: [], books: [] },
      isLoading: false,
      error: null,
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("does not render input when closed", () => {
    renderWithProviders(<MobileSearchSheet opened={false} onClose={vi.fn()} />);
    expect(
      screen.queryByPlaceholderText("Search series and books..."),
    ).not.toBeInTheDocument();
  });

  it("renders input when opened", () => {
    renderWithProviders(<MobileSearchSheet opened={true} onClose={vi.fn()} />);
    expect(
      screen.getByPlaceholderText("Search series and books..."),
    ).toBeInTheDocument();
  });

  it("does not render result groups when query is below the minimum length", async () => {
    vi.mocked(useSearch).mockReturnValue({
      results: mockResults,
      isLoading: false,
      error: null,
    });
    const user = userEvent.setup();
    renderWithProviders(<MobileSearchSheet opened={true} onClose={vi.fn()} />);

    const input = screen.getByPlaceholderText("Search series and books...");
    await user.type(input, "t");

    expect(screen.queryByText("Alpha Series")).not.toBeInTheDocument();
  });

  it("shows series and book results when query length is at least 2", async () => {
    vi.mocked(useSearch).mockReturnValue({
      results: mockResults,
      isLoading: false,
      error: null,
    });
    const user = userEvent.setup();
    renderWithProviders(<MobileSearchSheet opened={true} onClose={vi.fn()} />);

    const input = screen.getByPlaceholderText("Search series and books...");
    await user.type(input, "te");

    await waitFor(() => {
      expect(screen.getByText("Alpha Series")).toBeInTheDocument();
      expect(screen.getByText("First Book")).toBeInTheDocument();
    });
  });

  it("navigates and closes when a series result is clicked", async () => {
    vi.mocked(useSearch).mockReturnValue({
      results: mockResults,
      isLoading: false,
      error: null,
    });
    const onClose = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(<MobileSearchSheet opened={true} onClose={onClose} />);

    const input = screen.getByPlaceholderText("Search series and books...");
    await user.type(input, "alpha");

    await waitFor(() => {
      expect(screen.getByText("Alpha Series")).toBeInTheDocument();
    });

    await user.click(screen.getByText("Alpha Series"));

    expect(mockNavigate).toHaveBeenCalledWith("/series/s1");
    expect(onClose).toHaveBeenCalled();
  });

  it("navigates and closes when a book result is clicked", async () => {
    vi.mocked(useSearch).mockReturnValue({
      results: mockResults,
      isLoading: false,
      error: null,
    });
    const onClose = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(<MobileSearchSheet opened={true} onClose={onClose} />);

    const input = screen.getByPlaceholderText("Search series and books...");
    await user.type(input, "book");

    await waitFor(() => {
      expect(screen.getByText("First Book")).toBeInTheDocument();
    });

    await user.click(screen.getByText("First Book"));

    expect(mockNavigate).toHaveBeenCalledWith("/books/b1");
    expect(onClose).toHaveBeenCalled();
  });

  it("navigates to /search and closes on Enter when query is long enough", async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(<MobileSearchSheet opened={true} onClose={onClose} />);

    const input = screen.getByPlaceholderText("Search series and books...");
    await user.type(input, "hello");
    await user.keyboard("{Enter}");

    expect(mockNavigate).toHaveBeenCalledWith("/search?q=hello");
    expect(onClose).toHaveBeenCalled();
  });

  it("does not navigate on Enter when query is too short", async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(<MobileSearchSheet opened={true} onClose={onClose} />);

    const input = screen.getByPlaceholderText("Search series and books...");
    await user.type(input, "a");
    await user.keyboard("{Enter}");

    expect(mockNavigate).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("shows the loading state while searching", async () => {
    vi.mocked(useSearch).mockReturnValue({
      results: { series: [], books: [] },
      isLoading: true,
      error: null,
    });
    const user = userEvent.setup();
    renderWithProviders(<MobileSearchSheet opened={true} onClose={vi.fn()} />);

    const input = screen.getByPlaceholderText("Search series and books...");
    await user.type(input, "te");

    await waitFor(() => {
      expect(screen.getByText("Searching...")).toBeInTheDocument();
    });
  });

  it("shows a no-results message when the query has no matches", async () => {
    const user = userEvent.setup();
    renderWithProviders(<MobileSearchSheet opened={true} onClose={vi.fn()} />);

    const input = screen.getByPlaceholderText("Search series and books...");
    await user.type(input, "te");

    await waitFor(() => {
      expect(screen.getByText("No results found")).toBeInTheDocument();
    });
  });
});
