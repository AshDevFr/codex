/**
 * Books API mock handlers
 */

import { delay, HttpResponse, http } from "msw";
import {
  bookTitlesAndSummaries,
  createPaginatedResponse,
} from "../data/factories";
import { getBooksByLibrary, getBooksBySeries, mockBooks } from "../data/store";
import coverSvg from "../fixtures/cover.svg?raw";
import pageSvg from "../fixtures/page.svg?raw";
import sampleCbzUrl from "../fixtures/sample.cbz?url";
import sampleEpubUrl from "../fixtures/sample.epub?url";
import samplePdfUrl from "../fixtures/sample.pdf?url";

/**
 * Get a detailed summary for a book based on series and number
 */
function getBookSummary(seriesName: string, number: number | null): string {
  if (number === null) return `An exciting volume of ${seriesName}.`;

  const seriesBooks = bookTitlesAndSummaries[seriesName];
  const bookInfo = seriesBooks?.[number - 1];

  if (bookInfo) {
    return bookInfo.summary;
  }

  // Generate a generic but descriptive summary
  return `Volume ${number} of ${seriesName} continues the thrilling saga with new challenges, deepening character arcs, and unexpected twists that will keep readers on the edge of their seats. This installment expands the world-building while advancing the central plot toward its next major turning point.`;
}

/**
 * Publisher-specific creative teams for realistic metadata
 */
const creativeTeams: Record<
  string,
  {
    writers: string[];
    artists: string[];
    colorists: string[];
    letterers: string[];
    editors: string[];
  }
> = {
  "DC Comics": {
    writers: [
      "Frank Miller",
      "Scott Snyder",
      "Grant Morrison",
      "Geoff Johns",
      "Tom King",
    ],
    artists: [
      "David Mazzucchelli",
      "Greg Capullo",
      "Jim Lee",
      "Jason Fabok",
      "Mikel Janín",
    ],
    colorists: [
      "Richmond Lewis",
      "FCO Plascencia",
      "Alex Sinclair",
      "Brad Anderson",
      "Jordie Bellaire",
    ],
    letterers: [
      "Todd Klein",
      "Deron Bennett",
      "Clayton Cowles",
      "John Workman",
      "Sal Cipriano",
    ],
    editors: [
      "Dennis O'Neil",
      "Mark Doyle",
      "Ben Abernathy",
      "Chris Conroy",
      "Jamie S. Rich",
    ],
  },
  "Marvel Comics": {
    writers: [
      "Brian Michael Bendis",
      "Jonathan Hickman",
      "Ta-Nehisi Coates",
      "Jason Aaron",
      "Chip Zdarsky",
    ],
    artists: [
      "Alex Ross",
      "Esad Ribić",
      "Mike Deodato Jr.",
      "Stuart Immonen",
      "Marco Checchetto",
    ],
    colorists: [
      "Dean White",
      "Matthew Wilson",
      "Ive Svorcina",
      "Laura Martin",
      "Marte Gracia",
    ],
    letterers: [
      "Cory Petit",
      "Joe Sabino",
      "Clayton Cowles",
      "Travis Lanham",
      "Joe Caramagna",
    ],
    editors: [
      "Tom Brevoort",
      "Nick Lowe",
      "Jordan D. White",
      "Will Moss",
      "Devin Lewis",
    ],
  },
  "Image Comics": {
    writers: [
      "Brian K. Vaughan",
      "Robert Kirkman",
      "Ed Brubaker",
      "Jeff Lemire",
      "Rick Remender",
    ],
    artists: [
      "Fiona Staples",
      "Charlie Adlard",
      "Sean Phillips",
      "Andrea Sorrentino",
      "Jerome Opeña",
    ],
    colorists: [
      "Fiona Staples",
      "Cliff Rathburn",
      "Elizabeth Breitweiser",
      "Dave Stewart",
      "Matt Hollingsworth",
    ],
    letterers: [
      "Fonografiks",
      "Rus Wooton",
      "Sean Phillips",
      "Steve Wands",
      "Clem Robins",
    ],
    editors: [
      "Eric Stephenson",
      "Sean Mackiewicz",
      "Jon Moisan",
      "Briah Skelly",
      "Drew Gill",
    ],
  },
  "Shueisha / Viz Media": {
    writers: [
      "Eiichiro Oda",
      "Masashi Kishimoto",
      "Hajime Isayama",
      "Kohei Horikoshi",
      "Tatsuki Fujimoto",
    ],
    artists: [
      "Eiichiro Oda",
      "Masashi Kishimoto",
      "Hajime Isayama",
      "Kohei Horikoshi",
      "Tatsuki Fujimoto",
    ],
    colorists: [],
    letterers: [
      "Alexis Kirsch",
      "Erika Terriquez",
      "Steve Dutro",
      "John Hunt",
      "Evan Waldinger",
    ],
    editors: [
      "Alexis Kirsch",
      "Joel Enos",
      "Mike Montesa",
      "Hope Donovan",
      "Erica Yee",
    ],
  },
  Kodansha: {
    writers: [
      "Hajime Isayama",
      "Koyoharu Gotouge",
      "Gege Akutami",
      "Yoshihiro Togashi",
      "Naoki Urasawa",
    ],
    artists: [
      "Hajime Isayama",
      "Koyoharu Gotouge",
      "Gege Akutami",
      "Yoshihiro Togashi",
      "Naoki Urasawa",
    ],
    colorists: [],
    letterers: ["Steve Wands", "Evan Waldinger", "John Hunt", "Brandon Bovia"],
    editors: [
      "Ben Applegate",
      "Haruko Hashimoto",
      "Alethea Nibley",
      "Athena Nibley",
    ],
  },
};

