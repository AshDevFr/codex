import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { booksApi } from "@/api/books";
import { seriesApi } from "@/api/series";
import { createBook } from "@/mocks/data/factories";
import { renderWithProviders, screen } from "@/test/utils";
import type { Book, PaginatedResponse } from "@/types";
import { RecommendedSection } from "./RecommendedSection";

vi.mock("@/api/books", () => ({
  booksApi: {
    getInProgress: vi.fn(),
    getOnDeck: vi.fn(),
    getRecentlyAdded: vi.fn(),
    getRecentlyRead: vi.fn(),
  },
}));

vi.mock("@/api/series", () => ({
  seriesApi: {
    getRecentlyAdded: vi.fn(),
    getRecentlyUpdated: vi.fn(),
  },
}));

function makeBook(id: string, title: string): Book {
  return createBook({ id, title }) as Book;
}

function page(books: Book[], total: number): PaginatedResponse<Book> {
  return {
    data: books,
    page: 1,
    pageSize: 50,
    total,
    totalPages: 1,
    links: {},
  } as PaginatedResponse<Book>;
}

describe("RecommendedSection - See all links", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(booksApi.getRecentlyAdded).mockResolvedValue(page([], 0));
    vi.mocked(booksApi.getRecentlyRead).mockResolvedValue([]);
    vi.mocked(seriesApi.getRecentlyAdded).mockResolvedValue([]);
    vi.mocked(seriesApi.getRecentlyUpdated).mockResolvedValue([]);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("shows a 'See all' link on Keep Reading when there are more than the carousel shows", async () => {
    vi.mocked(booksApi.getInProgress).mockResolvedValue(
      page([makeBook("b1", "Reading One")], 25),
    );
    vi.mocked(booksApi.getOnDeck).mockResolvedValue(page([], 0));

    renderWithProviders(<RecommendedSection libraryId="all" />, {
      initialEntries: ["/libraries/all/recommended"],
    });

    const link = await screen.findByRole("link", { name: /see all/i });
    expect(link).toHaveAttribute("href", "/libraries/all/keep-reading");
  });

  it("does not show a 'See all' link when the feed fits within the carousel", async () => {
    vi.mocked(booksApi.getInProgress).mockResolvedValue(
      page([makeBook("b1", "Reading One")], 5),
    );
    vi.mocked(booksApi.getOnDeck).mockResolvedValue(page([], 0));

    renderWithProviders(<RecommendedSection libraryId="all" />, {
      initialEntries: ["/libraries/all/recommended"],
    });

    // Wait for the carousel to render before asserting the link is absent.
    expect(await screen.findByText(/Reading One/)).toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: /see all/i }),
    ).not.toBeInTheDocument();
  });

  it("points the On Deck 'See all' link at the scoped on-deck route", async () => {
    vi.mocked(booksApi.getInProgress).mockResolvedValue(page([], 0));
    vi.mocked(booksApi.getOnDeck).mockResolvedValue(
      page([makeBook("b2", "Next Up")], 40),
    );

    renderWithProviders(<RecommendedSection libraryId="lib-9" />, {
      initialEntries: ["/libraries/lib-9/recommended"],
    });

    const link = await screen.findByRole("link", { name: /see all/i });
    expect(link).toHaveAttribute("href", "/libraries/lib-9/on-deck");
  });
});
