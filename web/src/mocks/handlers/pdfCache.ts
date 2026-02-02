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

// Mock data for PDF cache stats
let mockPdfCacheStats: PdfCacheStatsDto = {
  totalFiles: 1500,
  totalSizeBytes: 157_286_400, // ~150 MB
  totalSizeHuman: "150.0 MB",
  bookCount: 45,
  oldestFileAgeDays: 15,
  cacheDir: "/data/cache",
  cacheEnabled: true,
};

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
      filesDeleted: mockPdfCacheStats.totalFiles,
      bytesReclaimed: mockPdfCacheStats.totalSizeBytes,
      bytesReclaimedHuman: mockPdfCacheStats.totalSizeHuman,
    };

    // Reset mock stats after clearing
    mockPdfCacheStats = {
      totalFiles: 0,
      totalSizeBytes: 0,
      totalSizeHuman: "0 B",
      bookCount: 0,
      oldestFileAgeDays: 0,
      cacheDir: "/data/cache",
      cacheEnabled: true,
    };

    return HttpResponse.json(result);
  }),
];

// Helper to reset mock state (useful for tests)
export function resetPdfCacheMockState() {
  mockPdfCacheStats = {
    totalFiles: 1500,
    totalSizeBytes: 157_286_400,
    totalSizeHuman: "150.0 MB",
    bookCount: 45,
    oldestFileAgeDays: 15,
    cacheDir: "/data/cache",
    cacheEnabled: true,
  };
}

// Helper to set custom mock state for testing
export function setPdfCacheMockState(stats: Partial<typeof mockPdfCacheStats>) {
  mockPdfCacheStats = { ...mockPdfCacheStats, ...stats };
}
