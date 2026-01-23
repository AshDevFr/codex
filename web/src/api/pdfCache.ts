import { api } from "./client";

/**
 * Statistics about the PDF page cache
 */
export interface PdfCacheStatsDto {
	/** Total number of cached page files */
	total_files: number;
	/** Total size of cache in bytes */
	total_size_bytes: number;
	/** Human-readable total size (e.g., "150.0 MB") */
	total_size_human: string;
	/** Number of unique books with cached pages */
	book_count: number;
	/** Age of the oldest cached file in days (if any files exist) */
	oldest_file_age_days?: number;
	/** Path to the cache directory */
	cache_dir: string;
	/** Whether the PDF page cache is enabled */
	cache_enabled: boolean;
}

/**
 * Result of a PDF cache cleanup operation
 */
export interface PdfCacheCleanupResultDto {
	/** Number of cached page files deleted */
	files_deleted: number;
	/** Bytes freed by the cleanup */
	bytes_reclaimed: number;
	/** Human-readable size reclaimed (e.g., "25.0 MB") */
	bytes_reclaimed_human: string;
}

/**
 * Response when triggering a PDF cache cleanup task
 */
export interface TriggerPdfCacheCleanupResponse {
	/** ID of the queued cleanup task */
	task_id: string;
	/** Message describing the action taken */
	message: string;
	/** Max age setting being used for cleanup (in days) */
	max_age_days: number;
}

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
		const response = await api.delete<PdfCacheCleanupResultDto>(
			"/admin/pdf-cache",
		);
		return response.data;
	},
};
