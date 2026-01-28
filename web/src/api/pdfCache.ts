import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type PdfCacheStatsDto = components["schemas"]["PdfCacheStatsDto"];
export type PdfCacheCleanupResultDto =
	components["schemas"]["PdfCacheCleanupResultDto"];
export type TriggerPdfCacheCleanupResponse =
	components["schemas"]["TriggerPdfCacheCleanupResponse"];

export const pdfCacheApi = {
	/**
	 * Get statistics about the PDF page cache (admin only)
	 *
	 * Returns information about cached PDF pages including total size,
	 * file count, and age of oldest files.
	 */
	getStats: async (): Promise<PdfCacheStatsDto> => {
		const response = await api.get<PdfCacheStatsDto>("/admin/pdf-cache/stats");
		return response.data;
	},

	/**
	 * Trigger PDF cache cleanup task (admin only)
	 *
	 * Enqueues a background task to clean up cached PDF pages older than
	 * the configured max age (default 30 days). Returns a task ID which
	 * can be used to track progress.
	 */
	triggerCleanup: async (): Promise<TriggerPdfCacheCleanupResponse> => {
		const response = await api.post<TriggerPdfCacheCleanupResponse>(
			"/admin/pdf-cache/cleanup",
		);
		return response.data;
	},

	/**
	 * Clear all cached PDF pages immediately (admin only)
	 *
	 * Deletes all cached PDF pages immediately, returning the results.
	 * For large caches, prefer using the async triggerCleanup endpoint instead.
	 */
	clearCache: async (): Promise<PdfCacheCleanupResultDto> => {
		const response =
			await api.delete<PdfCacheCleanupResultDto>("/admin/pdf-cache");
		return response.data;
	},
};
