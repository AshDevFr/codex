/**
 * MSW handlers for cleanup API endpoints
 */

import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";

type OrphanStatsDto = components["schemas"]["OrphanStatsDto"];
type CleanupResultDto = components["schemas"]["CleanupResultDto"];

// Mock data for orphan stats
let mockOrphanStats: Omit<OrphanStatsDto, "files"> = {
  orphanedThumbnails: 42,
  orphanedCovers: 5,
  totalSizeBytes: 15_728_640, // ~15 MB
};

export const cleanupHandlers = [
  // Get orphan stats
  http.get("/api/v1/admin/cleanup-orphans/stats", async ({ request }) => {
    await delay(150);

    const url = new URL(request.url);
    const includeFiles = url.searchParams.get("includeFiles") === "true";

    const response: OrphanStatsDto = {
      ...mockOrphanStats,
    };

    if (includeFiles) {
      response.files = [];
      // Generate mock orphaned files
      for (let i = 0; i < mockOrphanStats.orphanedThumbnails; i++) {
        response.files.push({
          path: `/data/thumbnails/books/${i.toString(16).padStart(2, "0")}/${crypto.randomUUID()}.jpg`,
          entityId: crypto.randomUUID(),
          sizeBytes: Math.floor(Math.random() * 500000) + 10000,
          fileType: "thumbnail",
        });
      }
      for (let i = 0; i < mockOrphanStats.orphanedCovers; i++) {
        response.files.push({
          path: `/data/uploads/covers/${crypto.randomUUID()}.jpg`,
          entityId: crypto.randomUUID(),
          sizeBytes: Math.floor(Math.random() * 1000000) + 50000,
          fileType: "cover",
        });
      }
    }

    return HttpResponse.json(response);
  }),

  // Trigger async cleanup (queue background task)
  http.post("/api/v1/admin/cleanup-orphans", async () => {
    await delay(200);

    return HttpResponse.json({
      taskId: crypto.randomUUID(),
      message: "Cleanup task queued successfully",
    });
  }),

  // Delete orphans immediately (sync)
  http.delete("/api/v1/admin/cleanup-orphans", async () => {
    await delay(500);

    const result: CleanupResultDto = {
      thumbnailsDeleted: mockOrphanStats.orphanedThumbnails,
      coversDeleted: mockOrphanStats.orphanedCovers,
      bytesFreed: mockOrphanStats.totalSizeBytes,
      failures: 0,
      errors: [],
    };

    // Reset mock stats after cleanup
    mockOrphanStats = {
      orphanedThumbnails: 0,
      orphanedCovers: 0,
      totalSizeBytes: 0,
    };

    return HttpResponse.json(result);
  }),
];

// Helper to reset mock state (useful for tests)
export function resetCleanupMockState() {
  mockOrphanStats = {
    orphanedThumbnails: 42,
    orphanedCovers: 5,
    totalSizeBytes: 15_728_640,
  };
}