// Mock books with errors for testing
const mockBooksWithErrors: Array<{
  book: (typeof mockBooks)[0];
  errors: Array<{
    errorType:
      | "format_detection"
      | "parser"
      | "metadata"
      | "thumbnail"
      | "page_extraction"
      | "pdf_rendering"
      | "other";
    message: string;
    details?: unknown;
    occurredAt: string;
  }>;
}> = [];

// Initialize mock books with errors (first 5 books have various errors)
function initMockBooksWithErrors() {
  if (mockBooksWithErrors.length > 0) return;

  const errorSamples: Array<{
    errorType:
      | "format_detection"
      | "parser"
      | "metadata"
      | "thumbnail"
      | "page_extraction"
      | "pdf_rendering"
      | "other";
    message: string;
    details?: unknown;
  }> = [
    {
      errorType: "parser",
      message: "Failed to parse CBZ: Invalid ZIP archive structure",
      details: { zipError: "Central directory not found" },
    },
    {
      errorType: "thumbnail",
      message: "Failed to generate thumbnail: No valid image found on page 1",
      details: { page: 1, reason: "Image extraction failed" },
    },
    {
      errorType: "metadata",
      message: "Failed to parse ComicInfo.xml: Invalid XML structure",
      details: { line: 15, column: 23 },
    },
    {
      errorType: "page_extraction",
      message: "Failed to extract page 5: Corrupted image data",
      details: { page: 5, format: "JPEG" },
    },
    {
      errorType: "pdf_rendering",
      message:
        "Page could not be extracted from PDF: no embedded image found and PDFium renderer is not available",
      details: { page: 1, pdfiumAvailable: false },
    },
    {
      errorType: "format_detection",
      message: "Unable to detect file format: Unsupported or corrupted file",
      details: { detectedMime: "application/octet-stream" },
    },
  ];

  // Add errors to first few books
  for (let i = 0; i < Math.min(5, mockBooks.length); i++) {
    const book = mockBooks[i];
    const errorIndex = i % errorSamples.length;
    const error = errorSamples[errorIndex];

    mockBooksWithErrors.push({
      book,
      errors: [
        {
          ...error,
          occurredAt: new Date(
            Date.now() - Math.random() * 7 * 24 * 60 * 60 * 1000,
          ).toISOString(),
        },
      ],
    });

    // Add a second error to some books
    if (i % 2 === 0 && i + 1 < errorSamples.length) {
      const secondError = errorSamples[(errorIndex + 1) % errorSamples.length];
      mockBooksWithErrors[mockBooksWithErrors.length - 1].errors.push({
        ...secondError,
        occurredAt: new Date(
          Date.now() - Math.random() * 3 * 24 * 60 * 60 * 1000,
        ).toISOString(),
      });
    }
  }
}

