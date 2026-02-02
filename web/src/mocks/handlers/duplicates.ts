/**
 * MSW handlers for duplicates API endpoints
 */

import { delay, HttpResponse, http } from "msw";
import type { DuplicateGroup } from "../data/factories";
import { mockBooks } from "../data/store";

// Generate mock duplicate groups using actual book IDs
const createMockDuplicates = (): DuplicateGroup[] => {
  const groups: DuplicateGroup[] = [];
  // Create duplicate groups by pairing books from the store
  // We'll create 5 groups, each with 2-3 "duplicate" books
  for (let i = 0; i < 5; i++) {
    const startIndex = i * 3;
    if (startIndex + 1 < mockBooks.length) {
      const bookIds = [mockBooks[startIndex].id, mockBooks[startIndex + 1].id];
      // Add a third book to some groups
      if (i % 2 === 0 && startIndex + 2 < mockBooks.length) {
        bookIds.push(mockBooks[startIndex + 2].id);
      }
      groups.push({
        id: `dup-group-${i + 1}`,
        file_hash: `hash-${i + 1}-${"a".repeat(60)}`.slice(0, 64),
        duplicate_count: bookIds.length,
        book_ids: bookIds,
        created_at: new Date(
          Date.now() - i * 24 * 60 * 60 * 1000,
        ).toISOString(),
        updated_at: new Date(
          Date.now() - i * 12 * 60 * 60 * 1000,
        ).toISOString(),
      });
    }
  }
  return groups;
};

const mockDuplicates = createMockDuplicates();

export const duplicatesHandlers = [
  // List all duplicates
  http.get("/api/v1/duplicates", async ({ request }) => {
    await delay(100);
    const url = new URL(request.url);
    const libraryId = url.searchParams.get("libraryId");

    // In a real implementation, we'd filter by libraryId
    // For now, just return all duplicates (libraryId filtering not implemented yet)
    void libraryId;
    const filteredDuplicates = mockDuplicates;

    const totalDuplicateBooks = filteredDuplicates.reduce(
      (sum, group) => sum + group.duplicate_count,
      0,
    );

    return HttpResponse.json({
      duplicates: filteredDuplicates,
      total_groups: filteredDuplicates.length,
      total_duplicate_books: totalDuplicateBooks,
    });
  }),

  // Get single duplicate group
  http.get("/api/v1/duplicates/:groupId", async ({ params }) => {
    await delay(50);
    const { groupId } = params;
    const group = mockDuplicates.find((d) => d.id === groupId);

    if (!group) {
      return new HttpResponse(null, { status: 404 });
    }

    return HttpResponse.json(group);
  }),

  // Delete duplicate (keep one, delete others)
  http.delete("/api/v1/duplicates/:groupId", async ({ params, request }) => {
    await delay(150);
    const { groupId } = params;
    const url = new URL(request.url);
    const keepBookId = url.searchParams.get("keepBookId");

    const groupIndex = mockDuplicates.findIndex((d) => d.id === groupId);

    if (groupIndex === -1) {
      return new HttpResponse(null, { status: 404 });
    }

    const deletedCount = mockDuplicates[groupIndex].duplicate_count - 1;
    mockDuplicates.splice(groupIndex, 1);

    return HttpResponse.json({
      deleted_count: deletedCount,
      kept_book_id: keepBookId,
    });
  }),

  // Rescan for duplicates
  http.post("/api/v1/duplicates/scan", async () => {
    await delay(500);
    return HttpResponse.json({
      task_id: crypto.randomUUID(),
      message: "Duplicate scan task queued",
    });
  }),
];
