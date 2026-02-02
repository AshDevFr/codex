import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import type { Book, ReadProgress } from "@/types";
import { BookInfoModal } from "./BookInfoModal";

// Create a mock book with all required fields
const createMockBook = (overrides?: Partial<Book>): Book => ({
  id: "book-123",
  title: "Test Book",
  fileFormat: "cbz",
  fileSize: 52428800, // 50 MB
  pageCount: 200,
  fileHash: "abc123def456ghi789jkl012mno345pqr678",
  filePath: "/library/comics/Test Book/issue-01.cbz",
  libraryId: "lib-1",
  libraryName: "Comics",
  seriesId: "series-1",
  seriesName: "Test Series",
  createdAt: "2024-06-15T12:00:00Z",
  updatedAt: "2024-06-16T14:30:00Z",
  analysisError: null,
  number: 1,
  readProgress: null,
  deleted: false,
  ...overrides,
});

const createMockReadProgress = (
  overrides?: Partial<ReadProgress>,
): ReadProgress => ({
  id: "progress-1",
  bookId: "book-123",
  userId: "user-123",
  currentPage: 50,
  completed: false,
  completedAt: null,
  progressPercentage: 0.25,
  startedAt: "2024-06-15T10:00:00Z",
  updatedAt: "2024-06-15T12:00:00Z",
  ...overrides,
});

