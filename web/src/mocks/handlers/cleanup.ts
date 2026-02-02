/**
 * MSW handlers for cleanup API endpoints
 */

import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";

type OrphanStatsDto = components["schemas"]["OrphanStatsDto"];
type CleanupResultDto = components["schemas"]["CleanupResultDto"];

// Mock data for orphan stats
let mockOrphanStats: Omit<OrphanStatsDto, "files"> = {
  orphaned_thumbnails: 42,
  orphaned_covers: 5,
  total_size_bytes: 15_728_640, // ~15 MB
};

export const cleanupHandlers = [
  // Get orphan stats
  http.get("/api/v1/admin/cleanup-orphans/stats", async ({ request }) => {
    await delay(150);

    const url = new URL(request.url);
    const includeFiles = url.searchParams.get("includeFiles") === "true";

    const response: {
      orphaned_thumbnails: number;
      orphaned_covers: number;
      total_size_bytes: number;
      files?: Array<{
        path: string;
        entity_id: string;
        size_bytes: number;
        file_type: string;
      }>;
    } = {
      ...mockOrphanStats,
    };

    if (includeFiles) {
      response.files = [];
      // Generate mock orphaned files
      for (let i = 0; i < mockOrphanStats.orphaned_thumbnails; i++) {
        response.files.push({
          path: `/data/thumbnails/books/${i.toString(16).padStart(2, "0")}/${crypto.randomUUID()}.jpg`,
          entity_id: crypto.randomUUID(),
          size_bytes: Math.floor(Math.random() * 500000) + 10000,
          file_type: "thumbnail",
        });
      }
      for (let i = 0; i < mockOrphanStats.orphaned_covers; i++) {
        response.files.push({
          path: `/data/uploads/covers/${crypto.randomUUID()}.jpg`,
          entity_id: crypto.randomUUID(),
          size_bytes: Math.floor(Math.random() * 1000000) + 50000,
          file_type: "cover",
        });
      }
    }

    return HttpResponse.json(response);
  }),

  // Trigger async cleanup (queue background task)
  http.post("/api/v1/admin/cleanup-orphans", async () => {
    await delay(200);

    return HttpResponse.json({
      task_id: crypto.randomUUID(),
      message: "Cleanup task queued successfully",
    });
  }),

  // Delete orphans immediately (sync)
  http.delete("/api/v1/admin/cleanup-orphans", async () => {
    await delay(500);

    const result: CleanupResultDto = {
      thumbnails_deleted: mockOrphanStats.orphaned_thumbnails,
      covers_deleted: mockOrphanStats.orphaned_covers,
      bytes_freed: mockOrphanStats.total_size_bytes,
      failures: 0,
      errors: [],
    };

    // Reset mock stats after cleanup
    mockOrphanStats = {
      orphaned_thumbnails: 0,
      orphaned_covers: 0,
      total_size_bytes: 0,
    };

    return HttpResponse.json(result);
  }),
];

// Helper to reset mock state (useful for tests)
export function resetCleanupMockState() {
  mockOrphanStats = {
    orphaned_thumbnails: 42,
    orphaned_covers: 5,
    total_size_bytes: 15_728_640,
  };
}
