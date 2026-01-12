/**
 * MSW handlers for duplicates API endpoints
 */

import { http, HttpResponse, delay } from "msw";
import { createDuplicateGroup, createList } from "../data/factories";

// Generate mock duplicate groups
const mockDuplicates = createList(() => createDuplicateGroup(), 10);

export const duplicatesHandlers = [
	// List all duplicates
	http.get("/api/v1/duplicates", async ({ request }) => {
		await delay(100);
		const url = new URL(request.url);
		const libraryId = url.searchParams.get("library_id");

		let filteredDuplicates = mockDuplicates;

		// In a real implementation, we'd filter by library_id
		// For now, just return all duplicates

		const totalDuplicateBooks = filteredDuplicates.reduce(
			(sum, group) => sum + group.duplicate_count,
			0
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
		const keepBookId = url.searchParams.get("keep_book_id");

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
