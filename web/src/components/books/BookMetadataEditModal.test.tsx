import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, waitFor } from "@/test/utils";
import { BookMetadataEditModal } from "./BookMetadataEditModal";

// Mock the API modules
vi.mock("@/api/books", () => ({
  booksApi: {
    getDetail: vi.fn(),
    getMetadataLocks: vi.fn(),
    patchMetadata: vi.fn(),
    updateMetadataLocks: vi.fn(),
    uploadCover: vi.fn(),
  },
}));

vi.mock("@/api/genres", () => ({
  genresApi: {
    getForBook: vi.fn(),
    setForBook: vi.fn(),
    getAll: vi.fn(),
  },
}));

vi.mock("@/api/tags", () => ({
  tagsApi: {
    getForBook: vi.fn(),
    setForBook: vi.fn(),
    getAll: vi.fn(),
  },
}));

import { booksApi } from "@/api/books";
import { genresApi } from "@/api/genres";
import { tagsApi } from "@/api/tags";

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
    volume: 5,
    chapter: 42.5,
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
  chapterLock: false,
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
    (genresApi.getForBook as ReturnType<typeof vi.fn>).mockResolvedValue([]);
    (genresApi.getAll as ReturnType<typeof vi.fn>).mockResolvedValue([]);
    (tagsApi.getForBook as ReturnType<typeof vi.fn>).mockResolvedValue([]);
    (tagsApi.getAll as ReturnType<typeof vi.fn>).mockResolvedValue([]);
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
      expect(
        screen.getByRole("tab", { name: /Publication/i }),
      ).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: /Authors/i })).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: /Tags/i })).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: /Links/i })).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: /Cover/i })).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: /Custom/i })).toBeInTheDocument();
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

  it("hydrates and round-trips fractional chapter through PATCH", async () => {
    renderWithProviders(
      <BookMetadataEditModal
        opened={true}
        onClose={vi.fn()}
        bookId="test-book-id"
      />,
    );

    // Switch to the publication tab where Volume + Chapter live
    await waitFor(() => {
      expect(
        screen.getByRole("tab", { name: /Publication/i }),
      ).toBeInTheDocument();
    });
    screen.getByRole("tab", { name: /Publication/i }).click();

    // Volume hydrates from mock (5), Chapter hydrates from mock (42.5)
    await waitFor(() => {
      expect(screen.getByDisplayValue("5")).toBeInTheDocument();
      expect(screen.getByDisplayValue("42.5")).toBeInTheDocument();
    });

    // Save without further edits; patchMetadata should still receive both values
    const saveButton = screen.getByRole("button", { name: /Save Changes/i });
    saveButton.click();

    await waitFor(() => {
      expect(booksApi.patchMetadata).toHaveBeenCalled();
    });

    const patchCall = (booksApi.patchMetadata as ReturnType<typeof vi.fn>).mock
      .calls[0];
    expect(patchCall[0]).toBe("test-book-id");
    expect(patchCall[1].volume).toBe(5);
    expect(patchCall[1].chapter).toBe(42.5);

    // Locks payload should propagate the chapterLock field independently
    await waitFor(() => {
      expect(booksApi.updateMetadataLocks).toHaveBeenCalled();
    });
    const locksCall = (booksApi.updateMetadataLocks as ReturnType<typeof vi.fn>)
      .mock.calls[0];
    expect(locksCall[1]).toHaveProperty("chapterLock");
    expect(locksCall[1]).toHaveProperty("volumeLock");
  });
});
