/**
 * Series API mock handlers
 */

import { http, HttpResponse, delay } from "msw";
import { createPaginatedResponse } from "../data/factories";
import { mockSeries, getSeriesByLibrary } from "../data/store";

export const seriesHandlers = [
  // IMPORTANT: Specific routes MUST come before parameterized routes
  // Otherwise /api/v1/series/:id will match "started" or "search" as an ID

  // Search series
  http.get("/api/v1/series/search", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const query = url.searchParams.get("q")?.toLowerCase() || "";
    const libraryId = url.searchParams.get("libraryId");

    let results = mockSeries.filter((s) => s.name.toLowerCase().includes(query));

    if (libraryId) {
      results = results.filter((s) => s.libraryId === libraryId);
    }

    return HttpResponse.json(
      createPaginatedResponse(results.slice(0, 20), {
        total: results.length,
      })
    );
  }),

  // List in-progress series (global - all libraries)
  // Returns plain array (not paginated) - matches API expectation
  http.get("/api/v1/series/in-progress", async () => {
    await delay(200);

    // Return a subset as "in-progress" series (those with reading progress)
    const inProgressSeries = mockSeries.slice(0, 5);

    return HttpResponse.json(inProgressSeries);
  }),

  // List series with pagination
  http.get("/api/v1/series", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const page = Number.parseInt(url.searchParams.get("page") || "0");
    const pageSize = Number.parseInt(url.searchParams.get("pageSize") || "20");
    const libraryId = url.searchParams.get("libraryId");

    const filteredSeries = libraryId
      ? getSeriesByLibrary(libraryId)
      : mockSeries;

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

  // Get series by ID (must come AFTER specific routes like /started, /search)
  http.get("/api/v1/series/:id", async ({ params }) => {
    await delay(100);
    const seriesItem = mockSeries.find((s) => s.id === params.id);

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
  http.get(
    "/api/v1/libraries/:libraryId/series",
    async ({ params, request }) => {
      await delay(200);
      const url = new URL(request.url);
      const page = Number.parseInt(url.searchParams.get("page") || "0");
      const pageSize = Number.parseInt(url.searchParams.get("pageSize") || "20");

      const filteredSeries = getSeriesByLibrary(params.libraryId as string);
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
    }
  ),

  // Library-scoped: List in-progress series
  http.get("/api/v1/libraries/:libraryId/series/in-progress", async ({ params }) => {
    await delay(200);

    // Return a subset of in-progress series for this library
    const librarySeries = getSeriesByLibrary(params.libraryId as string);
    const inProgressSeries = librarySeries.slice(0, 5);

    return HttpResponse.json(inProgressSeries);
  }),
];

// Helper to get current mock series (for testing)
export const getMockSeries = () => [...mockSeries];
