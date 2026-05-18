/**
 * MSW handlers for PDF cache API endpoints
 */

import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";

type PdfCacheStatsDto = components["schemas"]["PdfCacheStatsDto"];
type PdfCacheCleanupResultDto =
  components["schemas"]["PdfCacheCleanupResultDto"];
type TriggerPdfCacheCleanupResponse =
  components["schemas"]["TriggerPdfCacheCleanupResponse"];

const defaultMockPdfCacheStats: PdfCacheStatsDto = {
  handles: {
    capacity: 256,
    currentSize: 12,
    enabled: true,
    entries: [
      {
        ageSeconds: 312,
        bookId: "550e8400-e29b-41d4-a716-446655440000",
        filePath: "/library/books/manual.pdf",
        idleSeconds: 14,
        renderCount: 27,
      },
    ],
    evictions: 5,
    hits: 4321,
    idleEvictions: 3,
    idleTtlSeconds: 900,
    misses: 87,
    opens: 87,
  },
  pages: {
    bookCount: 45,
    cacheDir: "/data/cache",
    cacheEnabled: true,
    oldestFileAgeDays: 15,
    totalFiles: 1500,
    totalSizeBytes: 157_286_400,
    totalSizeHuman: "150.0 MB",
  },
};

let mockPdfCacheStats: PdfCacheStatsDto = structuredClone(
  defaultMockPdfCacheStats,
);

export const pdfCacheHandlers = [
  // Get PDF cache stats
  http.get("/api/v1/admin/pdf-cache/stats", async () => {
    await delay(150);
    return HttpResponse.json(mockPdfCacheStats);
  }),

  // Trigger async cleanup (queue background task)
  http.post("/api/v1/admin/pdf-cache/cleanup", async () => {
    await delay(200);

    const response: TriggerPdfCacheCleanupResponse = {
      taskId: crypto.randomUUID(),
      message: "PDF cache cleanup task queued successfully",
      maxAgeDays: 30,
    };
    return HttpResponse.json(response);
  }),

  // Clear entire cache immediately (sync)
  http.delete("/api/v1/admin/pdf-cache", async () => {
    await delay(500);

    const result: PdfCacheCleanupResultDto = {
      filesDeleted: mockPdfCacheStats.pages.totalFiles,
      bytesReclaimed: mockPdfCacheStats.pages.totalSizeBytes,
      bytesReclaimedHuman: mockPdfCacheStats.pages.totalSizeHuman,
    };

    // Reset mock stats after clearing
    mockPdfCacheStats = {
      ...mockPdfCacheStats,
      pages: {
        ...mockPdfCacheStats.pages,
        bookCount: 0,
        oldestFileAgeDays: 0,
        totalFiles: 0,
        totalSizeBytes: 0,
        totalSizeHuman: "0 B",
      },
    };

    return HttpResponse.json(result);
  }),
];

// Helper to reset mock state (useful for tests)
export function resetPdfCacheMockState() {
  mockPdfCacheStats = structuredClone(defaultMockPdfCacheStats);
}

// Helper to set custom mock state for testing
export function setPdfCacheMockState(stats: Partial<PdfCacheStatsDto>) {
  mockPdfCacheStats = { ...mockPdfCacheStats, ...stats };
}
