/**
 * Series API mock handlers
 */

import { http, HttpResponse, delay } from "msw";
import { createPaginatedResponse } from "../data/factories";
import { mockSeries, getSeriesByLibrary } from "../data/store";

export const seriesHandlers = [
  // IMPORTANT: Specific routes MUST come before parameterized routes
  // Otherwise /api/v1/series/:id will match "started" or "search" as an ID

  // Search series (GET - legacy)
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

  // POST /series/list - Advanced filtering with condition tree
  http.post("/api/v1/series/list", async ({ request }) => {
    await delay(200);
    const body = await request.json() as {
      condition?: unknown;
      search?: string;
      page?: number;
      pageSize?: number;
      sort?: string;
    };

    const page = body.page ?? 0;
    const pageSize = body.pageSize ?? 20;

    // For mock purposes, we'll do basic filtering
    // In a real implementation, the backend evaluates the full condition tree
    let results = [...mockSeries];

    // Apply basic library filtering if condition contains libraryId
    if (body.condition && typeof body.condition === 'object') {
      const condition = body.condition as Record<string, unknown>;

      // Handle direct libraryId condition
      if ('libraryId' in condition) {
        const libOp = condition.libraryId as { operator: string; value: string };
        if (libOp.operator === 'is') {
          results = results.filter(s => s.libraryId === libOp.value);
        }
      }

      // Handle allOf wrapper with libraryId
      if ('allOf' in condition && Array.isArray(condition.allOf)) {
        for (const c of condition.allOf) {
          if (c && typeof c === 'object' && 'libraryId' in c) {
            const libOp = (c as Record<string, unknown>).libraryId as { operator: string; value: string };
            if (libOp.operator === 'is') {
              results = results.filter(s => s.libraryId === libOp.value);
            }
          }
        }
      }
    }

    // Apply text search
    if (body.search) {
      const searchLower = body.search.toLowerCase();
      results = results.filter(s => s.name.toLowerCase().includes(searchLower));
    }

    // Apply sorting
    if (body.sort) {
      const [field, direction] = body.sort.split(',');
      results.sort((a, b) => {
        const aVal = (a as Record<string, unknown>)[field];
        const bVal = (b as Record<string, unknown>)[field];
        if (typeof aVal === 'string' && typeof bVal === 'string') {
          return direction === 'desc' ? bVal.localeCompare(aVal) : aVal.localeCompare(bVal);
        }
        return 0;
      });
    }

    // Paginate
    const start = page * pageSize;
    const end = start + pageSize;
    const items = results.slice(start, end);

    return HttpResponse.json(
      createPaginatedResponse(items, {
        page,
        pageSize,
        total: results.length,
      })
    );
  }),

  // List in-progress series
  // Supports ?library_id= query param for library filtering
  // Returns plain array (not paginated) - matches API expectation
  http.get("/api/v1/series/in-progress", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const libraryId = url.searchParams.get("library_id");

    // Return a subset as "in-progress" series (those with reading progress)
    const baseSeries = libraryId ? getSeriesByLibrary(libraryId) : mockSeries;
    const inProgressSeries = baseSeries.slice(0, 5);

    return HttpResponse.json(inProgressSeries);
  }),

  // List series with pagination
  // Supports both ?library_id= (new) and ?libraryId= (legacy) for library filtering
  http.get("/api/v1/series", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const page = Number.parseInt(url.searchParams.get("page") || "0");
    const pageSize = Number.parseInt(url.searchParams.get("page_size") || url.searchParams.get("pageSize") || "20");
    // Support both library_id (new) and libraryId (legacy)
    const libraryId = url.searchParams.get("library_id") || url.searchParams.get("libraryId");

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

  // List recently added series
  // Supports ?library_id= query param for library filtering
  http.get("/api/v1/series/recently-added", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const libraryId = url.searchParams.get("library_id");
    const limit = Number.parseInt(url.searchParams.get("limit") || "50");

    const baseSeries = libraryId ? getSeriesByLibrary(libraryId) : mockSeries;
    // Sort by createdAt desc and limit
    const recentSeries = [...baseSeries]
      .sort((a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime())
      .slice(0, limit);

    return HttpResponse.json(recentSeries);
  }),

  // List recently updated series
  // Supports ?library_id= query param for library filtering
  http.get("/api/v1/series/recently-updated", async ({ request }) => {
    await delay(200);
    const url = new URL(request.url);
    const libraryId = url.searchParams.get("library_id");
    const limit = Number.parseInt(url.searchParams.get("limit") || "50");

    const baseSeries = libraryId ? getSeriesByLibrary(libraryId) : mockSeries;
    // Sort by updatedAt desc and limit
    const recentSeries = [...baseSeries]
      .sort((a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime())
      .slice(0, limit);

    return HttpResponse.json(recentSeries);
  }),

  // Get series by ID (must come AFTER specific routes like /in-progress, /recently-added, etc.)
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
