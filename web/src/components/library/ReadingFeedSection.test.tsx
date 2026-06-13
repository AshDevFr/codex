import { waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { booksApi } from "@/api/books";
import { createBook } from "@/mocks/data/factories";
import { useBulkSelectionStore } from "@/store/bulkSelectionStore";
import { renderWithProviders, screen } from "@/test/utils";
import type { Book, PaginatedResponse } from "@/types";
import { ReadingFeedSection } from "./ReadingFeedSection";

vi.mock("@/api/books", () => ({
  booksApi: {
    getInProgress: vi.fn(),
    getOnDeck: vi.fn(),
  },
}));

function makeBook(id: string, title: string): Book {
  return createBook({ id, title }) as Book;
}

function makeResponse(
  books: Book[],
  overrides: Partial<PaginatedResponse<Book>> = {},
): PaginatedResponse<Book> {
  return {
    data: books,
    page: 1,
    pageSize: 50,
    total: books.length,
    totalPages: 1,
    links: {},
    ...overrides,
  } as PaginatedResponse<Book>;
}

describe("ReadingFeedSection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useBulkSelectionStore.getState().clearSelection();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renders book cards from the in-progress feed", async () => {
    vi.mocked(booksApi.getInProgress).mockResolvedValueOnce(
      makeResponse([
        makeBook("b1", "Reading One"),
        makeBook("b2", "Reading Two"),
      ]),
    );

    renderWithProviders(
      <ReadingFeedSection
        libraryId="all"
        feed="in-progress"
        searchParams={new URLSearchParams()}
      />,
    );

    expect(await screen.findByText(/Reading One/)).toBeInTheDocument();
    expect(screen.getByText(/Reading Two/)).toBeInTheDocument();
  });

  it("calls the on-deck endpoint for the on-deck feed", async () => {
    vi.mocked(booksApi.getOnDeck).mockResolvedValueOnce(
      makeResponse([makeBook("b1", "Next Up")]),
    );

    renderWithProviders(
      <ReadingFeedSection
        libraryId="lib-1"
        feed="on-deck"
        searchParams={new URLSearchParams()}
      />,
    );

    await waitFor(() => {
      expect(booksApi.getOnDeck).toHaveBeenCalledWith("lib-1", {
        page: 1,
        pageSize: 50,
      });
    });
    expect(booksApi.getInProgress).not.toHaveBeenCalled();
  });

  it("forwards page/pageSize from the URL search params", async () => {
    vi.mocked(booksApi.getInProgress).mockResolvedValueOnce(
      makeResponse([], { total: 0 }),
    );

    renderWithProviders(
      <ReadingFeedSection
        libraryId="all"
        feed="in-progress"
        searchParams={new URLSearchParams("page=3&pageSize=24")}
      />,
    );

    await waitFor(() => {
      expect(booksApi.getInProgress).toHaveBeenCalledWith("all", {
        page: 3,
        pageSize: 24,
      });
    });
  });

  it("shows the feed-specific empty state when there are no items", async () => {
    vi.mocked(booksApi.getOnDeck).mockResolvedValueOnce(
      makeResponse([], { total: 0 }),
    );

    renderWithProviders(
      <ReadingFeedSection
        libraryId="all"
        feed="on-deck"
        searchParams={new URLSearchParams()}
      />,
    );

    expect(await screen.findByText("Nothing on deck")).toBeInTheDocument();
  });

  it("shows pagination when total exceeds the page size", async () => {
    vi.mocked(booksApi.getInProgress).mockResolvedValueOnce(
      makeResponse([makeBook("b1", "Reading One")], {
        total: 120,
        pageSize: 50,
        totalPages: 3,
      }),
    );

    renderWithProviders(
      <ReadingFeedSection
        libraryId="all"
        feed="in-progress"
        searchParams={new URLSearchParams()}
      />,
    );

    // Controls for page 2 (top + bottom) confirm the Pagination rendered.
    await screen.findByText(/Showing 1 to 50 of 120 books/);
    expect(screen.getAllByRole("button", { name: "2" }).length).toBeGreaterThan(
      0,
    );
  });

  it("does not show pagination when total fits in one page", async () => {
    vi.mocked(booksApi.getInProgress).mockResolvedValueOnce(
      makeResponse([makeBook("b1", "Reading One")], {
        total: 1,
        pageSize: 50,
        totalPages: 1,
      }),
    );

    renderWithProviders(
      <ReadingFeedSection
        libraryId="all"
        feed="in-progress"
        searchParams={new URLSearchParams()}
      />,
    );

    expect(await screen.findByText(/Reading One/)).toBeInTheDocument();
    // No page-2 control when everything fits on one page.
    expect(screen.queryAllByRole("button", { name: "2" })).toHaveLength(0);
  });

  it("reports the total to onTotalChange", async () => {
    const onTotalChange = vi.fn();
    vi.mocked(booksApi.getInProgress).mockResolvedValueOnce(
      makeResponse([makeBook("b1", "Reading One")], { total: 7 }),
    );

    renderWithProviders(
      <ReadingFeedSection
        libraryId="all"
        feed="in-progress"
        searchParams={new URLSearchParams()}
        onTotalChange={onTotalChange}
      />,
    );

    await waitFor(() => {
      expect(onTotalChange).toHaveBeenCalledWith(7);
    });
  });
});