describe("BookInfoModal", () => {
  it("should not render when closed", () => {
    renderWithProviders(
      <BookInfoModal
        opened={false}
        onClose={vi.fn()}
        book={createMockBook()}
      />,
    );

    expect(screen.queryByText("Book Information")).not.toBeInTheDocument();
  });

  it("should render modal title when opened", () => {
    renderWithProviders(
      <BookInfoModal opened={true} onClose={vi.fn()} book={createMockBook()} />,
    );

    expect(screen.getByText("Book Information")).toBeInTheDocument();
  });

  it("should display basic book information", () => {
    renderWithProviders(
      <BookInfoModal
        opened={true}
        onClose={vi.fn()}
        book={createMockBook({
          title: "Batman: Year One",
          number: 1,
          seriesName: "Batman",
          libraryName: "Comics",
        })}
      />,
    );

    expect(screen.getByText("Basic Information")).toBeInTheDocument();
    expect(screen.getByText("Batman: Year One")).toBeInTheDocument();
    expect(screen.getByText("1")).toBeInTheDocument();
    expect(screen.getByText("Batman")).toBeInTheDocument();
    expect(screen.getByText("Comics")).toBeInTheDocument();
  });

  it("should display file information", () => {
    renderWithProviders(
      <BookInfoModal
        opened={true}
        onClose={vi.fn()}
        book={createMockBook({
          fileFormat: "epub",
          fileSize: 10485760, // 10 MB
          pageCount: 150,
        })}
      />,
    );

    expect(screen.getByText("File Information")).toBeInTheDocument();
    expect(screen.getByText("EPUB")).toBeInTheDocument();
    expect(screen.getByText("10.00 MB")).toBeInTheDocument();
    expect(screen.getByText("150")).toBeInTheDocument();
  });

  it("should display file path and hash with copy buttons", () => {
    renderWithProviders(
      <BookInfoModal
        opened={true}
        onClose={vi.fn()}
        book={createMockBook({
          filePath: "/path/to/book.cbz",
          fileHash: "abcdef123456",
        })}
      />,
    );

    expect(screen.getByText("/path/to/book.cbz")).toBeInTheDocument();
    expect(screen.getByText("abcdef123456")).toBeInTheDocument();
    // Copy buttons exist - check by the Tabler icon SVG or button presence
    const copyButtons = document.querySelectorAll(
      'button[class*="ActionIcon"]',
    );
    expect(copyButtons.length).toBeGreaterThanOrEqual(2);
  });

  it("should display reading progress when present", () => {
    renderWithProviders(
      <BookInfoModal
        opened={true}
        onClose={vi.fn()}
        book={createMockBook({
          pageCount: 200,
          readProgress: createMockReadProgress({
            currentPage: 50,
            progressPercentage: 0.25,
            completed: false,
          }),
        })}
      />,
    );

    expect(screen.getByText("Reading Progress")).toBeInTheDocument();
    expect(screen.getByText("50 / 200")).toBeInTheDocument();
    expect(screen.getByText("25%")).toBeInTheDocument();
    expect(screen.getByText("In Progress")).toBeInTheDocument();
  });

  it("should display completed status for finished books", () => {
    renderWithProviders(
      <BookInfoModal
        opened={true}
        onClose={vi.fn()}
        book={createMockBook({
          readProgress: createMockReadProgress({
            completed: true,
            completedAt: "2024-06-16T10:00:00Z",
          }),
        })}
      />,
    );

    // There should be a "Completed" badge in the Status row
    const badges = screen.getAllByText("Completed");
    expect(badges.length).toBeGreaterThanOrEqual(1);
  });

  it("should not display reading progress section when no progress", () => {
    renderWithProviders(
      <BookInfoModal
        opened={true}
        onClose={vi.fn()}
        book={createMockBook({ readProgress: null })}
      />,
    );

    expect(screen.queryByText("Reading Progress")).not.toBeInTheDocument();
  });

  it("should display timestamps and status", () => {
    renderWithProviders(
      <BookInfoModal
        opened={true}
        onClose={vi.fn()}
        book={createMockBook({ deleted: false })}
      />,
    );

    expect(screen.getByText("Timestamps & Status")).toBeInTheDocument();
    expect(screen.getByText("Added")).toBeInTheDocument();
    expect(screen.getByText("Updated")).toBeInTheDocument();
    expect(screen.getByText("Active")).toBeInTheDocument();
  });

  it("should display deleted status for soft-deleted books", () => {
    renderWithProviders(
      <BookInfoModal
        opened={true}
        onClose={vi.fn()}
        book={createMockBook({ deleted: true })}
      />,
    );

    expect(screen.getByText("Deleted")).toBeInTheDocument();
  });

  it("should display identifiers section with copyable IDs", () => {
    renderWithProviders(
      <BookInfoModal
        opened={true}
        onClose={vi.fn()}
        book={createMockBook({
          id: "book-uuid-123",
          seriesId: "series-uuid-456",
          libraryId: "library-uuid-789",
        })}
      />,
    );

    expect(screen.getByText("Identifiers")).toBeInTheDocument();
    expect(screen.getByText("book-uuid-123")).toBeInTheDocument();
    expect(screen.getByText("series-uuid-456")).toBeInTheDocument();
    expect(screen.getByText("library-uuid-789")).toBeInTheDocument();
  });

  it("should display analysis error when present", () => {
    renderWithProviders(
      <BookInfoModal
        opened={true}
        onClose={vi.fn()}
        book={createMockBook({
          analysisError: "Failed to parse CBZ: invalid archive",
        })}
      />,
    );

    expect(screen.getByText("Analysis Error")).toBeInTheDocument();
    expect(
      screen.getByText("Failed to parse CBZ: invalid archive"),
    ).toBeInTheDocument();
  });

  it("should not display analysis error section when no error", () => {
    renderWithProviders(
      <BookInfoModal
        opened={true}
        onClose={vi.fn()}
        book={createMockBook({ analysisError: null })}
      />,
    );

    expect(screen.queryByText("Analysis Error")).not.toBeInTheDocument();
  });

  it("should display reading direction when set", () => {
    renderWithProviders(
      <BookInfoModal
        opened={true}
        onClose={vi.fn()}
        book={createMockBook({ readingDirection: "rtl" })}
      />,
    );

    expect(screen.getByText("Reading Direction")).toBeInTheDocument();
    expect(screen.getByText("Right to Left")).toBeInTheDocument();
  });

  it("should display sort title when set", () => {
    renderWithProviders(
      <BookInfoModal
        opened={true}
        onClose={vi.fn()}
        book={createMockBook({ titleSort: "batman year one 001" })}
      />,
    );

    expect(screen.getByText("Sort Title")).toBeInTheDocument();
    expect(screen.getByText("batman year one 001")).toBeInTheDocument();
  });

  it("should call onClose when modal is closed", async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <BookInfoModal opened={true} onClose={onClose} book={createMockBook()} />,
    );

    // Click the close button (X) in the modal header - it has the Modal-close class
    const closeButton = document.querySelector(".mantine-Modal-close");
    expect(closeButton).toBeInTheDocument();
    if (closeButton) {
      await user.click(closeButton);
    }

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("should format file sizes correctly", () => {
    // Test GB
    const { rerender } = renderWithProviders(
      <BookInfoModal
        opened={true}
        onClose={vi.fn()}
        book={createMockBook({ fileSize: 2147483648 })} // 2 GB
      />,
    );
    expect(screen.getByText("2.00 GB")).toBeInTheDocument();

    // Test MB
    rerender(
      <BookInfoModal
        opened={true}
        onClose={vi.fn()}
        book={createMockBook({ fileSize: 52428800 })} // 50 MB
      />,
    );
    expect(screen.getByText("50.00 MB")).toBeInTheDocument();

    // Test KB
    rerender(
      <BookInfoModal
        opened={true}
        onClose={vi.fn()}
        book={createMockBook({ fileSize: 512000 })} // 500 KB
      />,
    );
    expect(screen.getByText("500.00 KB")).toBeInTheDocument();

    // Test bytes
    rerender(
      <BookInfoModal
        opened={true}
        onClose={vi.fn()}
        book={createMockBook({ fileSize: 500 })} // 500 bytes
      />,
    );
    expect(screen.getByText("500 B")).toBeInTheDocument();
  });
});
