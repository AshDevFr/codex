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
    exportType: "series",
    status: "completed",
    libraryIds: ["lib-1"],
    fields: ["title", "summary", "genres", "user_rating"],
    bookFields: [],
    fileSizeBytes: 24576,
    rowCount: 42,
    error: null,
    createdAt: new Date(Date.now() - 86400000).toISOString(),
    startedAt: new Date(Date.now() - 86400000 + 1000).toISOString(),
    completedAt: new Date(Date.now() - 86400000 + 5000).toISOString(),
    expiresAt: new Date(Date.now() + 6 * 86400000).toISOString(),
  },
  {
    id: "550e8400-e29b-41d4-a716-446655440002",
    format: "csv",
    exportType: "series",
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
    bookFields: [],
    fileSizeBytes: null,
    rowCount: null,
    error: null,
    createdAt: new Date(Date.now() - 60000).toISOString(),
    startedAt: new Date(Date.now() - 55000).toISOString(),
    completedAt: null,
    expiresAt: new Date(Date.now() + 7 * 86400000).toISOString(),
  },
  {
    id: "550e8400-e29b-41d4-a716-446655440003",
    format: "md",
    exportType: "books",
    status: "failed",
    libraryIds: ["lib-3"],
    fields: [],
    bookFields: ["title", "progress", "series_name"],
    fileSizeBytes: null,
    rowCount: null,
    error: "disk full",
    createdAt: new Date(Date.now() - 3600000).toISOString(),
    startedAt: new Date(Date.now() - 3599000).toISOString(),
    completedAt: new Date(Date.now() - 3590000).toISOString(),
    expiresAt: new Date(Date.now() + 7 * 86400000).toISOString(),
  },
];

const mockSeriesFields: ExportFieldDto[] = [
  {
    key: "series_name",
    label: "Series Name",
    multiValue: false,
    userSpecific: false,
    isAnchor: true,
  },
  {
    key: "series_id",
    label: "Series ID",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "library_id",
    label: "Library ID",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "library_name",
    label: "Library Name",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "path",
    label: "Path",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "created_at",
    label: "Created At",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "updated_at",
    label: "Updated At",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "title",
    label: "Title",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "summary",
    label: "Summary",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "publisher",
    label: "Publisher",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "status",
    label: "Status",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "year",
    label: "Year",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "language",
    label: "Language",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "authors",
    label: "Authors",
    multiValue: true,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "genres",
    label: "Genres",
    multiValue: true,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "tags",
    label: "Tags",
    multiValue: true,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "alternate_titles",
    label: "Alternate Titles",
    multiValue: true,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "expected_book_count",
    label: "Expected Book Count",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "actual_book_count",
    label: "Actual Book Count",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "unread_book_count",
    label: "Unread Book Count",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "progress",
    label: "Progress",
    multiValue: false,
    userSpecific: true,
    isAnchor: false,
  },
  {
    key: "user_rating",
    label: "User Rating",
    multiValue: false,
    userSpecific: true,
    isAnchor: false,
  },
  {
    key: "user_notes",
    label: "User Notes",
    multiValue: false,
    userSpecific: true,
    isAnchor: false,
  },
  {
    key: "community_avg_rating",
    label: "Community Avg Rating",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "external_ratings",
    label: "External Ratings",
    multiValue: true,
    userSpecific: false,
    isAnchor: false,
  },
];

const mockBookFields: ExportFieldDto[] = [
  {
    key: "book_name",
    label: "Book Name",
    multiValue: false,
    userSpecific: false,
    isAnchor: true,
  },
  {
    key: "book_id",
    label: "Book ID",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "series_id",
    label: "Series ID",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "library_id",
    label: "Library ID",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "series_name",
    label: "Series Name",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "library_name",
    label: "Library Name",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "file_name",
    label: "File Name",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "file_path",
    label: "File Path",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "file_size",
    label: "File Size",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "book_format",
    label: "Format",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "page_count",
    label: "Page Count",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "number",
    label: "Number",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "created_at",
    label: "Created At",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "updated_at",
    label: "Updated At",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "title",
    label: "Title",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "summary",
    label: "Summary",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "publisher",
    label: "Publisher",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "year",
    label: "Year",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "language",
    label: "Language",
    multiValue: false,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "authors",
    label: "Authors",
    multiValue: true,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "genres",
    label: "Genres",
    multiValue: true,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "tags",
    label: "Tags",
    multiValue: true,
    userSpecific: false,
    isAnchor: false,
  },
  {
    key: "progress",
    label: "Progress",
    multiValue: false,
    userSpecific: true,
    isAnchor: false,
  },
  {
    key: "current_page",
    label: "Current Page",
    multiValue: false,
    userSpecific: true,
    isAnchor: false,
  },
  {
    key: "completed",
    label: "Completed",
    multiValue: false,
    userSpecific: true,
    isAnchor: false,
  },
  {
    key: "completed_at",
    label: "Completed At",
    multiValue: false,
    userSpecific: true,
    isAnchor: false,
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
    const response: ExportFieldCatalogResponse = {
      fields: mockSeriesFields,
      bookFields: mockBookFields,
      presets: {
        llmSelect: [
          "title",
          "summary",
          "status",
          "year",
          "authors",
          "genres",
          "actual_book_count",
          "unread_book_count",
          "community_avg_rating",
          "user_rating",
          "user_notes",
          "progress",
        ],
        llmSelectBooks: [
          "title",
          "summary",
          "year",
          "authors",
          "genres",
          "series_name",
          "number",
          "progress",
        ],
      },
    };
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
      exportType: string;
      libraryIds: string[];
      fields: string[];
      bookFields: string[];
    };

    const newExport: SeriesExportDto = {
      id: crypto.randomUUID(),
      format: body.format,
      exportType: body.exportType || "series",
      status: "pending",
      libraryIds: body.libraryIds,
      fields: body.fields,
      bookFields: body.bookFields || [],
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

    let mockContent: string;
    let contentType: string;

    if (exp.format === "md") {
      mockContent =
        "## Mock Series A\n\n- **Title:** Series A\n- **Year:** 2024\n";
      contentType = "text/markdown";
    } else if (exp.format === "csv") {
      mockContent = "series_name,title,genres\nMock Series A,Title A,action\n";
      contentType = "text/csv";
    } else {
      mockContent = JSON.stringify(
        [{ series_name: "Mock Series A", title: "Title A" }],
        null,
        2,
      );
      contentType = "application/json";
    }

    return new HttpResponse(mockContent, {
      headers: {
        "Content-Type": contentType,
        "Content-Disposition": `attachment; filename="codex-export.${exp.format}"`,
      },
    });
  }),
];
