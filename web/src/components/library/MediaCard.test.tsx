import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  createBook,
  createReadProgress,
  createSeries,
} from "@/mocks/data/factories";
import { useAuthStore } from "@/store/authStore";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import type { User } from "@/types";
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

vi.mock("@/api/collections", () => ({
  collectionsApi: {
    list: vi.fn().mockResolvedValue([{ id: "col-1", name: "My Collection" }]),
    forSeries: vi.fn().mockResolvedValue([]),
    addSeries: vi.fn(),
    removeSeries: vi.fn(),
  },
}));

vi.mock("@/api/readlists", () => ({
  readListsApi: {
    list: vi.fn().mockResolvedValue([{ id: "rl-1", name: "My Read List" }]),
    forBook: vi.fn().mockResolvedValue([]),
    addBooks: vi.fn(),
    removeBook: vi.fn(),
  },
}));

const wantToReadApiMock = vi.hoisted(() => ({
  list: vi.fn().mockResolvedValue([]),
  addSeries: vi.fn().mockResolvedValue({}),
  addBook: vi.fn().mockResolvedValue({}),
  removeSeries: vi.fn().mockResolvedValue(undefined),
  removeBook: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@/api/wantToRead", () => ({
  wantToReadApi: wantToReadApiMock,
}));

const adminUser = {
  id: "u1",
  role: "admin",
  permissions: [],
} as unknown as User;

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

  describe("press affordance", () => {
    it("opts the card into the data-pressable affordance by default", () => {
      const book = createBook({ deleted: false });

      const { container } = renderWithProviders(
        <MediaCard type="book" data={book} />,
      );

      // The Phase 3 press/hover styling fires off `data-pressable="true"`.
      // Without this attribute, cards still navigate but the press scale +
      // hover lift won't apply.
      const card = container.querySelector(".mantine-Card-root");
      expect(card).toHaveAttribute("data-pressable", "true");
    });

    it("drops data-pressable when the card cannot be selected in selection mode", () => {
      const series = createBook({ deleted: false });

      const { container } = renderWithProviders(
        <MediaCard
          type="book"
          data={series}
          onSelect={vi.fn()}
          isSelectionMode={true}
          canBeSelected={false}
        />,
      );

      // When the type mismatches the active selection, the card is not
      // interactive. The press scale would mislead the user, so we strip
      // the affordance opt-in.
      const card = container.querySelector(".mantine-Card-root");
      expect(card).not.toHaveAttribute("data-pressable");
    });
  });

  describe("cover treatment", () => {
    it("leaves the cover image free of inline opacity transition so the CSS rule owns the fade", () => {
      // The Phase 5 fade-in (200ms desktop / 150ms mobile) lives in
      // index.css scoped to `.media-card-cover .mantine-Image-root`.
      // Re-introducing an inline `transition` on the <Image> would beat
      // the CSS rule due to inline-style specificity, so this assertion
      // guards against that regression.
      const book = createBook({ deleted: false });

      renderWithProviders(<MediaCard type="book" data={book} />);

      const image = screen.getByRole("img") as HTMLImageElement;
      expect(image.style.transition).toBe("");
      // The opacity toggle is the React-driven half of the fade and must
      // still be present; the CSS rule transitions whatever we set here.
      expect(image.style.opacity).toBe("0");
    });

    it("rounds the series unread badge's top-right corner to match the cover curve", () => {
      // The Phase 5 cover gets `border-top-right-radius: 10px`; without
      // a matching rounding on the badge, overflow:hidden on the cover
      // would clip the badge's top-right square corner flat against the
      // curve. Mirroring the radius keeps the badge sitting cleanly in
      // the rounded corner.
      const series = createSeries({ unreadCount: 3 });

      renderWithProviders(<MediaCard type="series" data={series} />);

      const badge = screen.getByText("3").closest("div");
      expect(badge).toHaveStyle({ borderTopRightRadius: "10px" });
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
        currentPage: 15,
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
        currentPage: 10,
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
        currentPage: 50,
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

  describe("collection / read list membership", () => {
    afterEach(() => {
      // Clear the admin user primed in these tests so other suites keep the
      // unauthenticated default (which hides the management menu entries).
      useAuthStore.setState({ user: null });
    });

    it("offers 'Add to collection' on a series card for users with collections-write", async () => {
      const user = userEvent.setup();
      useAuthStore.setState({ user: adminUser });
      const series = createSeries({ unreadCount: 2 });

      renderWithProviders(<MediaCard type="series" data={series} />);

      await user.click(screen.getByRole("button", { name: "Card actions" }));

      expect(await screen.findByText("Add to collection")).toBeInTheDocument();
    });

    it("offers 'Add to read list' on a book card for users with readlists-write", async () => {
      const user = userEvent.setup();
      useAuthStore.setState({ user: adminUser });
      const book = createBook();

      renderWithProviders(<MediaCard type="book" data={book} />);

      await user.click(screen.getByRole("button", { name: "Card actions" }));

      expect(await screen.findByText("Add to read list")).toBeInTheDocument();
    });

    it("hides membership entries when the user lacks write permission", async () => {
      const user = userEvent.setup();
      // No user primed -> usePermissions denies everything.
      const series = createSeries({ unreadCount: 2 });

      renderWithProviders(<MediaCard type="series" data={series} />);

      await user.click(screen.getByRole("button", { name: "Card actions" }));

      // The reading action still renders, but the membership submenu does not.
      expect(await screen.findByText("Mark as Read")).toBeInTheDocument();
      expect(screen.queryByText("Add to collection")).not.toBeInTheDocument();
    });
  });

  describe("hover card", () => {
    it("reveals a panel with the series summary and local counts on hover", async () => {
      const user = userEvent.setup();
      const series = createSeries({
        title: "Berserk",
        summary: "Guts, a lone mercenary, wanders a brutal medieval world.",
        bookCount: 41,
        localMaxVolume: 41,
        localMaxChapter: 364.5,
      });

      renderWithProviders(<MediaCard type="series" data={series} />);

      // The dropdown is lazy: nothing is rendered until the cover is hovered.
      expect(
        screen.queryByText(/Guts, a lone mercenary/),
      ).not.toBeInTheDocument();

      const cover = document.querySelector(".media-card-cover");
      expect(cover).not.toBeNull();
      await user.hover(cover as Element);

      expect(
        await screen.findByText(/Guts, a lone mercenary/),
      ).toBeInTheDocument();
      expect(await screen.findByText("41 vol · 364.5 ch")).toBeInTheDocument();
    });

    it("reveals a panel with book metadata on hover", async () => {
      const user = userEvent.setup();
      const book = createBook({
        title: "Volume 3",
        volume: 3,
        deleted: false,
      });

      renderWithProviders(<MediaCard type="book" data={book} />);

      const cover = document.querySelector(".media-card-cover");
      await user.hover(cover as Element);

      expect(await screen.findByText(/Vol 3/)).toBeInTheDocument();
    });
  });

  describe("want to read", () => {
    beforeEach(() => {
      wantToReadApiMock.addBook.mockClear();
      wantToReadApiMock.addSeries.mockClear();
      wantToReadApiMock.removeBook.mockClear();
      wantToReadApiMock.removeSeries.mockClear();
    });

    it("offers 'Add to Want to Read' on a book not in the queue", async () => {
      const user = userEvent.setup();
      const book = createBook({ wantToRead: false });

      renderWithProviders(<MediaCard type="book" data={book} />);

      await user.click(screen.getByRole("button", { name: "Card actions" }));

      expect(
        await screen.findByText("Add to Want to Read"),
      ).toBeInTheDocument();
    });

    it("offers 'Remove from Want to Read' on a book already in the queue", async () => {
      const user = userEvent.setup();
      const book = createBook({ wantToRead: true });

      renderWithProviders(<MediaCard type="book" data={book} />);

      await user.click(screen.getByRole("button", { name: "Card actions" }));

      expect(
        await screen.findByText("Remove from Want to Read"),
      ).toBeInTheDocument();
    });

    it("adds a book to the queue when the entry is clicked", async () => {
      const user = userEvent.setup();
      const book = createBook({ id: "book-wtr", wantToRead: false });

      renderWithProviders(<MediaCard type="book" data={book} />);

      await user.click(screen.getByRole("button", { name: "Card actions" }));
      await user.click(await screen.findByText("Add to Want to Read"));

      expect(wantToReadApiMock.addBook).toHaveBeenCalledWith("book-wtr");
    });

    it("removes a series from the queue when the entry is clicked", async () => {
      const user = userEvent.setup();
      const series = createSeries({ id: "series-wtr", wantToRead: true });

      renderWithProviders(<MediaCard type="series" data={series} />);

      await user.click(screen.getByRole("button", { name: "Card actions" }));
      await user.click(await screen.findByText("Remove from Want to Read"));

      expect(wantToReadApiMock.removeSeries).toHaveBeenCalledWith("series-wtr");
    });
  });
});
