/**
 * MSW handlers for duplicates API endpoints
 */

import { delay, HttpResponse, http } from "msw";
import type { DuplicateGroup, SeriesDuplicateGroup } from "../data/factories";
import { mockBooks, mockSeries } from "../data/store";

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
        fileHash: `hash-${i + 1}-${"a".repeat(60)}`.slice(0, 64),
        duplicateCount: bookIds.length,
        bookIds: bookIds,
        createdAt: new Date(Date.now() - i * 24 * 60 * 60 * 1000).toISOString(),
        updatedAt: new Date(Date.now() - i * 12 * 60 * 60 * 1000).toISOString(),
      });
    }
  }
  return groups;
};

// Generate mock series duplicate groups. We create one `external_id` group
// (cross-library) and one `title` group (scoped to a single library) so the
// frontend can exercise both code paths.
const createMockSeriesDuplicates = (): SeriesDuplicateGroup[] => {
  const groups: SeriesDuplicateGroup[] = [];
  if (mockSeries.length >= 2) {
    groups.push({
      id: "series-dup-external-1",
      matchType: "external_id",
      matchKey: "plugin:mangabaka:12345",
      libraryId: null,
      seriesIds: [mockSeries[0].id, mockSeries[1].id],
      duplicateCount: 2,
      createdAt: new Date(Date.now() - 48 * 60 * 60 * 1000).toISOString(),
      updatedAt: new Date(Date.now() - 6 * 60 * 60 * 1000).toISOString(),
    });
  }
  if (mockSeries.length >= 4) {
    // Pick two series that share the same library so the `title` group has a
    // libraryId set (matching backend semantics).
    const sameLibrarySeries = mockSeries.filter(
      (s) => s.libraryId === mockSeries[2].libraryId,
    );
    if (sameLibrarySeries.length >= 2) {
      groups.push({
        id: "series-dup-title-1",
        matchType: "title",
        matchKey: "naruto",
        libraryId: sameLibrarySeries[0].libraryId,
        seriesIds: [sameLibrarySeries[0].id, sameLibrarySeries[1].id],
        duplicateCount: 2,
        createdAt: new Date(Date.now() - 36 * 60 * 60 * 1000).toISOString(),
        updatedAt: new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString(),
      });
    }
  }
  return groups;
};

const mockDuplicates = createMockDuplicates();
const mockSeriesDuplicates = createMockSeriesDuplicates();

const computeSeriesDuplicateTotals = (groups: SeriesDuplicateGroup[]) => {
  let externalIdGroups = 0;
  let titleGroups = 0;
  let totalDuplicateSeries = 0;
  for (const g of groups) {
    totalDuplicateSeries += g.duplicateCount;
    if (g.matchType === "external_id") externalIdGroups += 1;
    else if (g.matchType === "title") titleGroups += 1;
  }
  return {
    totalGroups: groups.length,
    totalDuplicateSeries,
    externalIdGroups,
    titleGroups,
  };
};

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
      (sum, group) => sum + group.duplicateCount,
      0,
    );

    return HttpResponse.json({
      duplicates: filteredDuplicates,
      totalGroups: filteredDuplicates.length,
      totalDuplicateBooks: totalDuplicateBooks,
    });
  }),

  // List series duplicates (optionally filtered by matchType)
  http.get("/api/v1/duplicates/series", async ({ request }) => {
    await delay(100);
    const url = new URL(request.url);
    const matchType = url.searchParams.get("matchType");
    const filtered = matchType
      ? mockSeriesDuplicates.filter((g) => g.matchType === matchType)
      : mockSeriesDuplicates;
    const totals = computeSeriesDuplicateTotals(filtered);

    return HttpResponse.json({
      duplicates: filtered,
      ...totals,
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

    const deletedCount = mockDuplicates[groupIndex].duplicateCount - 1;
    mockDuplicates.splice(groupIndex, 1);

    return HttpResponse.json({
      deletedCount: deletedCount,
      keptBookId: keepBookId,
    });
  }),

  // Delete a series duplicate group
  http.delete("/api/v1/duplicates/series/:groupId", async ({ params }) => {
    await delay(120);
    const { groupId } = params;
    const idx = mockSeriesDuplicates.findIndex((g) => g.id === groupId);
    if (idx === -1) {
      return new HttpResponse(null, { status: 404 });
    }
    mockSeriesDuplicates.splice(idx, 1);
    return new HttpResponse(null, { status: 204 });
  }),

  // Rescan for duplicates
  http.post("/api/v1/duplicates/scan", async () => {
    await delay(500);
    return HttpResponse.json({
      taskId: crypto.randomUUID(),
      message: "Duplicate scan task queued",
    });
  }),
];
