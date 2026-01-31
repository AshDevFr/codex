import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type MetricsDto = components["schemas"]["MetricsDto"];
export type LibraryMetricsDto = components["schemas"]["LibraryMetricsDto"];
export type TaskMetricsResponse = components["schemas"]["TaskMetricsResponse"];
export type TaskMetricsSummaryDto =
	components["schemas"]["TaskMetricsSummaryDto"];
export type TaskTypeMetricsDto = components["schemas"]["TaskTypeMetricsDto"];
export type QueueHealthMetricsDto =
	components["schemas"]["QueueHealthMetricsDto"];
export type TaskMetricsHistoryResponse =
	components["schemas"]["TaskMetricsHistoryResponse"];
export type TaskMetricsDataPointDto =
	components["schemas"]["TaskMetricsDataPointDto"];
export type MetricsCleanupResponse =
	components["schemas"]["MetricsCleanupResponse"];

// Plugin metrics types
export type PluginMetricsResponse =
	components["schemas"]["PluginMetricsResponse"];
export type PluginMetricsSummaryDto =
	components["schemas"]["PluginMetricsSummaryDto"];
export type PluginMetricsDto = components["schemas"]["PluginMetricsDto"];
export type PluginMethodMetricsDto =
	components["schemas"]["PluginMethodMetricsDto"];

export const metricsApi = {
	/**
	 * Get inventory metrics (libraries, books, series counts)
	 */
	getInventory: async (): Promise<MetricsDto> => {
		const response = await api.get<MetricsDto>("/metrics/inventory");
		return response.data;
	},

	/**
	 * Get task metrics (performance statistics)
	 */
	getTaskMetrics: async (): Promise<TaskMetricsResponse> => {
		const response = await api.get<TaskMetricsResponse>("/metrics/tasks");
		return response.data;
	},

	/**
	 * Get task metrics history
	 */
	getTaskHistory: async (params?: {
		days?: number;
		taskType?: string;
		granularity?: "hour" | "day";
	}): Promise<TaskMetricsHistoryResponse> => {
		const response = await api.get<TaskMetricsHistoryResponse>(
			"/metrics/tasks/history",
			{
				params: {
					days: params?.days,
					taskType: params?.taskType,
					granularity: params?.granularity,
				},
			},
		);
		return response.data;
	},

	/**
	 * Cleanup old task metrics
	 */
	cleanupTaskMetrics: async (): Promise<MetricsCleanupResponse> => {
		const response = await api.post<MetricsCleanupResponse>(
			"/metrics/tasks/cleanup",
		);
		return response.data;
	},

	/**
	 * Get plugin metrics (performance statistics for all plugins)
	 */
	getPluginMetrics: async (): Promise<PluginMetricsResponse> => {
		const response = await api.get<PluginMetricsResponse>("/metrics/plugins");
		return response.data;
	},
};
