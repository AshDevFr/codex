/**
 * MSW handlers for series export API endpoints
 */

import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";

type SeriesExportDto = components["schemas"]["SeriesExportDto"];
type SeriesExportListResponse =
  components["schemas"]["SeriesExportListResponse"];
type ExportFieldDto = components["schemas"]["ExportFieldDto"];
type ExportFieldCatalogResponse =
  components["schemas"]["ExportFieldCatalogResponse"];

// Mock data store
let mockExports: SeriesExportDto[] = [
  {
    id: "550e8400-e29b-41d4-a716-446655440001",
    format: "json",
    status: "completed",
    libraryIds: ["lib-1"],
    fields: ["title", "summary", "genres", "user_rating"],
    fileSizeBytes: 24576,
    rowCount: 42,
    error: null,
    createdAt: new Date(Date.now() - 86400000).toISOString(), // 1 day ago
    startedAt: new Date(Date.now() - 86400000 + 1000).toISOString(),
    completedAt: new Date(Date.now() - 86400000 + 5000).toISOString(),
    expiresAt: new Date(Date.now() + 6 * 86400000).toISOString(), // 6 days from now
  },
  {
    id: "550e8400-e29b-41d4-a716-446655440002",
    format: "csv",
    status: "running",
    libraryIds: ["lib-1", "lib-2"],
    fields: [
      "title",
      "summary",
      "genres",
      "tags",
      "authors",
      "actual_book_count",
    ],
    fileSizeBytes: null,
    rowCount: null,
    error: null,
    createdAt: new Date(Date.now() - 60000).toISOString(), // 1 min ago
    startedAt: new Date(Date.now() - 55000).toISOString(),
    completedAt: null,
    expiresAt: new Date(Date.now() + 7 * 86400000).toISOString(),
  },
  {
    id: "550e8400-e29b-41d4-a716-446655440003",
    format: "json",
    status: "failed",
    libraryIds: ["lib-3"],
    fields: ["title", "genres"],
    fileSizeBytes: null,
    rowCount: null,
    error: "disk full",
    createdAt: new Date(Date.now() - 3600000).toISOString(), // 1 hour ago
    startedAt: new Date(Date.now() - 3599000).toISOString(),
    completedAt: new Date(Date.now() - 3590000).toISOString(),
    expiresAt: new Date(Date.now() + 7 * 86400000).toISOString(),
  },
];

const mockFieldCatalog: ExportFieldDto[] = [
  {
    key: "series_id",
    label: "Series ID",
    multiValue: false,
    userSpecific: false,
  },
  {
    key: "series_name",
    label: "Series Name",
    multiValue: false,
    userSpecific: false,
  },
  {
    key: "library_id",
    label: "Library ID",
    multiValue: false,
    userSpecific: false,
  },
  {
    key: "library_name",
    label: "Library Name",
    multiValue: false,
    userSpecific: false,
  },
  { key: "path", label: "Path", multiValue: false, userSpecific: false },
  {
    key: "created_at",
    label: "Created At",
    multiValue: false,
    userSpecific: false,
  },
  {
    key: "updated_at",
    label: "Updated At",
    multiValue: false,
    userSpecific: false,
  },
  { key: "title", label: "Title", multiValue: false, userSpecific: false },
  { key: "summary", label: "Summary", multiValue: false, userSpecific: false },
  {
    key: "publisher",
    label: "Publisher",
    multiValue: false,
    userSpecific: false,
  },
  { key: "status", label: "Status", multiValue: false, userSpecific: false },
  { key: "year", label: "Year", multiValue: false, userSpecific: false },
  {
    key: "language",
    label: "Language",
    multiValue: false,
    userSpecific: false,
  },
  { key: "authors", label: "Authors", multiValue: true, userSpecific: false },
  { key: "genres", label: "Genres", multiValue: true, userSpecific: false },
  { key: "tags", label: "Tags", multiValue: true, userSpecific: false },
  {
    key: "alternate_titles",
    label: "Alternate Titles",
    multiValue: true,
    userSpecific: false,
  },
  {
    key: "expected_book_count",
    label: "Expected Book Count",
    multiValue: false,
    userSpecific: false,
  },
  {
    key: "actual_book_count",
    label: "Actual Book Count",
    multiValue: false,
    userSpecific: false,
  },
  {
    key: "unread_book_count",
    label: "Unread Book Count",
    multiValue: false,
    userSpecific: false,
  },
  {
    key: "user_rating",
    label: "User Rating",
    multiValue: false,
    userSpecific: true,
  },
  {
    key: "user_notes",
    label: "User Notes",
    multiValue: false,
    userSpecific: true,
  },
  {
    key: "community_avg_rating",
    label: "Community Avg Rating",
    multiValue: false,
    userSpecific: false,
  },
  {
    key: "external_ratings",
    label: "External Ratings",
    multiValue: true,
    userSpecific: false,
  },
];

