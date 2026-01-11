/**
 * Books API mock handlers
 */

import { http, HttpResponse, delay } from "msw";
import { createPaginatedResponse } from "../data/factories";
import { mockBooks, getBooksByLibrary, getBooksBySeries } from "../data/store";

export const bookHandlers = [
  // IMPORTANT: Specific routes MUST come before parameterized routes
  // Otherwise /api/v1/books/:id will match "in-progress" as an ID

  // List in-progress books (global - all libraries)
  // Returns plain array (not paginated) - matches API expectation
  http.get("/api/v1/books/in-progress", async () => {
    await delay(200);

    // Return books that have read progress
    const inProgressBooks = mockBooks.filter((b) => b.readProgress !== null);

    return HttpResponse.json(inProgressBooks);
  }),

  // List on-deck books (global - all libraries)
  // Returns paginated response with next book in series where user has completed books
  http.get("/api/v1/books/on-deck", async () => {
    await delay(200);

    // Return books that don't have progress (simulating "next to read")
    // In reality this would be first unread book from series with completed books
    const onDeckBooks = mockBooks.filter((b) => b.readProgress === null).slice(0, 10);

    return HttpResponse.json(
      createPaginatedResponse(onDeckBooks, {
        total: onDeckBooks.length,
      })
    );
  }),

  // List recently added books (global - all libraries)
  // Returns plain array (not paginated) - matches API expectation
  http.get("/api/v1/books/recently-added", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const limit = Number.parseInt(url.searchParams.get("limit") || "50");

    // Sort by created date (newest first)
    const sortedBooks = [...mockBooks].sort(
      (a, b) =>
        new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
    );

    return HttpResponse.json(sortedBooks.slice(0, limit));
  }),

  // List books with pagination
  http.get("/api/v1/books", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const page = Number.parseInt(url.searchParams.get("page") || "0");
    const pageSize = Number.parseInt(url.searchParams.get("pageSize") || "20");
    const seriesId = url.searchParams.get("seriesId");

    const filteredBooks = seriesId ? getBooksBySeries(seriesId) : mockBooks;

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

  // Get book by ID (must come AFTER specific routes like /in-progress, /recently-added)
  http.get("/api/v1/books/:id", async ({ params }) => {
    await delay(100);
    const book = mockBooks.find((b) => b.id === params.id);

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

    const filteredBooks = getBooksBySeries(params.seriesId as string);
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
  http.get("/api/v1/libraries/:libraryId/books", async ({ params, request }) => {
    await delay(200);
    const url = new URL(request.url);
    const page = Number.parseInt(url.searchParams.get("page") || "0");
    const pageSize = Number.parseInt(url.searchParams.get("pageSize") || "20");

    const libraryBooks = getBooksByLibrary(params.libraryId as string);
    const start = page * pageSize;
    const end = start + pageSize;
    const items = libraryBooks.slice(start, end);

    return HttpResponse.json(
      createPaginatedResponse(items, {
        page,
        pageSize,
        total: libraryBooks.length,
      })
    );
  }),

  // Library-scoped: List in-progress books
  // Returns plain array (not paginated) - matches API expectation
  http.get(
    "/api/v1/libraries/:libraryId/books/in-progress",
    async ({ params }) => {
      await delay(200);

      // Get books for this library that have read progress
      const libraryBooks = getBooksByLibrary(params.libraryId as string);
      const inProgressBooks = libraryBooks.filter(
        (b) => b.readProgress !== null
      );

      return HttpResponse.json(inProgressBooks);
    }
  ),

  // Library-scoped: List recently added books
  // Returns plain array (not paginated) - matches API expectation
  http.get(
    "/api/v1/libraries/:libraryId/books/recently-added",
    async ({ params, request }) => {
      await delay(200);
      const url = new URL(request.url);
      const limit = Number.parseInt(url.searchParams.get("limit") || "50");

      // Get books for this library, sorted by created date
      const libraryBooks = getBooksByLibrary(params.libraryId as string);
      const sortedBooks = [...libraryBooks].sort(
        (a, b) =>
          new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
      );

      return HttpResponse.json(sortedBooks.slice(0, limit));
    }
  ),

  // Library-scoped: List on-deck books
  // Returns paginated response
  http.get(
    "/api/v1/libraries/:libraryId/books/on-deck",
    async ({ params }) => {
      await delay(200);

      // Get books for this library that don't have progress
      const libraryBooks = getBooksByLibrary(params.libraryId as string);
      const onDeckBooks = libraryBooks.filter(
        (b) => b.readProgress === null
      ).slice(0, 10);

      return HttpResponse.json(
        createPaginatedResponse(onDeckBooks, {
          total: onDeckBooks.length,
        })
      );
    }
  ),
];

// Helper to get current mock books (for testing)
export const getMockBooks = () => [...mockBooks];