export const bookHandlers = [
  // IMPORTANT: Specific routes MUST come before parameterized routes
  // Otherwise /api/v1/books/:id will match "in-progress" as an ID

  // ============================================
  // Books with Errors endpoints (v2)
  // ============================================

  // List books with errors (grouped by error type, 1-indexed)
  http.get("/api/v1/books/errors", async ({ request }) => {
    await delay(200);
    initMockBooksWithErrors();

    const url = new URL(request.url);
    const page = Math.max(
      1,
      Number.parseInt(url.searchParams.get("page") || "1", 10),
    );
    const pageSize = Number.parseInt(
      url.searchParams.get("pageSize") || "50",
      10,
    );
    const errorTypeFilter = url.searchParams.get("errorType") as
      | "format_detection"
      | "parser"
      | "metadata"
      | "thumbnail"
      | "page_extraction"
      | "pdf_rendering"
      | "other"
      | null;
    const libraryId = url.searchParams.get("libraryId");

    // Filter by library if specified
    let filteredBooks = mockBooksWithErrors;
    if (libraryId) {
      filteredBooks = filteredBooks.filter(
        (b) => b.book.libraryId === libraryId,
      );
    }

    // Filter by error type if specified
    if (errorTypeFilter) {
      filteredBooks = filteredBooks.filter((b) =>
        b.errors.some((e) => e.errorType === errorTypeFilter),
      );
    }

    // Count errors by type
    const errorCounts: Record<string, number> = {};
    for (const bookWithError of filteredBooks) {
      for (const error of bookWithError.errors) {
        errorCounts[error.errorType] = (errorCounts[error.errorType] || 0) + 1;
      }
    }

    // Group books by error type
    const errorTypes = [
      "parser",
      "thumbnail",
      "metadata",
      "page_extraction",
      "pdf_rendering",
      "format_detection",
      "other",
    ] as const;

    const errorTypeLabels: Record<string, string> = {
      parser: "Parser Error",
      thumbnail: "Thumbnail Error",
      metadata: "Metadata Error",
      page_extraction: "Page Extraction Error",
      pdf_rendering: "PDF Rendering Error",
      format_detection: "Format Detection Error",
      other: "Other Error",
    };

    const groups = errorTypes
      .filter((type) => !errorTypeFilter || type === errorTypeFilter)
      .map((errorType) => {
        const booksWithThisError = filteredBooks.filter((b) =>
          b.errors.some((e) => e.errorType === errorType),
        );

        return {
          errorType,
          label: errorTypeLabels[errorType] || errorType,
          count: booksWithThisError.length,
          books: booksWithThisError.map((b) => ({
            book: b.book,
            errors: b.errors.filter((e) => e.errorType === errorType),
          })),
        };
      })
      .filter((g) => g.count > 0);

    // Paginate (for simplicity, pagination applies to total books)
    const totalBooks = filteredBooks.length;

    return HttpResponse.json({
      totalBooksWithErrors: totalBooks,
      totalPages: Math.ceil(totalBooks / pageSize),
      page,
      pageSize,
      errorCounts,
      groups,
    });
  }),

  // Retry specific book errors
  http.post("/api/v1/books/:bookId/retry", async ({ params, request }) => {
    await delay(200);
    initMockBooksWithErrors();

    const bookId = params.bookId as string;
    const bookWithErrors = mockBooksWithErrors.find(
      (b) => b.book.id === bookId,
    );

    if (!bookWithErrors || bookWithErrors.errors.length === 0) {
      return HttpResponse.json(
        { error: "Book has no errors", message: "Book has no errors to retry" },
        { status: 400 },
      );
    }

    const body = (await request.json().catch(() => ({}))) as {
      errorTypes?: string[];
    };
    const errorTypesToRetry = body.errorTypes || [];

    // Determine how many tasks to enqueue
    let tasksEnqueued = 0;
    const errorsToRetry =
      errorTypesToRetry.length > 0
        ? bookWithErrors.errors.filter((e) =>
            errorTypesToRetry.includes(e.errorType),
          )
        : bookWithErrors.errors;

    // Count unique task types needed
    const needsAnalysis = errorsToRetry.some((e) =>
      [
        "parser",
        "metadata",
        "page_extraction",
        "format_detection",
        "other",
      ].includes(e.errorType),
    );
    const needsThumbnail = errorsToRetry.some(
      (e) => e.errorType === "thumbnail" || e.errorType === "pdf_rendering",
    );

    if (needsAnalysis) tasksEnqueued++;
    if (needsThumbnail) tasksEnqueued++;

    return HttpResponse.json({
      message: `Enqueued ${tasksEnqueued} task(s) for book retry`,
      tasksEnqueued,
    });
  }),

  // Retry all book errors (bulk)
  http.post("/api/v1/books/retry-all-errors", async ({ request }) => {
    await delay(300);
    initMockBooksWithErrors();

    const body = (await request.json().catch(() => ({}))) as {
      errorType?: string;
      libraryId?: string;
    };

    let booksToRetry = mockBooksWithErrors;

    // Filter by library if specified
    if (body.libraryId) {
      booksToRetry = booksToRetry.filter(
        (b) => b.book.libraryId === body.libraryId,
      );
    }

    // Filter by error type if specified
    if (body.errorType) {
      booksToRetry = booksToRetry.filter((b) =>
        b.errors.some((e) => e.errorType === body.errorType),
      );
    }

    const tasksEnqueued = booksToRetry.length;

    return HttpResponse.json({
      message: `Enqueued ${tasksEnqueued} task(s) for bulk retry`,
      tasksEnqueued,
    });
  }),

  // List in-progress books
  // Supports ?libraryId= query param for library filtering
  // Supports ?full=true for full book response with metadata
  // Returns plain array (not paginated) - matches API expectation
  http.get("/api/v1/books/in-progress", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const libraryId = url.searchParams.get("libraryId");
    const full = url.searchParams.get("full") === "true";

    // Return books that have read progress
    const baseBooks = libraryId ? getBooksByLibrary(libraryId) : mockBooks;
    const inProgressBooks = baseBooks.filter((b) => b.readProgress !== null);

    if (full) {
      return HttpResponse.json(inProgressBooks.map(toFullBookResponse));
    }
    return HttpResponse.json(inProgressBooks);
  }),

  // List on-deck books
  // Supports ?libraryId= query param for library filtering
  // Returns paginated response with next book in series where user has completed books
  http.get("/api/v1/books/on-deck", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const libraryId = url.searchParams.get("libraryId");

    // Return books that don't have progress (simulating "next to read")
    // In reality this would be first unread book from series with completed books
    const baseBooks = libraryId ? getBooksByLibrary(libraryId) : mockBooks;
    const onDeckBooks = baseBooks
      .filter((b) => b.readProgress === null)
      .slice(0, 10);

    return HttpResponse.json(
      createPaginatedResponse(onDeckBooks, {
        total: onDeckBooks.length,
      }),
    );
  }),

  // List recently added books
  // Supports ?libraryId= query param for library filtering
  // Supports ?full=true for full book response with metadata
  // Returns plain array (not paginated) - matches API expectation
  http.get("/api/v1/books/recently-added", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const libraryId = url.searchParams.get("libraryId");
    const limit = Number.parseInt(url.searchParams.get("limit") || "50", 10);
    const full = url.searchParams.get("full") === "true";

    // Sort by created date (newest first)
    const baseBooks = libraryId ? getBooksByLibrary(libraryId) : mockBooks;
    const sortedBooks = [...baseBooks].sort(
      (a, b) =>
        new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime(),
    );

    const result = sortedBooks.slice(0, limit);
    if (full) {
      return HttpResponse.json(result.map(toFullBookResponse));
    }
    return HttpResponse.json(result);
  }),

  // List recently read books
  // Supports ?libraryId= query param for library filtering
  // Returns plain array (not paginated) - matches API expectation
  http.get("/api/v1/books/recently-read", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const libraryId = url.searchParams.get("libraryId");
    const limit = Number.parseInt(url.searchParams.get("limit") || "50", 10);

    // Return books that have been read (have read progress), sorted by last read
    const baseBooks = libraryId ? getBooksByLibrary(libraryId) : mockBooks;
    const readBooks = baseBooks
      .filter((b) => b.readProgress !== null)
      .slice(0, limit);

    return HttpResponse.json(readBooks);
  }),

  // POST /books/list - Advanced filtering with condition tree (1-indexed)
  // Pagination params come from query string, filter criteria from body
  // Supports ?full=true for full book response with metadata
  http.post("/api/v1/books/list", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const page = Math.max(
      1,
      Number.parseInt(url.searchParams.get("page") || "1", 10),
    );
    const pageSize = Number.parseInt(
      url.searchParams.get("pageSize") || "50",
      10,
    );
    const full = url.searchParams.get("full") === "true";

    const body = (await request.json()) as {
      condition?: unknown;
      fullTextSearch?: string;
      search?: string;
    };

    let results = [...mockBooks];

    // Apply condition-based filtering
    if (body.condition && typeof body.condition === "object") {
      const condition = body.condition as Record<string, unknown>;

      // Handle direct title condition (for search)
      if ("title" in condition) {
        const titleOp = condition.title as { operator: string; value: string };
        if (titleOp.operator === "contains") {
          const searchLower = titleOp.value.toLowerCase();
          results = results.filter((b) =>
            b.title.toLowerCase().includes(searchLower),
          );
        }
      }

      // Handle direct libraryId condition
      if ("libraryId" in condition) {
        const libOp = condition.libraryId as {
          operator: string;
          value: string;
        };
        if (libOp.operator === "is") {
          results = results.filter((b) => b.libraryId === libOp.value);
        }
      }

      // Handle allOf wrapper
      if ("allOf" in condition && Array.isArray(condition.allOf)) {
        for (const c of condition.allOf) {
          if (c && typeof c === "object") {
            const subCondition = c as Record<string, unknown>;
            if ("title" in subCondition) {
              const titleOp = subCondition.title as {
                operator: string;
                value: string;
              };
              if (titleOp.operator === "contains") {
                const searchLower = titleOp.value.toLowerCase();
                results = results.filter((b) =>
                  b.title.toLowerCase().includes(searchLower),
                );
              }
            }
            if ("libraryId" in subCondition) {
              const libOp = subCondition.libraryId as {
                operator: string;
                value: string;
              };
              if (libOp.operator === "is") {
                results = results.filter((b) => b.libraryId === libOp.value);
              }
            }
          }
        }
      }
    }

    // Apply full-text search (case-insensitive)
    if (body.fullTextSearch) {
      const searchLower = body.fullTextSearch.toLowerCase();
      results = results.filter((b) =>
        b.title.toLowerCase().includes(searchLower),
      );
    }

    // Legacy text search support
    if (body.search) {
      const searchLower = body.search.toLowerCase();
      results = results.filter((b) =>
        b.title.toLowerCase().includes(searchLower),
      );
    }

    // Paginate (1-indexed)
    const start = (page - 1) * pageSize;
    const end = start + pageSize;
    const items = results.slice(start, end);

    if (full) {
      const fullItems = items.map(toFullBookResponse);
      return HttpResponse.json(
        createPaginatedResponse(fullItems, {
          page,
          pageSize,
          total: results.length,
          basePath: "/api/v1/books/list",
        }),
      );
    }

    return HttpResponse.json(
      createPaginatedResponse(items, {
        page,
        pageSize,
        total: results.length,
        basePath: "/api/v1/books/list",
      }),
    );
  }),

  // List books with pagination (1-indexed)
  // Supports ?library_id= and ?series_id= query params for filtering
  // Supports ?full=true for full book response with metadata
  http.get("/api/v1/books", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const page = Math.max(
      1,
      Number.parseInt(url.searchParams.get("page") || "1", 10),
    );
    const pageSize = Number.parseInt(
      url.searchParams.get("pageSize") || "50",
      10,
    );
    const libraryId = url.searchParams.get("libraryId");
    const seriesId = url.searchParams.get("seriesId");
    const full = url.searchParams.get("full") === "true";

    let filteredBooks = libraryId ? getBooksByLibrary(libraryId) : mockBooks;
    if (seriesId) {
      filteredBooks = filteredBooks.filter((b) => b.seriesId === seriesId);
    }

    // 1-indexed pagination
    const start = (page - 1) * pageSize;
    const end = start + pageSize;
    const items = filteredBooks.slice(start, end);

    if (full) {
      const fullItems = items.map(toFullBookResponse);
      return HttpResponse.json(
        createPaginatedResponse(fullItems, {
          page,
          pageSize,
          total: filteredBooks.length,
          basePath: "/api/v1/books",
        }),
      );
    }

    return HttpResponse.json(
      createPaginatedResponse(items, {
        page,
        pageSize,
        total: filteredBooks.length,
        basePath: "/api/v1/books",
      }),
    );
  }),

  // Get book by ID (must come AFTER specific routes like /in-progress, /recently-added)
  // Supports ?full=true for full book response with metadata
  http.get("/api/v1/books/:id", async ({ params, request }) => {
    await delay(100);
    const url = new URL(request.url);
    const full = url.searchParams.get("full") === "true";
    const book = mockBooks.find((b) => b.id === params.id);

    if (!book) {
      return HttpResponse.json({ error: "Book not found" }, { status: 404 });
    }

    // If full=true, return FullBookResponse
    if (full) {
      return HttpResponse.json(toFullBookResponse(book));
    }

    // Otherwise, return BookDetailResponse (book + metadata)
    // Determine publisher based on series
    const publisherMap: Record<string, string> = {
      "Batman: Year One": "DC Comics",
      "Batman: The Dark Knight Returns": "DC Comics",
      "Spider-Man: Blue": "Marvel Comics",
      "One Piece": "Shueisha / Viz Media",
      Naruto: "Shueisha / Viz Media",
      "Attack on Titan": "Kodansha",
      Saga: "Image Comics",
      "The Walking Dead": "Image Comics",
      Sandman: "DC Comics",
    };
    const publisher = publisherMap[book.seriesName] || "DC Comics";

    // Get creative team based on publisher
    const team = creativeTeams[publisher] || creativeTeams["DC Comics"];
    const writerIndex = (book.number ?? 1) % team.writers.length;
    const artistIndex = (book.number ?? 1) % team.artists.length;

    // Determine genre based on series type
    const genreMap: Record<string, string> = {
      "Batman: Year One": "Superhero / Crime",
      "One Piece": "Action / Adventure",
      "Attack on Titan": "Dark Fantasy / Action",
      Saga: "Science Fiction / Fantasy",
      "The Walking Dead": "Horror / Drama",
      Sandman: "Fantasy / Horror",
      Naruto: "Action / Martial Arts",
    };
    const genre = genreMap[book.seriesName] || "Superhero";

    return HttpResponse.json({
      book,
      metadata: {
        id: book.id,
        bookId: book.id,
        title: book.title,
        series: book.seriesName,
        number: book.number?.toString(),
        summary: getBookSummary(book.seriesName, book.number ?? null),
        publisher,
        imprint: publisher.includes("Vertigo") ? "Vertigo" : null,
        genre,
        pageCount: book.pageCount,
        languageIso:
          publisher.includes("Viz") || publisher.includes("Kodansha")
            ? "ja"
            : "en",
        releaseDate: null,
        writers: [team.writers[writerIndex]],
        pencillers: [team.artists[artistIndex]],
        inkers: [team.artists[artistIndex]],
        colorists:
          team.colorists.length > 0
            ? [team.colorists[writerIndex % team.colorists.length]]
            : [],
        letterers: [team.letterers[writerIndex % team.letterers.length]],
        coverArtists: [team.artists[artistIndex]],
        editors: [team.editors[writerIndex % team.editors.length]],
      },
    });
  }),

  // Get book thumbnail
  http.get("/api/v1/books/:id/thumbnail", async () => {
    await delay(50);
    // Return the cover SVG as an image response
    return new HttpResponse(coverSvg, {
      headers: {
        "Content-Type": "image/svg+xml",
      },
    });
  }),

  // Get book page image
  http.get("/api/v1/books/:id/pages/:pageNum", async () => {
    await delay(100);
    // Return the page SVG as an image response
    return new HttpResponse(pageSvg, {
      headers: {
        "Content-Type": "image/svg+xml",
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
    const next =
      currentIndex < seriesBooks.length - 1
        ? seriesBooks[currentIndex + 1]
        : null;

    return HttpResponse.json({ prev, next });
  }),

  // Generate thumbnail for a book (queues a background task)
  http.post("/api/v1/books/:id/thumbnail/generate", async () => {
    await delay(100);
    return HttpResponse.json({ task_id: crypto.randomUUID() });
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

  // ============================================
  // Read Progress
  // ============================================

  // Get read progress for book
  http.get("/api/v1/books/:id/progress", async ({ params }) => {
    await delay(100);
    const book = mockBooks.find((b) => b.id === params.id);

    if (!book) {
      return HttpResponse.json({ error: "Book not found" }, { status: 404 });
    }

    if (!book.readProgress) {
      // No progress exists — return null with 200
      return HttpResponse.json(null);
    }

    const currentPage = book.readProgress.currentPage;
    return HttpResponse.json({
      id: `progress-${params.id}`,
      bookId: params.id,
      userId: "mock-user-id",
      currentPage,
      totalPages: book.pageCount,
      percentage: Math.round((currentPage / book.pageCount) * 100),
      isCompleted: currentPage >= book.pageCount,
      lastReadAt: new Date().toISOString(),
      createdAt: "2024-01-01T00:00:00Z",
      updatedAt: new Date().toISOString(),
    });
  }),

  // Update read progress for book
  http.put("/api/v1/books/:id/progress", async ({ params, request }) => {
    await delay(100);
    const book = mockBooks.find((b) => b.id === params.id);

    if (!book) {
      return HttpResponse.json({ error: "Book not found" }, { status: 404 });
    }

    const body = (await request.json()) as {
      currentPage: number;
      isCompleted?: boolean;
    };
    const now = new Date().toISOString();

    return HttpResponse.json({
      id: `progress-${params.id}`,
      bookId: params.id,
      userId: "mock-user-id",
      currentPage: body.currentPage,
      totalPages: book.pageCount,
      percentage: Math.round((body.currentPage / book.pageCount) * 100),
      isCompleted: body.isCompleted ?? body.currentPage >= book.pageCount,
      lastReadAt: now,
      createdAt: "2024-01-01T00:00:00Z",
      updatedAt: now,
    });
  }),

  // Delete read progress for book
  http.delete("/api/v1/books/:id/progress", async () => {
    await delay(50);
    return new HttpResponse(null, { status: 204 });
  }),

  // ============================================
  // Book Metadata
  // ============================================

  // PATCH book metadata
  http.patch("/api/v1/books/:id/metadata", async ({ params, request }) => {
    await delay(100);
    const book = mockBooks.find((b) => b.id === params.id);

    if (!book) {
      return HttpResponse.json({ error: "Book not found" }, { status: 404 });
    }

    const body = (await request.json()) as Record<string, unknown>;

    return HttpResponse.json({
      id: book.id,
      bookId: book.id,
      title: body.title ?? book.title,
      series: body.series ?? book.seriesName,
      number: body.number ?? book.number?.toString(),
      summary: body.summary ?? null,
      publisher: body.publisher ?? null,
      imprint: body.imprint ?? null,
      genre: body.genre ?? null,
      pageCount: book.pageCount,
      languageIso: body.languageIso ?? "en",
      releaseDate: body.releaseDate ?? null,
      writers: body.writers ?? [],
      pencillers: body.pencillers ?? [],
      inkers: body.inkers ?? [],
      colorists: body.colorists ?? [],
      letterers: body.letterers ?? [],
      coverArtists: body.coverArtists ?? [],
      editors: body.editors ?? [],
      updatedAt: new Date().toISOString(),
    });
  }),

  // PATCH book core fields (title, number)
  http.patch("/api/v1/books/:id", async ({ params, request }) => {
    await delay(100);
    const book = mockBooks.find((b) => b.id === params.id);

    if (!book) {
      return HttpResponse.json({ error: "Book not found" }, { status: 404 });
    }

    const body = (await request.json()) as { title?: string; number?: number };

    return HttpResponse.json({
      ...book,
      title: body.title ?? book.title,
      number: body.number ?? book.number,
      updatedAt: new Date().toISOString(),
    });
  }),

  // Get book metadata locks
  http.get("/api/v1/books/:id/metadata/locks", async ({ params }) => {
    await delay(100);
    const book = mockBooks.find((b) => b.id === params.id);

    if (!book) {
      return HttpResponse.json({ error: "Book not found" }, { status: 404 });
    }

    // Return all locks as false by default
    return HttpResponse.json({
      title: false,
      series: false,
      number: false,
      summary: false,
      publisher: false,
      imprint: false,
      genre: false,
      languageIso: false,
      releaseDate: false,
      writers: false,
      pencillers: false,
      inkers: false,
      colorists: false,
      letterers: false,
      coverArtists: false,
      editors: false,
    });
  }),

  // Update book metadata locks
  http.put("/api/v1/books/:id/metadata/locks", async ({ params, request }) => {
    await delay(100);
    const book = mockBooks.find((b) => b.id === params.id);

    if (!book) {
      return HttpResponse.json({ error: "Book not found" }, { status: 404 });
    }

    const body = (await request.json()) as Record<string, boolean>;

    return HttpResponse.json({
      title: body.title ?? false,
      series: body.series ?? false,
      number: body.number ?? false,
      summary: body.summary ?? false,
      publisher: body.publisher ?? false,
      imprint: body.imprint ?? false,
      genre: body.genre ?? false,
      languageIso: body.languageIso ?? false,
      releaseDate: body.releaseDate ?? false,
      writers: body.writers ?? false,
      pencillers: body.pencillers ?? false,
      inkers: body.inkers ?? false,
      colorists: body.colorists ?? false,
      letterers: body.letterers ?? false,
      coverArtists: body.coverArtists ?? false,
      editors: body.editors ?? false,
    });
  }),

  // Analyze unanalyzed book
  http.post("/api/v1/books/:id/analyze-unanalyzed", async () => {
    await delay(100);
    return HttpResponse.json({
      message: "Analysis queued for unanalyzed pages",
    });
  }),

  // Upload book cover
  http.post("/api/v1/books/:id/cover", async ({ params }) => {
    await delay(200);
    return HttpResponse.json({
      id: `cover-${params.id}-custom-${Date.now()}`,
      bookId: params.id,
      source: "custom",
      isSelected: true,
      createdAt: new Date().toISOString(),
    });
  }),

  // Download book file
  http.get("/api/v1/books/:id/file", async ({ params }) => {
    await delay(100);
    const book = mockBooks.find((b) => b.id === params.id);

    if (!book) {
      return HttpResponse.json({ error: "Book not found" }, { status: 404 });
    }

    // Map file format to fixture URL and content type
    const format = book.fileFormat.toLowerCase();
    let fixtureUrl: string;
    let contentType: string;
    let filename: string;

    switch (format) {
      case "epub":
        fixtureUrl = sampleEpubUrl;
        contentType = "application/epub+zip";
        filename = `${book.title}.epub`;
        break;
      case "pdf":
        fixtureUrl = samplePdfUrl;
        contentType = "application/pdf";
        filename = `${book.title}.pdf`;
        break;
      default:
        fixtureUrl = sampleCbzUrl;
        contentType = "application/zip";
        filename = `${book.title}.cbz`;
        break;
    }

    // Fetch the fixture file and return it
    const response = await fetch(fixtureUrl);
    const blob = await response.blob();

    return new HttpResponse(blob, {
      headers: {
        "Content-Type": contentType,
        "Content-Disposition": `attachment; filename="${filename}"`,
      },
    });
  }),

  // List books by series
  // Supports ?full=true for full book response with metadata
  // Returns plain array (not paginated) - matches API expectation
  http.get("/api/v1/series/:seriesId/books", async ({ params, request }) => {
    await delay(200);
    const url = new URL(request.url);
    const full = url.searchParams.get("full") === "true";

    const filteredBooks = getBooksBySeries(params.seriesId as string);

    if (full) {
      return HttpResponse.json(filteredBooks.map(toFullBookResponse));
    }
    return HttpResponse.json(filteredBooks);
  }),

  // List books by library (1-indexed)
  // Supports ?full=true for full book response with metadata
  http.get(
    "/api/v1/libraries/:libraryId/books",
    async ({ params, request }) => {
      await delay(200);
      const url = new URL(request.url);
      const page = Math.max(
        1,
        Number.parseInt(url.searchParams.get("page") || "1", 10),
      );
      const pageSize = Number.parseInt(
        url.searchParams.get("pageSize") || "50",
        10,
      );
      const full = url.searchParams.get("full") === "true";

      const libraryBooks = getBooksByLibrary(params.libraryId as string);
      // 1-indexed pagination
      const start = (page - 1) * pageSize;
      const end = start + pageSize;
      const items = libraryBooks.slice(start, end);

      if (full) {
        const fullItems = items.map(toFullBookResponse);
        return HttpResponse.json(
          createPaginatedResponse(fullItems, {
            page,
            pageSize,
            total: libraryBooks.length,
            basePath: `/api/v1/libraries/${params.libraryId}/books`,
          }),
        );
      }

      return HttpResponse.json(
        createPaginatedResponse(items, {
          page,
          pageSize,
          total: libraryBooks.length,
          basePath: `/api/v1/libraries/${params.libraryId}/books`,
        }),
      );
    },
  ),

  // Library-scoped: List in-progress books
  // Supports ?full=true for full book response with metadata
  // Returns plain array (not paginated) - matches API expectation
  http.get(
    "/api/v1/libraries/:libraryId/books/in-progress",
    async ({ params, request }) => {
      await delay(200);
      const url = new URL(request.url);
      const full = url.searchParams.get("full") === "true";

      // Get books for this library that have read progress
      const libraryBooks = getBooksByLibrary(params.libraryId as string);
      const inProgressBooks = libraryBooks.filter(
        (b) => b.readProgress !== null,
      );

      if (full) {
        return HttpResponse.json(inProgressBooks.map(toFullBookResponse));
      }
      return HttpResponse.json(inProgressBooks);
    },
  ),

  // Library-scoped: List recently added books
  // Supports ?full=true for full book response with metadata
  // Returns plain array (not paginated) - matches API expectation
  http.get(
    "/api/v1/libraries/:libraryId/books/recently-added",
    async ({ params, request }) => {
      await delay(200);
      const url = new URL(request.url);
      const limit = Number.parseInt(url.searchParams.get("limit") || "50", 10);
      const full = url.searchParams.get("full") === "true";

      // Get books for this library, sorted by created date
      const libraryBooks = getBooksByLibrary(params.libraryId as string);
      const sortedBooks = [...libraryBooks].sort(
        (a, b) =>
          new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime(),
      );

      const result = sortedBooks.slice(0, limit);
      if (full) {
        return HttpResponse.json(result.map(toFullBookResponse));
      }
      return HttpResponse.json(result);
    },
  ),

  // Library-scoped: List on-deck books
  // Returns paginated response
  http.get("/api/v1/libraries/:libraryId/books/on-deck", async ({ params }) => {
    await delay(200);

    // Get books for this library that don't have progress
    const libraryBooks = getBooksByLibrary(params.libraryId as string);
    const onDeckBooks = libraryBooks
      .filter((b) => b.readProgress === null)
      .slice(0, 10);

    return HttpResponse.json(
      createPaginatedResponse(onDeckBooks, {
        total: onDeckBooks.length,
      }),
    );
  }),
];

/**
 * Convert a BookDto to FullBookResponse format
 * Used when ?full=true query parameter is specified
 */
function toFullBookResponse(book: (typeof mockBooks)[0]) {
  // Determine publisher based on series
  const publisherMap: Record<string, string> = {
    "Batman: Year One": "DC Comics",
    "Batman: The Dark Knight Returns": "DC Comics",
    "Spider-Man: Blue": "Marvel Comics",
    "One Piece": "Shueisha / Viz Media",
    Naruto: "Shueisha / Viz Media",
    "Attack on Titan": "Kodansha",
    Saga: "Image Comics",
    "The Walking Dead": "Image Comics",
    Sandman: "DC Comics",
  };
  const publisher = publisherMap[book.seriesName] || "DC Comics";

  // Get creative team based on publisher
  const team = creativeTeams[publisher] || creativeTeams["DC Comics"];
  const writerIndex = (book.number ?? 1) % team.writers.length;
  const artistIndex = (book.number ?? 1) % team.artists.length;

  // Determine language based on publisher
  const isJapanese =
    publisher.includes("Viz") || publisher.includes("Kodansha");
  const language = isJapanese ? "ja" : "en";

  return {
    id: book.id,
    seriesId: book.seriesId,
    seriesName: book.seriesName,
    libraryId: book.libraryId,
    libraryName: book.libraryName || "Unknown Library",
    number: book.number,
    pageCount: book.pageCount,
    filePath: `/media/comics/${book.seriesName}/${book.title}.${book.fileFormat}`,
    fileSize: book.fileSize || 52428800,
    fileFormat: book.fileFormat,
    fileHash: book.fileHash || `hash-${book.id}`,
    deleted: book.deleted || false,
    analysisError: null,
    readingDirection: isJapanese ? "rtl" : "ltr",
    readProgress: book.readProgress,
    createdAt: book.createdAt,
    updatedAt: book.updatedAt,
    metadata: {
      title: book.title,
      series: book.seriesName,
      number: book.number?.toString() ?? null,
      summary: getBookSummary(book.seriesName, book.number ?? null),
      publisher,
      imprint: publisher.includes("Vertigo") ? "Vertigo" : null,
      genre: null,
      releaseDate: null,
      pageCount: book.pageCount,
      languageIso: language,
      writers: [team.writers[writerIndex]],
      pencillers: [team.artists[artistIndex]],
      inkers: [team.artists[artistIndex]],
      colorists:
        team.colorists.length > 0
          ? [team.colorists[writerIndex % team.colorists.length]]
          : [],
      letterers: [team.letterers[writerIndex % team.letterers.length]],
      coverArtists: [team.artists[artistIndex]],
      editors: [team.editors[writerIndex % team.editors.length]],
      customMetadata: null,
      locks: {
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
        pageCountLock: false,
        languageIsoLock: false,
        releaseDateLock: false,
        seriesLock: false,
        numberLock: false,
        titleLock: false,
        customMetadataLock: false,
      },
    },
  };
}

// Helper to get current mock books (for testing)
export const getMockBooks = () => [...mockBooks];