export const seriesExportsHandlers = [
  // List exports
  http.get("/api/v1/user/exports/series", async () => {
    await delay(150);
    const response: SeriesExportListResponse = { exports: mockExports };
    return HttpResponse.json(response);
  }),

  // Get field catalog
  http.get("/api/v1/user/exports/series/fields", async () => {
    await delay(100);
    const response: ExportFieldCatalogResponse = { fields: mockFieldCatalog };
    return HttpResponse.json(response);
  }),

  // Get single export
  http.get("/api/v1/user/exports/series/:id", async ({ params }) => {
    await delay(100);
    const exp = mockExports.find((e) => e.id === params.id);
    if (!exp) {
      return HttpResponse.json({ error: "Export not found" }, { status: 404 });
    }
    return HttpResponse.json(exp);
  }),

  // Create export
  http.post("/api/v1/user/exports/series", async ({ request }) => {
    await delay(300);
    const body = (await request.json()) as {
      format: string;
      libraryIds: string[];
      fields: string[];
    };

    const newExport: SeriesExportDto = {
      id: crypto.randomUUID(),
      format: body.format,
      status: "pending",
      libraryIds: body.libraryIds,
      fields: body.fields,
      fileSizeBytes: null,
      rowCount: null,
      error: null,
      createdAt: new Date().toISOString(),
      startedAt: null,
      completedAt: null,
      expiresAt: new Date(Date.now() + 7 * 86400000).toISOString(),
    };

    mockExports = [newExport, ...mockExports];

    // Simulate task completing after 3 seconds
    setTimeout(() => {
      const idx = mockExports.findIndex((e) => e.id === newExport.id);
      if (idx >= 0) {
        mockExports[idx] = {
          ...mockExports[idx],
          status: "completed",
          startedAt: new Date().toISOString(),
          completedAt: new Date().toISOString(),
          rowCount: Math.floor(Math.random() * 100) + 5,
          fileSizeBytes: Math.floor(Math.random() * 100000) + 1000,
        };
      }
    }, 3000);

    return HttpResponse.json(newExport, { status: 202 });
  }),

  // Delete export
  http.delete("/api/v1/user/exports/series/:id", async ({ params }) => {
    await delay(200);
    const idx = mockExports.findIndex((e) => e.id === params.id);
    if (idx < 0) {
      return HttpResponse.json({ error: "Export not found" }, { status: 404 });
    }
    mockExports = mockExports.filter((e) => e.id !== params.id);
    return new HttpResponse(null, { status: 204 });
  }),

  // Download export (returns mock blob)
  http.get("/api/v1/user/exports/series/:id/download", async ({ params }) => {
    await delay(200);
    const exp = mockExports.find((e) => e.id === params.id);
    if (!exp || exp.status !== "completed") {
      return HttpResponse.json({ error: "Export not ready" }, { status: 409 });
    }

    const mockContent =
      exp.format === "json"
        ? JSON.stringify(
            [
              {
                series_id: "1",
                series_name: "Mock Series A",
                library_id: "lib-1",
              },
              {
                series_id: "2",
                series_name: "Mock Series B",
                library_id: "lib-1",
              },
            ],
            null,
            2,
          )
        : "series_id,series_name,library_id\n1,Mock Series A,lib-1\n2,Mock Series B,lib-1\n";

    const contentType = exp.format === "csv" ? "text/csv" : "application/json";

    return new HttpResponse(mockContent, {
      headers: {
        "Content-Type": contentType,
        "Content-Disposition": `attachment; filename="codex-export.${exp.format}"`,
      },
    });
  }),
];
