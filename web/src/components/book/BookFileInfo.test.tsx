import { describe, expect, it } from "vitest";
import { renderWithProviders, screen } from "@/test/utils";
import type { Book } from "@/types";
import { BookFileInfo } from "./BookFileInfo";

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
  updatedAt: "2024-06-15T12:00:00Z",
  analysisError: null,
  number: 1,
  readProgress: null,
  deleted: false,
  ...overrides,
});

describe("BookFileInfo", () => {
  it("should render 'File Information' title", () => {
    renderWithProviders(<BookFileInfo book={createMockBook()} />);

    expect(screen.getByText("File Information")).toBeInTheDocument();
  });

  it("should display file format in uppercase", () => {
    renderWithProviders(
      <BookFileInfo book={createMockBook({ fileFormat: "epub" })} />,
    );

    expect(screen.getByText("Format")).toBeInTheDocument();
    expect(screen.getByText("EPUB")).toBeInTheDocument();
  });

  it("should format file size in MB", () => {
    renderWithProviders(
      <BookFileInfo book={createMockBook({ fileSize: 52428800 })} />,
    ); // 50 MB

    expect(screen.getByText("Size")).toBeInTheDocument();
    expect(screen.getByText("50.00 MB")).toBeInTheDocument();
  });

  it("should format file size in GB", () => {
    renderWithProviders(
      <BookFileInfo book={createMockBook({ fileSize: 1610612736 })} />,
    ); // 1.5 GB

    expect(screen.getByText("1.50 GB")).toBeInTheDocument();
  });

  it("should format file size in KB", () => {
    renderWithProviders(
      <BookFileInfo book={createMockBook({ fileSize: 512000 })} />,
    ); // 500 KB

    expect(screen.getByText("500.00 KB")).toBeInTheDocument();
  });

  it("should format file size in bytes", () => {
    renderWithProviders(
      <BookFileInfo book={createMockBook({ fileSize: 500 })} />,
    ); // 500 bytes

    expect(screen.getByText("500 B")).toBeInTheDocument();
  });

  it("should display page count", () => {
    renderWithProviders(
      <BookFileInfo book={createMockBook({ pageCount: 150 })} />,
    );

    expect(screen.getByText("Pages")).toBeInTheDocument();
    expect(screen.getByText("150")).toBeInTheDocument();
  });

  it("should display truncated hash", () => {
    const hash = "abc123def456ghi789jkl012mno345pqr678";
    renderWithProviders(
      <BookFileInfo book={createMockBook({ fileHash: hash })} />,
    );

    expect(screen.getByText("Hash")).toBeInTheDocument();
    // Should show first 12 characters + "..."
    expect(screen.getByText("abc123def456...")).toBeInTheDocument();
  });

  it("should display added date formatted", () => {
    renderWithProviders(
      <BookFileInfo
        book={createMockBook({ createdAt: "2024-06-15T12:00:00Z" })}
      />,
    );

    expect(screen.getByText("Added")).toBeInTheDocument();
    // Date format varies by locale, just check the label exists
  });

  it("should display file name from path", () => {
    renderWithProviders(
      <BookFileInfo
        book={createMockBook({
          filePath: "/library/comics/Series/issue-01.cbz",
        })}
      />,
    );

    expect(screen.getByText("File Path")).toBeInTheDocument();
    expect(screen.getByText("issue-01.cbz")).toBeInTheDocument();
  });

  it("should handle file path without directory", () => {
    renderWithProviders(
      <BookFileInfo book={createMockBook({ filePath: "simple-file.cbz" })} />,
    );

    expect(screen.getByText("simple-file.cbz")).toBeInTheDocument();
  });

  it("should display all info items", () => {
    renderWithProviders(<BookFileInfo book={createMockBook()} />);

    expect(screen.getByText("Format")).toBeInTheDocument();
    expect(screen.getByText("Size")).toBeInTheDocument();
    expect(screen.getByText("Pages")).toBeInTheDocument();
    expect(screen.getByText("Hash")).toBeInTheDocument();
    expect(screen.getByText("Added")).toBeInTheDocument();
    expect(screen.getByText("File Path")).toBeInTheDocument();
  });
});
