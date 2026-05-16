/**
 * Release inbox + sources mock handlers
 *
 * Covers the three reads that ReleasesInbox.tsx makes on first paint
 * (`release-sources`, `releases/facets`, `releases`) plus the small
 * set of writes invoked from the inbox UI. The intent is "the page
 * renders without crashing in mock mode" — not a full simulation of
 * the polling/ledger lifecycle.
 */

import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";
import { createPaginatedResponse } from "../data/factories";

type ReleaseLedgerEntryDto = components["schemas"]["ReleaseLedgerEntryDto"];
type ReleaseSourceDto = components["schemas"]["ReleaseSourceDto"];
type ReleaseFacetsResponse = components["schemas"]["ReleaseFacetsResponse"];

const SERIES_A = "10000000-0000-0000-0000-000000000001";
const SERIES_B = "10000000-0000-0000-0000-000000000002";
const LIBRARY_MANGA = "20000000-0000-0000-0000-000000000001";
const LIBRARY_COMICS = "20000000-0000-0000-0000-000000000002";
const SOURCE_MANGAUPDATES = "30000000-0000-0000-0000-000000000001";
const SOURCE_NYAA = "30000000-0000-0000-0000-000000000002";

const mockSources: ReleaseSourceDto[] = [
  {
    id: SOURCE_MANGAUPDATES,
    displayName: "MangaUpdates Releases",
    sourceKey: "default",
    pluginId: "release-mangaupdates",
    kind: "metadata-feed",
    enabled: true,
    cronSchedule: null,
    effectiveCronSchedule: "0 0 * * *",
    createdAt: "2026-04-01T00:00:00Z",
    updatedAt: "2026-05-15T12:00:00Z",
    lastPolledAt: "2026-05-15T12:00:00Z",
    lastSummary: "Polled 42 series · 3 new releases",
  },
  {
    id: SOURCE_NYAA,
    displayName: "Nyaa (tsuna69)",
    sourceKey: "nyaa:user:tsuna69",
    pluginId: "release-nyaa",
    kind: "rss-uploader",
    enabled: true,
    cronSchedule: "*/30 * * * *",
    effectiveCronSchedule: "*/30 * * * *",
    createdAt: "2026-04-01T00:00:00Z",
    updatedAt: "2026-05-15T11:30:00Z",
    lastPolledAt: "2026-05-15T11:30:00Z",
    lastSummary: "1 new release",
  },
];

const mockEntries: ReleaseLedgerEntryDto[] = [
  {
    id: "40000000-0000-0000-0000-000000000001",
    seriesId: SERIES_A,
    seriesTitle: "Solo Leveling",
    sourceId: SOURCE_MANGAUPDATES,
    externalReleaseId: "mu:solo-leveling:200",
    payloadUrl: "https://www.mangaupdates.com/releases.html",
    confidence: 0.95,
    state: "announced",
    observedAt: "2026-05-15T11:55:00Z",
    createdAt: "2026-05-15T11:55:00Z",
    chapters: [{ start: 200, end: 200 }],
    volumes: null,
    language: "en",
    groupOrUploader: "Disastrous Scans",
  },
  {
    id: "40000000-0000-0000-0000-000000000002",
    seriesId: SERIES_B,
    seriesTitle: "Chainsaw Man",
    sourceId: SOURCE_NYAA,
    externalReleaseId: "nyaa:1234567",
    payloadUrl: "https://nyaa.si/view/1234567",
    mediaUrl: "https://nyaa.si/download/1234567.torrent",
    mediaUrlKind: "torrent",
    confidence: 0.88,
    state: "announced",
    observedAt: "2026-05-15T10:10:00Z",
    createdAt: "2026-05-15T10:10:00Z",
    chapters: [{ start: 162, end: 162 }],
    volumes: null,
    language: "en",
    groupOrUploader: "GroupZ",
  },
];

const mockFacets: ReleaseFacetsResponse = {
  languages: [{ language: "en", count: 2 }],
  libraries: [
    { libraryId: LIBRARY_MANGA, libraryName: "Manga", count: 2 },
    { libraryId: LIBRARY_COMICS, libraryName: "Comics", count: 0 },
  ],
  series: [
    {
      seriesId: SERIES_A,
      seriesTitle: "Solo Leveling",
      libraryId: LIBRARY_MANGA,
      libraryName: "Manga",
      count: 1,
    },
    {
      seriesId: SERIES_B,
      seriesTitle: "Chainsaw Man",
      libraryId: LIBRARY_MANGA,
      libraryName: "Manga",
      count: 1,
    },
  ],
};

