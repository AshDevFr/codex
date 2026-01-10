/**
 * Books API mock handlers
 */

import { http, HttpResponse, delay } from "msw";
import {
  createBook,
  createList,
  createPaginatedResponse,
  createReadProgress,
  type MockBook,
} from "../data/factories";

// In-memory mock data store
let books: MockBook[] = createList(
  (i) =>
    createBook({
      seriesName: [
        "Batman: Year One",
        "Spider-Man",
        "Saga",
        "The Walking Dead",
        "One Piece",
      ][i % 5],
      number: (i % 20) + 1,
    }),
  100
);

export const bookHandlers = [
  // List books with pagination
  http.get("/api/v1/books", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const page = Number.parseInt(url.searchParams.get("page") || "0");
    const pageSize = Number.parseInt(url.searchParams.get("pageSize") || "20");
    const seriesId = url.searchParams.get("seriesId");

    let filteredBooks = books;
    if (seriesId) {
      filteredBooks = books.filter((b) => b.seriesId === seriesId);
    }

    const start = page * pageSize;
    const end = start + pageSize;
    const items = filteredBooks.slice(start, end);

    return HttpResponse.json(
      createPaginatedResponse(items, {
        page,
        pageSize,
        total: filteredBooks.length,
      })
    );
  }),

  // Get book by ID
  http.get("/api/v1/books/:id", async ({ params }) => {
    await delay(100);
    const book = books.find((b) => b.id === params.id);

    if (!book) {
      return HttpResponse.json({ error: "Book not found" }, { status: 404 });
    }

    return HttpResponse.json({
      book,
      metadata: {
        id: book.id,
        bookId: book.id,
        title: book.title,
        series: book.seriesName,
        number: book.number?.toString(),
        summary: `A thrilling issue of ${book.seriesName}.`,
        publisher: "DC Comics",
        imprint: null,
        genre: "Superhero",
        pageCount: book.pageCount,
        languageIso: "en",
        releaseDate: null,
        writers: ["Frank Miller"],
        pencillers: ["David Mazzucchelli"],
        inkers: ["David Mazzucchelli"],
        colorists: ["Richmond Lewis"],
        letterers: ["Todd Klein"],
        coverArtists: ["David Mazzucchelli"],
        editors: ["Dennis O'Neil"],
      },
    });
  }),

  // Get book thumbnail
  http.get("/api/v1/books/:id/thumbnail", async () => {
    await delay(50);
    // Return a placeholder image response
    return new HttpResponse(null, {
      status: 302,
      headers: {
        Location: "https://placehold.co/300x450/333/fff?text=Cover",
      },
    });
  }),

  // Get book page image
  http.get("/api/v1/books/:id/pages/:pageNum", async ({ params }) => {
    await delay(100);
    // Return a placeholder page image
    return new HttpResponse(null, {
      status: 302,
      headers: {
        Location: `https://placehold.co/800x1200/222/fff?text=Page+${params.pageNum}`,
      },
    });
  }),

  // List books by series
  http.get("/api/v1/series/:seriesId/books", async ({ params, request }) => {
    await delay(200);
    const url = new URL(request.url);
    const page = Number.parseInt(url.searchParams.get("page") || "0");
    const pageSize = Number.parseInt(url.searchParams.get("pageSize") || "20");

    const filteredBooks = books.filter((b) => b.seriesId === params.seriesId);
    const start = page * pageSize;
    const end = start + pageSize;
    const items = filteredBooks.slice(start, end);

    return HttpResponse.json(
      createPaginatedResponse(items, {
        page,
        pageSize,
        total: filteredBooks.length,
      })
    );
  }),

  // List books by library
  http.get("/api/v1/libraries/:libraryId/books", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const page = Number.parseInt(url.searchParams.get("page") || "0");
    const pageSize = Number.parseInt(url.searchParams.get("pageSize") || "20");

    const start = page * pageSize;
    const end = start + pageSize;
    const items = books.slice(start, end);

    return HttpResponse.json(
      createPaginatedResponse(items, {
        page,
        pageSize,
        total: books.length,
      })
    );
  }),

  // List in-progress books
  http.get("/api/v1/books/in-progress", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const page = Number.parseInt(url.searchParams.get("page") || "0");
    const pageSize = Number.parseInt(url.searchParams.get("pageSize") || "20");

    // Return some books with read progress
    const inProgressBooks = books.slice(0, 10).map((book) => ({
      ...book,
      readProgress: createReadProgress({
        bookId: book.id,
        totalPages: book.pageCount,
      }),
    }));

    const start = page * pageSize;
    const end = start + pageSize;
    const items = inProgressBooks.slice(start, end);

    return HttpResponse.json(
      createPaginatedResponse(items, {
        page,
        pageSize,
        total: inProgressBooks.length,
      })
    );
  }),

  // List recently added books
  http.get("/api/v1/books/recently-added", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const page = Number.parseInt(url.searchParams.get("page") || "0");
    const pageSize = Number.parseInt(url.searchParams.get("pageSize") || "20");

    // Sort by created date (newest first)
    const sortedBooks = [...books].sort(
      (a, b) =>
        new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
    );

    const start = page * pageSize;
    const end = start + pageSize;
    const items = sortedBooks.slice(start, end);

    return HttpResponse.json(
      createPaginatedResponse(items, {
        page,
        pageSize,
        total: sortedBooks.length,
      })
    );
  }),
];

// Helper to reset mock data (for testing)
export const resetMockBooks = () => {
  books = createList(
    (i) =>
      createBook({
        seriesName: [
          "Batman: Year One",
          "Spider-Man",
          "Saga",
          "The Walking Dead",
          "One Piece",
        ][i % 5],
        number: (i % 20) + 1,
      }),
    100
  );
};

// Helper to get current mock books (for testing)
export const getMockBooks = () => [...books];
