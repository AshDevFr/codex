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
  /** Hand-picked members. Always empty for a rule-backed collection. */
  seriesIds: string[];
  /** Membership rule; `null` for a hand-picked collection. */
  condition: Record<string, unknown> | null;
  createdAt: string;
  updatedAt: string;
}

/**
 * Resolve a rule-backed collection's members.
 *
 * Only `libraryId` leaves and the two combinators are evaluated. That is the
 * limit of what the mock series store can answer: its entries carry a library
 * but no tags or genres, so a tag rule has nothing to match against. An
 * unevaluable leaf matches nothing rather than everything, so an unsupported
 * rule shows up as an empty collection instead of the whole library.
 */
function resolveRule(condition: Record<string, unknown>): string[] {
  const matches = (series: (typeof mockSeries)[number]): boolean => {
    if (Array.isArray(condition.allOf)) {
      return (condition.allOf as Record<string, unknown>[]).every((child) =>
        resolveRule(child).includes(series.id),
      );
    }
    if (Array.isArray(condition.anyOf)) {
      return (condition.anyOf as Record<string, unknown>[]).some((child) =>
        resolveRule(child).includes(series.id),
      );
    }

    const library = condition.libraryId as
      | { operator?: string; value?: string; values?: string[] }
      | undefined;
    switch (library?.operator) {
      case "is":
        return series.libraryId === library.value;
      case "isNot":
        return series.libraryId !== library.value;
      case "in":
        return (library.values ?? []).includes(series.libraryId);
      case "notIn":
        return !(library.values ?? []).includes(series.libraryId);
      default:
        return false;
    }
  };

  return mockSeries.filter(matches).map((s) => s.id);
}

/** The series a collection contains, from its rule or its junction. */
function memberIds(collection: MockCollection): string[] {
  return collection.condition
    ? resolveRule(collection.condition)
    : collection.seriesIds;
}

const favoriteSeriesIds = mockSeries
  .filter((s) => s.libraryName === "Manga")
  .slice(0, 10)
  .map((s) => s.id);

const batmanSeriesIds = mockSeries
  .filter((s) => s.title.startsWith("Batman"))
  .map((s) => s.id);

const mangaLibraryId = mockSeries.find(
  (s) => s.libraryName === "Manga",
)?.libraryId;

let mockCollections: MockCollection[] = [
  {
    id: seededUuid("collection-favorites"),
    name: "Favorites",
    summary: null,
    ordered: false,
    seriesIds: favoriteSeriesIds,
    condition: null,
    createdAt: "2024-01-10T10:00:00Z",
    updatedAt: "2024-06-01T10:00:00Z",
  },
  {
    id: seededUuid("collection-batman"),
    name: "Batman Reading Order",
    summary: "The essential Batman arcs, in reading order.",
    ordered: true,
    seriesIds: batmanSeriesIds,
    condition: null,
    createdAt: "2024-02-15T10:00:00Z",
    updatedAt: "2024-05-20T10:00:00Z",
  },
  {
    id: seededUuid("collection-auto-manga"),
    name: "All Manga (automatic)",
    summary: "Every series in the Manga library, kept current automatically.",
    ordered: false,
    seriesIds: [],
    // Scoped by library rather than by tag: the mock series store populates
    // libraryId but not tags or genres, so a tag rule would resolve to nothing
    // and the demo collection would look broken rather than automatic.
    condition: mangaLibraryId
      ? { libraryId: { operator: "is", value: mangaLibraryId } }
      : null,
    createdAt: "2024-03-01T10:00:00Z",
    updatedAt: "2024-03-01T10:00:00Z",
  },
];

const toDto = (collection: MockCollection): CollectionDto => ({
  id: collection.id,
  name: collection.name,
  summary: collection.summary,
  ordered: collection.ordered,
  // The generated DTO models the rule as an opaque object; the mock holds it as
  // a plain record so it can actually be evaluated above.
  condition: collection.condition as CollectionDto["condition"],
  automatic: collection.condition !== null,
  // Null for automatic collections, matching the real API: counting one means
  // resolving its whole rule.
  seriesCount: collection.condition ? null : collection.seriesIds.length,
  createdAt: collection.createdAt,
  updatedAt: collection.updatedAt,
});

/** 409 body for hand-editing a rule-backed collection, as the real API sends. */
const automaticConflict = (collection: MockCollection) =>
  HttpResponse.json(
    {
      error: `Collection '${collection.name}' is automatic: its members come from its rule, so they cannot be edited by hand.`,
    },
    { status: 409 },
  );

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
      url.searchParams.get("sort") ??
      (collection.ordered && !collection.condition ? "manual" : "title");
    const direction = url.searchParams.get("direction") ?? "asc";

    const members = memberIds(collection)
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
      // Forced off alongside a rule, like the real API.
      ordered: (body.ordered ?? false) && !body.condition,
      seriesIds: [],
      condition:
        (body.condition as Record<string, unknown> | undefined) ?? null,
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
    // Absent leaves the rule alone; explicit null clears it and converts the
    // collection to manual.
    if (body.condition !== undefined) {
      collection.condition =
        (body.condition as Record<string, unknown> | null) ?? null;
    }
    if (body.ordered != null) collection.ordered = body.ordered;
    if (collection.condition) collection.ordered = false;
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
    if (collection.condition) return automaticConflict(collection);
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
      if (collection.condition) return automaticConflict(collection);
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
    if (collection.condition) return automaticConflict(collection);
    const body = (await request.json()) as { seriesIds: string[] };
    collection.seriesIds = body.seriesIds;
    return new HttpResponse(null, { status: 204 });
  }),

  // Collections containing a given series
  http.get("/api/v1/series/:seriesId/collections", async ({ params }) => {
    await delay(100);
    // Manual membership only: a rule-backed collection is a view over the
    // library rather than a container the series belongs to.
    const items = mockCollections
      .filter(
        (c) => !c.condition && c.seriesIds.includes(params.seriesId as string),
      )
      .map(toDto);
    return HttpResponse.json({ items, total: items.length });
  }),
];
