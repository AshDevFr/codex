import { beforeEach, describe, expect, it, vi } from "vitest";
import { createBook, createReadProgress } from "@/mocks/data/factories";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { MediaCard } from "./MediaCard";

const mockNavigate = vi.fn();

// Mock react-router-dom
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return {
    ...actual,
    useNavigate: () => mockNavigate,
  };
});

// Mock the API modules
vi.mock("@/api/books", () => ({
  booksApi: {
    analyze: vi.fn(),
    markAsRead: vi.fn(),
    markAsUnread: vi.fn(),
  },
}));

vi.mock("@/api/series", () => ({
  seriesApi: {
    analyze: vi.fn(),
    analyzeUnanalyzed: vi.fn(),
    markAsRead: vi.fn(),
    markAsUnread: vi.fn(),
  },
}));

describe("MediaCard", () => {
  describe("book display", () => {
    it("should display cover image for non-deleted book", () => {
      const book = createBook({ deleted: false });

      renderWithProviders(<MediaCard type="book" data={book} />);

      // Should have an image element with the thumbnail URL
      const image = screen.getByRole("img");
      expect(image).toBeInTheDocument();
      expect(image).toHaveAttribute(
        "src",
        expect.stringContaining(`/api/v1/books/${book.id}/thumbnail`),
      );
    });

    it("should display deleted placeholder for deleted book", () => {
      const book = createBook({ deleted: true });

      renderWithProviders(<MediaCard type="book" data={book} />);

      // Should show "Deleted" text instead of cover image
      expect(screen.getByText("Deleted")).toBeInTheDocument();

      // Should not have an image element for the thumbnail
      const images = screen.queryAllByRole("img");
      // There should be no images (deleted placeholder uses icon, not img)
      expect(images).toHaveLength(0);
    });

    it("should display book title", () => {
      const book = createBook({ title: "Test Book Title", number: 5 });

      renderWithProviders(<MediaCard type="book" data={book} />);

      expect(screen.getByText("5 - Test Book Title")).toBeInTheDocument();
    });

    it("should display series name for book", () => {
      const book = createBook({ seriesName: "Test Series Name" });

      renderWithProviders(<MediaCard type="book" data={book} />);

      expect(screen.getByText("Test Series Name")).toBeInTheDocument();
    });

    it("should display page count and file format", () => {
      const book = createBook({
        pageCount: 42,
        fileFormat: "cbz",
      });

      renderWithProviders(<MediaCard type="book" data={book} />);

      expect(screen.getByText("42 pages")).toBeInTheDocument();
      expect(screen.getByText("CBZ")).toBeInTheDocument();
    });
  });

  describe("hover overlay and progress", () => {
    beforeEach(() => {
      mockNavigate.mockClear();
    });

    it("should display read button overlay for non-deleted books", () => {
      const book = createBook({ deleted: false });

      renderWithProviders(<MediaCard type="book" data={book} />);

      // Read button should be present (even if hidden by CSS)
      const readButton = screen.getByRole("button", { name: "Read book" });
      expect(readButton).toBeInTheDocument();
    });

    it("should not display read button overlay for deleted books", () => {
      const book = createBook({ deleted: true });

      renderWithProviders(<MediaCard type="book" data={book} />);

      // Read button should not be present for deleted books
      expect(
        screen.queryByRole("button", { name: "Read book" }),
      ).not.toBeInTheDocument();
    });

    it("should navigate to reader on read button click", async () => {
      const user = userEvent.setup();
      const book = createBook({ id: "book-123", deleted: false });

      renderWithProviders(<MediaCard type="book" data={book} />);

      const readButton = screen.getByRole("button", { name: "Read book" });
      await user.click(readButton);

      expect(mockNavigate).toHaveBeenCalledWith("/reader/book-123?page=1");
    });

    it("should navigate to reader with current page when book has progress", async () => {
      const user = userEvent.setup();
      const progress = createReadProgress({
        current_page: 15,
        completed: false,
      });
      const book = createBook({
        id: "book-456",
        deleted: false,
        readProgress: progress,
      });

      renderWithProviders(<MediaCard type="book" data={book} />);

      const readButton = screen.getByRole("button", { name: "Read book" });
      await user.click(readButton);

      expect(mockNavigate).toHaveBeenCalledWith("/reader/book-456?page=15");
    });

    it("should display progress bar for books with in-progress reading", () => {
      const progress = createReadProgress({
        current_page: 10,
        completed: false,
      });
      const book = createBook({ pageCount: 50, readProgress: progress });

      renderWithProviders(<MediaCard type="book" data={book} />);

      // Progress bar should be present
      const progressBar = document.querySelector('[role="progressbar"]');
      expect(progressBar).toBeInTheDocument();
      expect(progressBar).toHaveAttribute("aria-valuenow", "20"); // 10/50 = 20%
    });

    it("should not display progress bar for completed books", () => {
      const progress = createReadProgress({
        current_page: 50,
        completed: true,
      });
      const book = createBook({ pageCount: 50, readProgress: progress });

      renderWithProviders(<MediaCard type="book" data={book} />);

      // Progress bar should not be present for completed books
      const progressBar = document.querySelector('[role="progressbar"]');
      expect(progressBar).not.toBeInTheDocument();
    });

    it("should not display progress bar for books without progress", () => {
      const book = createBook({ readProgress: null });

      renderWithProviders(<MediaCard type="book" data={book} />);

      // Progress bar should not be present
      const progressBar = document.querySelector('[role="progressbar"]');
      expect(progressBar).not.toBeInTheDocument();
    });
  });

  describe("selection functionality", () => {
    beforeEach(() => {
      mockNavigate.mockClear();
    });

    it("should not show checkbox when onSelect is not provided", () => {
      const book = createBook();

      renderWithProviders(<MediaCard type="book" data={book} />);

      // Checkbox should not be present
      expect(
        screen.queryByRole("checkbox", { name: /select/i }),
      ).not.toBeInTheDocument();
    });

    it("should show checkbox when onSelect is provided", () => {
      const book = createBook({ title: "Test Book" });
      const onSelect = vi.fn();

      renderWithProviders(
        <MediaCard type="book" data={book} onSelect={onSelect} />,
      );

      // Checkbox should be present
      expect(
        screen.getByRole("checkbox", { name: /select test book/i }),
      ).toBeInTheDocument();
    });

    it("should call onSelect when checkbox is clicked", async () => {
      const user = userEvent.setup();
      const book = createBook({ id: "book-123", title: "Test Book" });
      const onSelect = vi.fn();

      renderWithProviders(
        <MediaCard type="book" data={book} onSelect={onSelect} />,
      );

      const checkbox = screen.getByRole("checkbox", {
        name: /select test book/i,
      });
      await user.click(checkbox);

      expect(onSelect).toHaveBeenCalledWith("book-123", false, undefined);
    });

    it("should show checkbox as checked when isSelected is true", () => {
      const book = createBook({ title: "Test Book" });
      const onSelect = vi.fn();

      renderWithProviders(
        <MediaCard
          type="book"
          data={book}
          onSelect={onSelect}
          isSelected={true}
        />,
      );

      const checkbox = screen.getByRole("checkbox", {
        name: /select test book/i,
      });
      expect(checkbox).toBeChecked();
    });

    it("should show checkbox as unchecked when isSelected is false", () => {
      const book = createBook({ title: "Test Book" });
      const onSelect = vi.fn();

      renderWithProviders(
        <MediaCard
          type="book"
          data={book}
          onSelect={onSelect}
          isSelected={false}
        />,
      );

      const checkbox = screen.getByRole("checkbox", {
        name: /select test book/i,
      });
      expect(checkbox).not.toBeChecked();
    });

    it("should disable checkbox when canBeSelected is false", () => {
      const book = createBook({ title: "Test Book" });
      const onSelect = vi.fn();

      renderWithProviders(
        <MediaCard
          type="book"
          data={book}
          onSelect={onSelect}
          canBeSelected={false}
        />,
      );

      const checkbox = screen.getByRole("checkbox", {
        name: /select test book/i,
      });
      expect(checkbox).toBeDisabled();
    });

    it("should not call onSelect when checkbox is clicked and canBeSelected is false", async () => {
      const user = userEvent.setup();
      const book = createBook({ title: "Test Book" });
      const onSelect = vi.fn();

      renderWithProviders(
        <MediaCard
          type="book"
          data={book}
          onSelect={onSelect}
          canBeSelected={false}
        />,
      );

      const checkbox = screen.getByRole("checkbox", {
        name: /select test book/i,
      });
      await user.click(checkbox);

      expect(onSelect).not.toHaveBeenCalled();
    });

    it("should navigate to book when card is clicked and not in selection mode", async () => {
      const user = userEvent.setup();
      const book = createBook({ id: "book-123", title: "Navigate Test Book" });
      const onSelect = vi.fn();

      renderWithProviders(
        <MediaCard
          type="book"
          data={book}
          onSelect={onSelect}
          isSelectionMode={false}
        />,
      );

      // Click on the card itself
      const card = document.querySelector(".mantine-Card-root");
      if (card) {
        await user.click(card);
      }

      expect(mockNavigate).toHaveBeenCalledWith("/books/book-123");
      expect(onSelect).not.toHaveBeenCalled();
    });

    it("should call onSelect when card is clicked in selection mode", async () => {
      const user = userEvent.setup();
      const book = createBook({
        id: "book-123",
        title: "Selection Mode Click Test",
      });
      const onSelect = vi.fn();

      renderWithProviders(
        <MediaCard
          type="book"
          data={book}
          onSelect={onSelect}
          isSelectionMode={true}
          canBeSelected={true}
        />,
      );

      // Click on the card itself (via the Card element)
      const card = document.querySelector(".mantine-Card-root");
      if (card) {
        await user.click(card);
      }

      expect(onSelect).toHaveBeenCalledWith("book-123", false, undefined);
      expect(mockNavigate).not.toHaveBeenCalled();
    });

    it("should not call onSelect when card is clicked in selection mode but canBeSelected is false", async () => {
      const user = userEvent.setup();
      const book = createBook({
        id: "book-123",
        title: "Disabled Selection Test",
      });
      const onSelect = vi.fn();

      renderWithProviders(
        <MediaCard
          type="book"
          data={book}
          onSelect={onSelect}
          isSelectionMode={true}
          canBeSelected={false}
        />,
      );

      // Click on the card itself
      const card = document.querySelector(".mantine-Card-root");
      if (card) {
        await user.click(card);
      }

      expect(onSelect).not.toHaveBeenCalled();
      // Should also not navigate when canBeSelected is false
      expect(mockNavigate).not.toHaveBeenCalled();
    });

    it("should have orange border when selected", () => {
      const book = createBook();
      const onSelect = vi.fn();

      renderWithProviders(
        <MediaCard
          type="book"
          data={book}
          onSelect={onSelect}
          isSelected={true}
        />,
      );

      const card = document.querySelector(".mantine-Card-root");
      expect(card).toHaveStyle({
        border: "1px solid var(--mantine-color-orange-6)",
      });
    });

    it("should apply selection mode class when isSelectionMode is true", () => {
      const book = createBook();
      const onSelect = vi.fn();

      renderWithProviders(
        <MediaCard
          type="book"
          data={book}
          onSelect={onSelect}
          isSelectionMode={true}
        />,
      );

      const card = document.querySelector(".mantine-Card-root");
      expect(card).toHaveClass("media-card--selection-mode");
    });

    it("should apply disabled class when in selection mode and canBeSelected is false", () => {
      const book = createBook();
      const onSelect = vi.fn();

      renderWithProviders(
        <MediaCard
          type="book"
          data={book}
          onSelect={onSelect}
          isSelectionMode={true}
          canBeSelected={false}
        />,
      );

      const card = document.querySelector(".mantine-Card-root");
      expect(card).toHaveClass("media-card--disabled");
    });
  });
});
