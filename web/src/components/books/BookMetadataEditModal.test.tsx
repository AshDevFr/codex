import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, waitFor } from "@/test/utils";
import { BookMetadataEditModal } from "./BookMetadataEditModal";

// Mock the API module
vi.mock("@/api/books", () => ({
  booksApi: {
    getDetail: vi.fn(),
    getMetadataLocks: vi.fn(),
    patchMetadata: vi.fn(),
    updateMetadataLocks: vi.fn(),
    uploadCover: vi.fn(),
  },
}));

import { booksApi } from "@/api/books";

const mockBookDetail = {
  book: {
    id: "test-book-id",
    seriesId: "test-series-id",
    libraryId: "test-library-id",
    title: "Test Book",
    number: 1,
    sortNumber: 1,
    fileFormat: "cbz",
    filePath: "/test/path",
    fileSize: 1000,
    fileHash: "abc123",
    pageCount: 24,
    deleted: false,
    createdAt: "2024-01-01T00:00:00Z",
    updatedAt: "2024-01-01T00:00:00Z",
  },
  metadata: {
    id: "meta-id",
    bookId: "test-book-id",
    summary: "A test summary",
    writers: ["Test Writer"],
    pencillers: ["Test Penciller"],
    inkers: [],
    colorists: [],
    letterers: [],
    coverArtists: [],
    editors: [],
    publisher: "Test Publisher",
    imprint: null,
    genre: "Action",
    languageIso: "en",
  },
};

const mockLocks = {
  summaryLock: false,
  writerLock: false,
  pencillerLock: false,
  inkerLock: false,
  coloristLock: false,
  lettererLock: false,
  coverArtistLock: false,
  editorLock: false,
  publisherLock: false,
  imprintLock: false,
  genreLock: false,
  languageIsoLock: false,
  formatDetailLock: false,
  blackAndWhiteLock: false,
  mangaLock: false,
  yearLock: false,
  monthLock: false,
  dayLock: false,
  volumeLock: false,
  countLock: false,
  isbnsLock: false,
};

describe("BookMetadataEditModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (booksApi.getDetail as ReturnType<typeof vi.fn>).mockResolvedValue(
      mockBookDetail,
    );
    (booksApi.getMetadataLocks as ReturnType<typeof vi.fn>).mockResolvedValue(
      mockLocks,
    );
    (booksApi.patchMetadata as ReturnType<typeof vi.fn>).mockResolvedValue({});
    (
      booksApi.updateMetadataLocks as ReturnType<typeof vi.fn>
    ).mockResolvedValue({});
  });

  it("renders modal with title", async () => {
    renderWithProviders(
      <BookMetadataEditModal
        opened={true}
        onClose={vi.fn()}
        bookId="test-book-id"
        bookTitle="Test Book"
      />,
    );

    expect(screen.getByText(/Edit Test Book/)).toBeInTheDocument();
  });

  it("shows loading state initially", () => {
    renderWithProviders(
      <BookMetadataEditModal
        opened={true}
        onClose={vi.fn()}
        bookId="test-book-id"
      />,
    );

    // Should show loader while fetching (Mantine loader uses a span)
    const loader = document.querySelector(".mantine-Loader-root");
    expect(loader).toBeInTheDocument();
  });

  it("loads and displays metadata", async () => {
    renderWithProviders(
      <BookMetadataEditModal
        opened={true}
        onClose={vi.fn()}
        bookId="test-book-id"
      />,
    );

    await waitFor(() => {
      expect(booksApi.getDetail).toHaveBeenCalledWith("test-book-id");
      expect(booksApi.getMetadataLocks).toHaveBeenCalledWith("test-book-id");
    });

    // Wait for form to load
    await waitFor(() => {
      expect(screen.getByDisplayValue("Test Book")).toBeInTheDocument();
    });
  });

  it("shows all tabs", async () => {
    renderWithProviders(
      <BookMetadataEditModal
        opened={true}
        onClose={vi.fn()}
        bookId="test-book-id"
      />,
    );

    await waitFor(() => {
      expect(screen.getByRole("tab", { name: /General/i })).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: /Authors/i })).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: /Tags/i })).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: /Links/i })).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: /Cover/i })).toBeInTheDocument();
    });
  });

  it("calls onClose when cancel is clicked", async () => {
    const onClose = vi.fn();

    renderWithProviders(
      <BookMetadataEditModal
        opened={true}
        onClose={onClose}
        bookId="test-book-id"
      />,
    );

    await waitFor(() => {
      expect(screen.getByDisplayValue("Test Book")).toBeInTheDocument();
    });

    const cancelButton = screen.getByRole("button", { name: /Cancel/i });
    cancelButton.click();

    expect(onClose).toHaveBeenCalled();
  });

  it("does not fetch when closed", () => {
    renderWithProviders(
      <BookMetadataEditModal
        opened={false}
        onClose={vi.fn()}
        bookId="test-book-id"
      />,
    );

    expect(booksApi.getDetail).not.toHaveBeenCalled();
    expect(booksApi.getMetadataLocks).not.toHaveBeenCalled();
  });
});
