/**
 * MSW handlers for PDF cache API endpoints
 */

import { delay, HttpResponse, http } from "msw";

// Mock data for PDF cache stats
let mockPdfCacheStats = {
	total_files: 1500,
	total_size_bytes: 157_286_400, // ~150 MB
	total_size_human: "150.0 MB",
	book_count: 45,
	oldest_file_age_days: 15,
	cache_dir: "/data/cache",
	cache_enabled: true,
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

		return HttpResponse.json({
			task_id: crypto.randomUUID(),
			message: "PDF cache cleanup task queued successfully",
			max_age_days: 30,
		});
	}),

	// Clear entire cache immediately (sync)
	http.delete("/api/v1/admin/pdf-cache", async () => {
		await delay(500);

		const result = {
			files_deleted: mockPdfCacheStats.total_files,
			bytes_reclaimed: mockPdfCacheStats.total_size_bytes,
			bytes_reclaimed_human: mockPdfCacheStats.total_size_human,
		};

		// Reset mock stats after clearing
		mockPdfCacheStats = {
			total_files: 0,
			total_size_bytes: 0,
			total_size_human: "0 B",
			book_count: 0,
			oldest_file_age_days: 0,
			cache_dir: "/data/cache",
			cache_enabled: true,
		};

		return HttpResponse.json(result);
	}),
];

// Helper to reset mock state (useful for tests)
export function resetPdfCacheMockState() {
	mockPdfCacheStats = {
		total_files: 1500,
		total_size_bytes: 157_286_400,
		total_size_human: "150.0 MB",
		book_count: 45,
		oldest_file_age_days: 15,
		cache_dir: "/data/cache",
		cache_enabled: true,
	};
}

// Helper to set custom mock state for testing
export function setPdfCacheMockState(stats: Partial<typeof mockPdfCacheStats>) {
	mockPdfCacheStats = { ...mockPdfCacheStats, ...stats };
}