function filterEntries(params: URLSearchParams): ReleaseLedgerEntryDto[] {
  const state = params.get("state") ?? "announced";
  const language = params.get("language");
  const seriesId = params.get("seriesId");

  return mockEntries.filter((e) => {
    if (state !== "all" && e.state !== state) return false;
    if (language && e.language !== language) return false;
    if (seriesId && e.seriesId !== seriesId) return false;
    return true;
  });
}

export const releasesHandlers = [
  // GET /api/v1/release-sources — list configured sources
  http.get("/api/v1/release-sources", async () => {
    await delay(100);
    return HttpResponse.json({ sources: mockSources });
  }),

  // GET /api/v1/release-sources/applicability — used by series detail to
  // gate the Tracking panel. Mock mode advertises tracking as available.
  http.get("/api/v1/release-sources/applicability", async () => {
    await delay(50);
    return HttpResponse.json({ applicable: true });
  }),

  // GET /api/v1/releases — inbox listing (paginated)
  http.get("/api/v1/releases", async ({ request }) => {
    await delay(150);
    const url = new URL(request.url);
    const page = Math.max(
      1,
      Number.parseInt(url.searchParams.get("page") || "1", 10),
    );
    const pageSize = Number.parseInt(
      url.searchParams.get("pageSize") || "50",
      10,
    );

    const filtered = filterEntries(url.searchParams);
    const start = (page - 1) * pageSize;
    const items = filtered.slice(start, start + pageSize);

    return HttpResponse.json(
      createPaginatedResponse(items, {
        page,
        pageSize,
        total: filtered.length,
        basePath: "/api/v1/releases",
      }),
    );
  }),

  // GET /api/v1/releases/facets — distinct values for the inbox dropdowns
  http.get("/api/v1/releases/facets", async () => {
    await delay(100);
    return HttpResponse.json(mockFacets);
  }),

  // GET /api/v1/series/:id/releases — per-series release listing
  http.get("/api/v1/series/:seriesId/releases", async ({ params, request }) => {
    await delay(150);
    const url = new URL(request.url);
    const page = Math.max(
      1,
      Number.parseInt(url.searchParams.get("page") || "1", 10),
    );
    const pageSize = Number.parseInt(
      url.searchParams.get("pageSize") || "50",
      10,
    );
    const filtered = mockEntries.filter((e) => e.seriesId === params.seriesId);
    const start = (page - 1) * pageSize;
    const items = filtered.slice(start, start + pageSize);

    return HttpResponse.json(
      createPaginatedResponse(items, {
        page,
        pageSize,
        total: filtered.length,
        basePath: `/api/v1/series/${params.seriesId}/releases`,
      }),
    );
  }),

  // Per-row writes — return the row with the new state so React Query's
  // optimistic update path stays happy.
  http.post("/api/v1/releases/:id/dismiss", async ({ params }) => {
    await delay(80);
    const entry = mockEntries.find((e) => e.id === params.id);
    if (!entry) {
      return HttpResponse.json({ error: "Not found" }, { status: 404 });
    }
    return HttpResponse.json({ ...entry, state: "dismissed" });
  }),

  http.post("/api/v1/releases/:id/mark-acquired", async ({ params }) => {
    await delay(80);
    const entry = mockEntries.find((e) => e.id === params.id);
    if (!entry) {
      return HttpResponse.json({ error: "Not found" }, { status: 404 });
    }
    return HttpResponse.json({ ...entry, state: "marked_acquired" });
  }),

  http.delete("/api/v1/releases/:id", async () => {
    await delay(80);
    return HttpResponse.json({ deleted: true });
  }),

  http.post("/api/v1/releases/bulk", async ({ request }) => {
    await delay(150);
    const body = (await request.json()) as {
      ids: string[];
      action: string;
    };
    return HttpResponse.json({
      affected: body.ids.length,
      action: body.action,
    });
  }),
];
