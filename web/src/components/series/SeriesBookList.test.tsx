import { beforeEach, describe, expect, it, vi } from "vitest";
import { seriesApi } from "@/api/series";
import { createBook } from "@/mocks/data/factories";
import { useBulkSelectionStore } from "@/store/bulkSelectionStore";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { SeriesBookList } from "./SeriesBookList";

// Mock the series API
vi.mock("@/api/series", () => ({
  seriesApi: {
    getBooks: vi.fn(),
  },
}));

const mockSeriesApi = vi.mocked(seriesApi);

describe("SeriesBookList", () => {
  const seriesId = "test-series-id";
  const seriesName = "Test Series";
  const bookCount = 5;

  const mockBooks = [
    createBook({ id: "book-1", title: "Book One", number: 1 }),
    createBook({ id: "book-2", title: "Book Two", number: 2 }),
    createBook({ id: "book-3", title: "Book Three", number: 3 }),
    createBook({ id: "book-4", title: "Book Four", number: 4 }),
    createBook({ id: "book-5", title: "Book Five", number: 5 }),
  ];

  beforeEach(() => {
    vi.clearAllMocks();
    // Reset the bulk selection store before each test
    useBulkSelectionStore.getState().clearSelection();
    // Default mock to return books
    mockSeriesApi.getBooks.mockResolvedValue(mockBooks);
  });

  const renderComponent = () => {
    return renderWithProviders(
      <SeriesBookList
        seriesId={seriesId}
        seriesName={seriesName}
        bookCount={bookCount}
      />,
    );
  };

  describe("loading state", () => {
    it("should render loading state initially", () => {
      // Never resolve to keep loading state
      mockSeriesApi.getBooks.mockReturnValue(new Promise(() => {}));

      const { container } = renderComponent();

      // Mantine Loader uses a span with class mantine-Loader-root
      expect(
        container.querySelector(".mantine-Loader-root"),
      ).toBeInTheDocument();
    });
  });

  describe("data display", () => {
    it("should display books after loading", async () => {
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText("1 - Book One")).toBeInTheDocument();
      });

      expect(screen.getByText("2 - Book Two")).toBeInTheDocument();
      expect(screen.getByText("3 - Book Three")).toBeInTheDocument();
    });

    it("should display book count in title", async () => {
      renderComponent();

      await waitFor(() => {
        expect(
          screen.getByRole("heading", { name: /Books \(5\)/i }),
        ).toBeInTheDocument();
      });
    });

    it("should display empty state when no books", async () => {
      mockSeriesApi.getBooks.mockResolvedValue([]);

      renderComponent();

      await waitFor(() => {
        expect(screen.getByText("No books in this series")).toBeInTheDocument();
      });
    });
  });

  describe("error state", () => {
    it("should display error message on API failure", async () => {
      mockSeriesApi.getBooks.mockRejectedValue(new Error("Failed to fetch"));

      renderComponent();

      await waitFor(() => {
        expect(screen.getByText("Failed to load books")).toBeInTheDocument();
      });
    });
  });

  describe("bulk selection", () => {
    it("should render MediaCards with selection props", async () => {
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText("1 - Book One")).toBeInTheDocument();
      });

      // Each book card should have a checkbox for selection
      const checkboxes = screen.getAllByRole("checkbox");
      expect(checkboxes.length).toBe(5);
    });

    it("should toggle selection when checkbox is clicked", async () => {
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText("1 - Book One")).toBeInTheDocument();
      });

      // Click on the first book's checkbox
      const checkboxes = screen.getAllByRole("checkbox");
      await user.click(checkboxes[0]);

      // Check that the selection store was updated
      const state = useBulkSelectionStore.getState();
      expect(state.selectedIds.has("book-1")).toBe(true);
      expect(state.selectionType).toBe("book");
      expect(state.isSelectionMode).toBe(true);
    });

    it("should allow selecting multiple books", async () => {
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText("1 - Book One")).toBeInTheDocument();
      });

      const checkboxes = screen.getAllByRole("checkbox");

      // Select first and third books
      await user.click(checkboxes[0]);
      await user.click(checkboxes[2]);

      const state = useBulkSelectionStore.getState();
      expect(state.selectedIds.has("book-1")).toBe(true);
      expect(state.selectedIds.has("book-3")).toBe(true);
      expect(state.selectedIds.size).toBe(2);
    });

    it("should deselect book when clicking selected checkbox", async () => {
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText("1 - Book One")).toBeInTheDocument();
      });

      const checkboxes = screen.getAllByRole("checkbox");

      // Select then deselect the first book
      await user.click(checkboxes[0]);
      expect(useBulkSelectionStore.getState().selectedIds.has("book-1")).toBe(
        true,
      );

      await user.click(checkboxes[0]);
      expect(useBulkSelectionStore.getState().selectedIds.has("book-1")).toBe(
        false,
      );
    });

    it("should exit selection mode when all items are deselected", async () => {
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText("1 - Book One")).toBeInTheDocument();
      });

      const checkboxes = screen.getAllByRole("checkbox");

      // Select then deselect
      await user.click(checkboxes[0]);
      expect(useBulkSelectionStore.getState().isSelectionMode).toBe(true);

      await user.click(checkboxes[0]);
      expect(useBulkSelectionStore.getState().isSelectionMode).toBe(false);
    });

    it("should show checkbox as checked when item is selected", async () => {
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText("1 - Book One")).toBeInTheDocument();
      });

      const checkboxes = screen.getAllByRole("checkbox");

      // Initially unchecked
      expect(checkboxes[0]).not.toBeChecked();

      // Click to select
      await user.click(checkboxes[0]);

      // Should now be checked
      expect(checkboxes[0]).toBeChecked();
    });

    it("should not allow selecting books when series are selected", async () => {
      // Pre-select a series to lock selection type
      useBulkSelectionStore.getState().toggleSelection("series-1", "series");

      renderComponent();

      await waitFor(() => {
        expect(screen.getByText("1 - Book One")).toBeInTheDocument();
      });

      // Checkboxes should be disabled since we're in series selection mode
      const checkboxes = screen.getAllByRole("checkbox");
      checkboxes.forEach((checkbox) => {
        expect(checkbox).toBeDisabled();
      });
    });
  });

  describe("sorting", () => {
    it("should sort books by number ascending by default", async () => {
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText("1 - Book One")).toBeInTheDocument();
      });

      // Get all book titles in order
      const titles = screen.getAllByText(/^\d+ - Book/);
      expect(titles[0]).toHaveTextContent("1 - Book One");
      expect(titles[4]).toHaveTextContent("5 - Book Five");
    });

    it("should display current sort option", async () => {
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText("Number (Ascending)")).toBeInTheDocument();
      });
    });
  });

  describe("pagination", () => {
    it("should show pagination when there are more books than page size", async () => {
      // Create 25 books to trigger pagination (default page size is 20)
      const manyBooks = Array.from({ length: 25 }, (_, i) =>
        createBook({
          id: `book-${i + 1}`,
          title: `Book ${i + 1}`,
          number: i + 1,
        }),
      );
      mockSeriesApi.getBooks.mockResolvedValue(manyBooks);

      const { container } = renderWithProviders(
        <SeriesBookList
          seriesId={seriesId}
          seriesName={seriesName}
          bookCount={25}
        />,
      );

      await waitFor(() => {
        expect(screen.getByText("1 - Book 1")).toBeInTheDocument();
      });

      // Mantine Pagination uses a div with class mantine-Pagination-root
      expect(
        container.querySelector(".mantine-Pagination-root"),
      ).toBeInTheDocument();
    });

    it("should not show pagination when books fit on one page", async () => {
      const { container } = renderComponent();

      await waitFor(() => {
        expect(screen.getByText("1 - Book One")).toBeInTheDocument();
      });

      // Pagination should not be visible for 5 books with default page size of 20
      expect(
        container.querySelector(".mantine-Pagination-root"),
      ).not.toBeInTheDocument();
    });
  });
});
