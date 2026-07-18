/**
 * Collections API mock handlers
 *
 * In-memory collections built from the shared series store so the
 * collection pages work in mock mode (list, detail, sorting, membership
 * edits, manual reordering).
 */

import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";
import { seededUuid } from "../data/factories";
import { mockSeries } from "../data/store";

type CollectionDto = components["schemas"]["CollectionDto"];
type CreateCollectionRequest = components["schemas"]["CreateCollectionRequest"];
type UpdateCollectionRequest = components["schemas"]["UpdateCollectionRequest"];

interface MockCollection {
  id: string;
  name: string;
  summary: string | null;
  ordered: boolean;
  seriesIds: string[];
  createdAt: string;
  updatedAt: string;
}

const favoriteSeriesIds = mockSeries
  .filter((s) => s.libraryName === "Manga")
  .slice(0, 10)
  .map((s) => s.id);

const batmanSeriesIds = mockSeries
  .filter((s) => s.title.startsWith("Batman"))
  .map((s) => s.id);

let mockCollections: MockCollection[] = [
  {
    id: seededUuid("collection-favorites"),
    name: "Favorites",
    summary: null,
    ordered: false,
    seriesIds: favoriteSeriesIds,
    createdAt: "2024-01-10T10:00:00Z",
    updatedAt: "2024-06-01T10:00:00Z",
  },
  {
    id: seededUuid("collection-batman"),
    name: "Batman Reading Order",
    summary: "The essential Batman arcs, in reading order.",
    ordered: true,
    seriesIds: batmanSeriesIds,
    createdAt: "2024-02-15T10:00:00Z",
    updatedAt: "2024-05-20T10:00:00Z",
  },
];

const toDto = (collection: MockCollection): CollectionDto => ({
  id: collection.id,
  name: collection.name,
  summary: collection.summary,
  ordered: collection.ordered,
  seriesCount: collection.seriesIds.length,
  createdAt: collection.createdAt,
  updatedAt: collection.updatedAt,
});

export const collectionsHandlers = [
  // List collections
  http.get("/api/v1/collections", async () => {
    await delay(150);
    return HttpResponse.json({
      items: mockCollections.map(toDto),
      total: mockCollections.length,
    });
  }),

  // Get collection by ID
  http.get("/api/v1/collections/:id", async ({ params }) => {
    await delay(100);
    const collection = mockCollections.find((c) => c.id === params.id);
    if (!collection) {
      return HttpResponse.json(
        { error: "Collection not found" },
        { status: 404 },
      );
    }
    return HttpResponse.json(toDto(collection));
  }),

  // Member series, sorted like the real API (manual keeps stored order)
  http.get("/api/v1/collections/:id/series", async ({ params, request }) => {
    await delay(150);
    const collection = mockCollections.find((c) => c.id === params.id);
    if (!collection) {
      return HttpResponse.json(
        { error: "Collection not found" },
        { status: 404 },
      );
    }

    const url = new URL(request.url);
    const sort =
      url.searchParams.get("sort") ?? (collection.ordered ? "manual" : "title");
    const direction = url.searchParams.get("direction") ?? "asc";

    const members = collection.seriesIds
      .map((id) => mockSeries.find((s) => s.id === id))
      .filter((s) => s !== undefined);

    if (sort !== "manual") {
      members.sort((a, b) => {
        switch (sort) {
          case "added":
            return a.createdAt.localeCompare(b.createdAt);
          case "year":
            return (a.year ?? 0) - (b.year ?? 0);
          default:
            return (a.titleSort ?? a.title).localeCompare(
              b.titleSort ?? b.title,
            );
        }
      });
      if (direction === "desc") members.reverse();
    }

    return HttpResponse.json(members);
  }),

  // Create collection
  http.post("/api/v1/collections", async ({ request }) => {
    await delay(200);
    const body = (await request.json()) as CreateCollectionRequest;
    const collection: MockCollection = {
      id: seededUuid(`collection-${body.name}-${mockCollections.length}`),
      name: body.name,
      summary: body.summary ?? null,
      ordered: body.ordered ?? false,
      seriesIds: [],
      createdAt: "2024-06-15T10:00:00Z",
      updatedAt: "2024-06-15T10:00:00Z",
    };
    mockCollections.push(collection);
    return HttpResponse.json(toDto(collection), { status: 201 });
  }),

  // Update collection
  http.patch("/api/v1/collections/:id", async ({ params, request }) => {
    await delay(200);
    const collection = mockCollections.find((c) => c.id === params.id);
    if (!collection) {
      return HttpResponse.json(
        { error: "Collection not found" },
        { status: 404 },
      );
    }
    const body = (await request.json()) as UpdateCollectionRequest;
    if (body.name != null) collection.name = body.name;
    if (body.summary !== undefined) collection.summary = body.summary;
    if (body.ordered != null) collection.ordered = body.ordered;
    return HttpResponse.json(toDto(collection));
  }),

  // Delete collection
  http.delete("/api/v1/collections/:id", async ({ params }) => {
    await delay(200);
    mockCollections = mockCollections.filter((c) => c.id !== params.id);
    return new HttpResponse(null, { status: 204 });
  }),

  // Add series to collection
  http.post("/api/v1/collections/:id/series", async ({ params, request }) => {
    await delay(200);
    const collection = mockCollections.find((c) => c.id === params.id);
    if (!collection) {
      return HttpResponse.json(
        { error: "Collection not found" },
        { status: 404 },
      );
    }
    const body = (await request.json()) as { seriesIds: string[] };
    for (const seriesId of body.seriesIds) {
      if (!collection.seriesIds.includes(seriesId)) {
        collection.seriesIds.push(seriesId);
      }
    }
    return HttpResponse.json(toDto(collection));
  }),

  // Remove series from collection
  http.delete(
    "/api/v1/collections/:id/series/:seriesId",
    async ({ params }) => {
      await delay(200);
      const collection = mockCollections.find((c) => c.id === params.id);
      if (!collection) {
        return HttpResponse.json(
          { error: "Collection not found" },
          { status: 404 },
        );
      }
      collection.seriesIds = collection.seriesIds.filter(
        (id) => id !== params.seriesId,
      );
      return new HttpResponse(null, { status: 204 });
    },
  ),

  // Set full manual order
  http.put("/api/v1/collections/:id/series", async ({ params, request }) => {
    await delay(200);
    const collection = mockCollections.find((c) => c.id === params.id);
    if (!collection) {
      return HttpResponse.json(
        { error: "Collection not found" },
        { status: 404 },
      );
    }
    const body = (await request.json()) as { seriesIds: string[] };
    collection.seriesIds = body.seriesIds;
    return new HttpResponse(null, { status: 204 });
  }),

  // Collections containing a given series
  http.get("/api/v1/series/:seriesId/collections", async ({ params }) => {
    await delay(100);
    const items = mockCollections
      .filter((c) => c.seriesIds.includes(params.seriesId as string))
      .map(toDto);
    return HttpResponse.json({ items, total: items.length });
  }),
];
