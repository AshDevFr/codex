/**
 * Books API mock handlers
 */

import { http, HttpResponse, delay } from "msw";
import { createPaginatedResponse } from "../data/factories";
import { mockBooks, getBooksByLibrary, getBooksBySeries } from "../data/store";

export const bookHandlers = [
  // IMPORTANT: Specific routes MUST come before parameterized routes
  // Otherwise /api/v1/books/:id will match "in-progress" as an ID

  // List in-progress books
  // Supports ?library_id= query param for library filtering
  // Returns plain array (not paginated) - matches API expectation
  http.get("/api/v1/books/in-progress", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const libraryId = url.searchParams.get("library_id");

    // Return books that have read progress
    const baseBooks = libraryId ? getBooksByLibrary(libraryId) : mockBooks;
    const inProgressBooks = baseBooks.filter((b) => b.readProgress !== null);

    return HttpResponse.json(inProgressBooks);
  }),

  // List on-deck books
  // Supports ?library_id= query param for library filtering
  // Returns paginated response with next book in series where user has completed books
  http.get("/api/v1/books/on-deck", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const libraryId = url.searchParams.get("library_id");

    // Return books that don't have progress (simulating "next to read")
    // In reality this would be first unread book from series with completed books
    const baseBooks = libraryId ? getBooksByLibrary(libraryId) : mockBooks;
    const onDeckBooks = baseBooks.filter((b) => b.readProgress === null).slice(0, 10);

    return HttpResponse.json(
      createPaginatedResponse(onDeckBooks, {
        total: onDeckBooks.length,
      })
    );
  }),

  // List recently added books
  // Supports ?library_id= query param for library filtering
  // Returns plain array (not paginated) - matches API expectation
  http.get("/api/v1/books/recently-added", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const libraryId = url.searchParams.get("library_id");
    const limit = Number.parseInt(url.searchParams.get("limit") || "50");

    // Sort by created date (newest first)
    const baseBooks = libraryId ? getBooksByLibrary(libraryId) : mockBooks;
    const sortedBooks = [...baseBooks].sort(
      (a, b) =>
        new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
    );

    return HttpResponse.json(sortedBooks.slice(0, limit));
  }),

  // List recently read books
  // Supports ?library_id= query param for library filtering
  // Returns plain array (not paginated) - matches API expectation
  http.get("/api/v1/books/recently-read", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const libraryId = url.searchParams.get("library_id");
    const limit = Number.parseInt(url.searchParams.get("limit") || "50");

    // Return books that have been read (have read progress), sorted by last read
    const baseBooks = libraryId ? getBooksByLibrary(libraryId) : mockBooks;
    const readBooks = baseBooks.filter((b) => b.readProgress !== null).slice(0, limit);

    return HttpResponse.json(readBooks);
  }),

  // List books with pagination
  // Supports ?library_id= and ?series_id= query params for filtering
  http.get("/api/v1/books", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const page = Number.parseInt(url.searchParams.get("page") || "0");
    const pageSize = Number.parseInt(url.searchParams.get("page_size") || url.searchParams.get("pageSize") || "20");
    const libraryId = url.searchParams.get("library_id");
    const seriesId = url.searchParams.get("series_id") || url.searchParams.get("seriesId");

    let filteredBooks = libraryId ? getBooksByLibrary(libraryId) : mockBooks;
    if (seriesId) {
      filteredBooks = filteredBooks.filter((b) => b.seriesId === seriesId);
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

  // Get adjacent books (previous and next in series)
  http.get("/api/v1/books/:id/adjacent", async ({ params }) => {
    await delay(100);
    const book = mockBooks.find((b) => b.id === params.id);

    if (!book) {
      return HttpResponse.json({ error: "Book not found" }, { status: 404 });
    }

    // Find books in the same series
    const seriesBooks = mockBooks
      .filter((b) => b.seriesId === book.seriesId)
      .sort((a, b) => (a.number ?? 0) - (b.number ?? 0));

    const currentIndex = seriesBooks.findIndex((b) => b.id === book.id);
    const prev = currentIndex > 0 ? seriesBooks[currentIndex - 1] : null;
    const next = currentIndex < seriesBooks.length - 1 ? seriesBooks[currentIndex + 1] : null;

    return HttpResponse.json({ prev, next });
  }),

  // Generate thumbnail for a book
  http.post("/api/v1/books/:id/thumbnail", async () => {
    await delay(100);
    return HttpResponse.json({ message: "Thumbnail generation queued" });
  }),

  // Analyze book
  http.post("/api/v1/books/:id/analyze", async () => {
    await delay(100);
    return HttpResponse.json({ message: "Book analysis queued" });
  }),

  // Mark book as read
  http.post("/api/v1/books/:id/read", async () => {
    await delay(100);
    return HttpResponse.json({ message: "Book marked as read" });
  }),

  // Mark book as unread
  http.post("/api/v1/books/:id/unread", async () => {
    await delay(100);
    return HttpResponse.json({ message: "Book marked as unread" });
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
