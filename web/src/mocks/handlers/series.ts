/**
 * Series API mock handlers
 */

import { http, HttpResponse, delay } from "msw";
import {
  createSeries,
  createList,
  createPaginatedResponse,
  type MockSeries,
} from "../data/factories";

// In-memory mock data store
let series: MockSeries[] = createList(
  (i) =>
    createSeries({
      name: [
        "Batman: Year One",
        "Spider-Man",
        "Saga",
        "The Walking Dead",
        "One Piece",
        "Attack on Titan",
        "Sandman",
        "Watchmen",
        "X-Men",
        "Superman",
      ][i % 10],
    }),
  25
);

export const seriesHandlers = [
  // List series with pagination
  http.get("/api/v1/series", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const page = Number.parseInt(url.searchParams.get("page") || "0");
    const pageSize = Number.parseInt(url.searchParams.get("pageSize") || "20");
    const libraryId = url.searchParams.get("libraryId");

    let filteredSeries = series;
    if (libraryId) {
      filteredSeries = series.filter((s) => s.libraryId === libraryId);
    }

    const start = page * pageSize;
    const end = start + pageSize;
    const items = filteredSeries.slice(start, end);

    return HttpResponse.json(
      createPaginatedResponse(items, {
        page,
        pageSize,
        total: filteredSeries.length,
      })
    );
  }),

  // Search series
  http.get("/api/v1/series/search", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const query = url.searchParams.get("q")?.toLowerCase() || "";
    const libraryId = url.searchParams.get("libraryId");

    let results = series.filter((s) => s.name.toLowerCase().includes(query));

    if (libraryId) {
      results = results.filter((s) => s.libraryId === libraryId);
    }

    return HttpResponse.json(
      createPaginatedResponse(results.slice(0, 20), {
        total: results.length,
      })
    );
  }),

  // Get series by ID
  http.get("/api/v1/series/:id", async ({ params }) => {
    await delay(100);
    const seriesItem = series.find((s) => s.id === params.id);

    if (!seriesItem) {
      return HttpResponse.json({ error: "Series not found" }, { status: 404 });
    }

    return HttpResponse.json(seriesItem);
  }),

  // Get series thumbnail
  http.get("/api/v1/series/:id/thumbnail", async () => {
    await delay(50);
    // Return a placeholder image response
    return new HttpResponse(null, {
      status: 302,
      headers: {
        Location: "https://placehold.co/300x450/333/fff?text=Cover",
      },
    });
  }),

  // List series by library
  http.get("/api/v1/libraries/:libraryId/series", async ({ params, request }) => {
    await delay(200);
    const url = new URL(request.url);
    const page = Number.parseInt(url.searchParams.get("page") || "0");
    const pageSize = Number.parseInt(url.searchParams.get("pageSize") || "20");

    const filteredSeries = series.filter((s) => s.libraryId === params.libraryId);
    const start = page * pageSize;
    const end = start + pageSize;
    const items = filteredSeries.slice(start, end);

    return HttpResponse.json(
      createPaginatedResponse(items, {
        page,
        pageSize,
        total: filteredSeries.length,
      })
    );
  }),

  // List started series (with read progress)
  http.get("/api/v1/series/started", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const page = Number.parseInt(url.searchParams.get("page") || "0");
    const pageSize = Number.parseInt(url.searchParams.get("pageSize") || "20");

    // Return a subset as "started"
    const startedSeries = series.slice(0, 5);
    const start = page * pageSize;
    const end = start + pageSize;
    const items = startedSeries.slice(start, end);

    return HttpResponse.json(
      createPaginatedResponse(items, {
        page,
        pageSize,
        total: startedSeries.length,
      })
    );
  }),
];

// Helper to reset mock data (for testing)
export const resetMockSeries = () => {
  series = createList(
    (i) =>
      createSeries({
        name: [
          "Batman: Year One",
          "Spider-Man",
          "Saga",
          "The Walking Dead",
          "One Piece",
          "Attack on Titan",
          "Sandman",
          "Watchmen",
          "X-Men",
          "Superman",
        ][i % 10],
      }),
    25
  );
};

// Helper to get current mock series (for testing)
export const getMockSeries = () => [...series];
