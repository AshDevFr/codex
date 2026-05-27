/**
 * Mock handlers for filter presets.
 *
 * Backs the "Presets" controls on the library list filter drawer/sheet and the
 * advanced search page. Without these, `filterPresetsApi.list` returns nothing
 * in mock mode and the "Apply preset…" dropdown never renders, so the preset
 * UI is impossible to exercise (or screenshot) against the mock backend.
 *
 * State is kept in an in-memory array so create / update / delete behave like a
 * real session for the lifetime of the page.
 */

import { HttpResponse, http } from "msw";

interface MockPreset {
  id: string;
  name: string;
  scope: string;
  target: string;
  condition: Record<string, unknown>;
  query: string | null;
  sort: string | null;
  libraryId: string | null;
  createdAt: string;
  updatedAt: string;
}

const now = () => new Date().toISOString();

let presets: MockPreset[] = [
  {
    id: "preset-books-unread",
    name: "Unread comics",
    scope: "list",
    target: "books",
    condition: {
      allOf: [{ readStatus: { operator: "equals", value: "unread" } }],
    },
    query: null,
    sort: null,
    libraryId: null,
    createdAt: now(),
    updatedAt: now(),
  },
  {
    id: "preset-books-read",
    name: "Finished reading",
    scope: "list",
    target: "books",
    condition: {
      allOf: [{ readStatus: { operator: "equals", value: "read" } }],
    },
    query: null,
    sort: null,
    libraryId: null,
    createdAt: now(),
    updatedAt: now(),
  },
  {
    id: "preset-series-ongoing",
    name: "Ongoing series",
    scope: "list",
    target: "series",
    condition: {
      allOf: [{ status: { operator: "equals", value: "ongoing" } }],
    },
    query: null,
    sort: null,
    libraryId: null,
    createdAt: now(),
    updatedAt: now(),
  },
  {
    id: "preset-search-isekai",
    name: "Isekai manga",
    scope: "search",
    target: "series",
    condition: { allOf: [{ tag: { operator: "equals", value: "isekai" } }] },
    query: "isekai",
    sort: null,
    libraryId: null,
    createdAt: now(),
    updatedAt: now(),
  },
];

export const filterPresetsHandlers = [
  http.get("/api/v1/filter-presets", ({ request }) => {
    const url = new URL(request.url);
    const scope = url.searchParams.get("scope");
    const target = url.searchParams.get("target");
    const filtered = presets.filter(
      (p) => (!scope || p.scope === scope) && (!target || p.target === target),
    );
    return HttpResponse.json({ presets: filtered });
  }),

  http.get("/api/v1/filter-presets/:id", ({ params }) => {
    const preset = presets.find((p) => p.id === params.id);
    if (!preset) {
      return new HttpResponse(null, { status: 404 });
    }
    return HttpResponse.json(preset);
  }),

  http.post("/api/v1/filter-presets", async ({ request }) => {
    const body = (await request.json()) as Partial<MockPreset>;
    const preset: MockPreset = {
      id: `preset-${crypto.randomUUID()}`,
      name: body.name ?? "Untitled preset",
      scope: body.scope ?? "list",
      target: body.target ?? "books",
      condition: body.condition ?? {},
      query: body.query ?? null,
      sort: body.sort ?? null,
      libraryId: body.libraryId ?? null,
      createdAt: now(),
      updatedAt: now(),
    };
    presets.push(preset);
    return HttpResponse.json(preset, { status: 201 });
  }),

  http.put("/api/v1/filter-presets/:id", async ({ params, request }) => {
    const body = (await request.json()) as Partial<MockPreset>;
    const idx = presets.findIndex((p) => p.id === params.id);
    if (idx === -1) {
      return new HttpResponse(null, { status: 404 });
    }
    presets[idx] = {
      ...presets[idx],
      ...body,
      id: presets[idx].id,
      updatedAt: now(),
    };
    return HttpResponse.json(presets[idx]);
  }),

  http.delete("/api/v1/filter-presets/:id", ({ params }) => {
    presets = presets.filter((p) => p.id !== params.id);
    return new HttpResponse(null, { status: 204 });
  }),
];
