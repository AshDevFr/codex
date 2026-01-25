import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type OrphanStatsDto = components["schemas"]["OrphanStatsDto"];
export type OrphanedFileDto = components["schemas"]["OrphanedFileDto"];
export type CleanupResultDto = components["schemas"]["CleanupResultDto"];
export type TriggerCleanupResponse =
	components["schemas"]["TriggerCleanupResponse"];

export interface OrphanStatsOptions {
	/** If true, includes the full list of orphaned files in the response */
	includeFiles?: boolean;
}

export const cleanupApi = {
	/**
	 * Get statistics about orphaned files (admin only)
	 *
	 * Scans the thumbnail and cover directories for files that don't have
	 * corresponding database entries. This is a read-only operation.
	 */
	getOrphanStats: async (
		options: OrphanStatsOptions = {},
	): Promise<OrphanStatsDto> => {
		const params = new URLSearchParams();
		if (options.includeFiles) {
			params.set("includeFiles", "true");
		}
		const queryString = params.toString();
		const url = queryString
			? `/admin/cleanup-orphans/stats?${queryString}`
			: "/admin/cleanup-orphans/stats";
		const response = await api.get<OrphanStatsDto>(url);
		return response.data;
	},

	/**
	 * Trigger orphan cleanup task (admin only)
	 *
	 * Enqueues a background task to scan and delete orphaned files.
	 * Returns a task ID which can be used to track progress.
	 */
	triggerCleanup: async (): Promise<TriggerCleanupResponse> => {
		const response = await api.post<TriggerCleanupResponse>(
			"/admin/cleanup-orphans",
		);
		return response.data;
	},

	/**
	 * Delete orphaned files immediately (admin only)
	 *
	 * Scans for and deletes orphaned files immediately, returning
	 * the results. For large numbers of files, prefer using the
	 * async triggerCleanup endpoint instead.
	 */
	deleteOrphans: async (): Promise<CleanupResultDto> => {
		const response = await api.delete<CleanupResultDto>(
			"/admin/cleanup-orphans",
		);
		return response.data;
	},
};
