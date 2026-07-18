/**
 * Read lists API mock handlers
 *
 * In-memory read lists built from the shared book store so the read list
 * pages work in mock mode (list, detail, sorting, membership edits, manual
 * reordering).
 */

import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";
import { seededUuid } from "../data/factories";
import { mockBooks } from "../data/store";

type ReadListDto = components["schemas"]["ReadListDto"];
type CreateReadListRequest = components["schemas"]["CreateReadListRequest"];
type UpdateReadListRequest = components["schemas"]["UpdateReadListRequest"];

interface MockReadList {
  id: string;
  name: string;
  summary: string | null;
  ordered: boolean;
  bookIds: string[];
  createdAt: string;
  updatedAt: string;
}

const onePieceBookIds = mockBooks
  .filter((b) => b.seriesName === "One Piece")
  .slice(0, 6)
  .map((b) => b.id);

const samplerBookIds = mockBooks
  .filter((b) => b.number === 1)
  .slice(0, 8)
  .map((b) => b.id);

let mockReadLists: MockReadList[] = [
  {
    id: seededUuid("readlist-one-piece-start"),
    name: "One Piece: East Blue",
    summary: "The East Blue saga, in reading order.",
    ordered: true,
    bookIds: onePieceBookIds,
    createdAt: "2024-03-01T10:00:00Z",
    updatedAt: "2024-05-10T10:00:00Z",
  },
  {
    id: seededUuid("readlist-first-issues"),
    name: "First Issues Sampler",
    summary: null,
    ordered: false,
    bookIds: samplerBookIds,
    createdAt: "2024-04-01T10:00:00Z",
    updatedAt: "2024-06-01T10:00:00Z",
  },
];

const toDto = (readList: MockReadList): ReadListDto => ({
  id: readList.id,
  name: readList.name,
  summary: readList.summary,
  ordered: readList.ordered,
  bookCount: readList.bookIds.length,
  createdAt: readList.createdAt,
  updatedAt: readList.updatedAt,
});

export const readListsHandlers = [
  // List read lists
  http.get("/api/v1/readlists", async () => {
    await delay(150);
    return HttpResponse.json({
      items: mockReadLists.map(toDto),
      total: mockReadLists.length,
    });
  }),

  // Get read list by ID
  http.get("/api/v1/readlists/:id", async ({ params }) => {
    await delay(100);
    const readList = mockReadLists.find((r) => r.id === params.id);
    if (!readList) {
      return HttpResponse.json(
        { error: "Read list not found" },
        { status: 404 },
      );
    }
    return HttpResponse.json(toDto(readList));
  }),

  // Member books, sorted like the real API (manual keeps stored order).
  // Mock books carry no release date, so `release` falls back to book number.
  http.get("/api/v1/readlists/:id/books", async ({ params, request }) => {
    await delay(150);
    const readList = mockReadLists.find((r) => r.id === params.id);
    if (!readList) {
      return HttpResponse.json(
        { error: "Read list not found" },
        { status: 404 },
      );
    }

    const url = new URL(request.url);
    const sort =
      url.searchParams.get("sort") ?? (readList.ordered ? "manual" : "release");
    const direction = url.searchParams.get("direction") ?? "asc";

    const members = readList.bookIds
      .map((id) => mockBooks.find((b) => b.id === id))
      .filter((b) => b !== undefined);

    if (sort !== "manual") {
      members.sort((a, b) => {
        switch (sort) {
          case "added":
            return a.createdAt.localeCompare(b.createdAt);
          case "title":
            return (a.titleSort ?? a.title).localeCompare(
              b.titleSort ?? b.title,
            );
          default:
            return (a.number ?? 0) - (b.number ?? 0);
        }
      });
      if (direction === "desc") members.reverse();
    }

    return HttpResponse.json(members);
  }),

  // Create read list
  http.post("/api/v1/readlists", async ({ request }) => {
    await delay(200);
    const body = (await request.json()) as CreateReadListRequest;
    const readList: MockReadList = {
      id: seededUuid(`readlist-${body.name}-${mockReadLists.length}`),
      name: body.name,
      summary: body.summary ?? null,
      ordered: body.ordered ?? false,
      bookIds: [],
      createdAt: "2024-06-15T10:00:00Z",
      updatedAt: "2024-06-15T10:00:00Z",
    };
    mockReadLists.push(readList);
    return HttpResponse.json(toDto(readList), { status: 201 });
  }),

  // Update read list
  http.patch("/api/v1/readlists/:id", async ({ params, request }) => {
    await delay(200);
    const readList = mockReadLists.find((r) => r.id === params.id);
    if (!readList) {
      return HttpResponse.json(
        { error: "Read list not found" },
        { status: 404 },
      );
    }
    const body = (await request.json()) as UpdateReadListRequest;
    if (body.name != null) readList.name = body.name;
    if (body.summary !== undefined) readList.summary = body.summary;
    if (body.ordered != null) readList.ordered = body.ordered;
    return HttpResponse.json(toDto(readList));
  }),

  // Delete read list
  http.delete("/api/v1/readlists/:id", async ({ params }) => {
    await delay(200);
    mockReadLists = mockReadLists.filter((r) => r.id !== params.id);
    return new HttpResponse(null, { status: 204 });
  }),

  // Add books to read list
  http.post("/api/v1/readlists/:id/books", async ({ params, request }) => {
    await delay(200);
    const readList = mockReadLists.find((r) => r.id === params.id);
    if (!readList) {
      return HttpResponse.json(
        { error: "Read list not found" },
        { status: 404 },
      );
    }
    const body = (await request.json()) as { bookIds: string[] };
    for (const bookId of body.bookIds) {
      if (!readList.bookIds.includes(bookId)) {
        readList.bookIds.push(bookId);
      }
    }
    return HttpResponse.json(toDto(readList));
  }),

  // Remove book from read list
  http.delete("/api/v1/readlists/:id/books/:bookId", async ({ params }) => {
    await delay(200);
    const readList = mockReadLists.find((r) => r.id === params.id);
    if (!readList) {
      return HttpResponse.json(
        { error: "Read list not found" },
        { status: 404 },
      );
    }
    readList.bookIds = readList.bookIds.filter((id) => id !== params.bookId);
    return new HttpResponse(null, { status: 204 });
  }),

  // Set full manual order
  http.put("/api/v1/readlists/:id/books", async ({ params, request }) => {
    await delay(200);
    const readList = mockReadLists.find((r) => r.id === params.id);
    if (!readList) {
      return HttpResponse.json(
        { error: "Read list not found" },
        { status: 404 },
      );
    }
    const body = (await request.json()) as { bookIds: string[] };
    readList.bookIds = body.bookIds;
    return new HttpResponse(null, { status: 204 });
  }),

  // Read lists containing a given book
  http.get("/api/v1/books/:bookId/readlists", async ({ params }) => {
    await delay(100);
    const items = mockReadLists
      .filter((r) => r.bookIds.includes(params.bookId as string))
      .map(toDto);
    return HttpResponse.json({ items, total: items.length });
  }),
];
