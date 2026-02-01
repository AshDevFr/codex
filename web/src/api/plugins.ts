import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type PluginDto = components["schemas"]["PluginDto"];
export type PluginsListResponse = components["schemas"]["PluginsListResponse"];
export type CreatePluginRequest = components["schemas"]["CreatePluginRequest"];
export type UpdatePluginRequest = components["schemas"]["UpdatePluginRequest"];
export type PluginTestResult = components["schemas"]["PluginTestResult"];
export type PluginStatusResponse =
	components["schemas"]["PluginStatusResponse"];
export type PluginHealthDto = components["schemas"]["PluginHealthDto"];
export type PluginHealthResponse =
	components["schemas"]["PluginHealthResponse"];
export type PluginManifestDto = components["schemas"]["PluginManifestDto"];
export type PluginCapabilitiesDto =
	components["schemas"]["PluginCapabilitiesDto"];
export type CredentialFieldDto = components["schemas"]["CredentialFieldDto"];
export type EnvVarDto = components["schemas"]["EnvVarDto"];

// Plugin Actions types
export type PluginActionDto = components["schemas"]["PluginActionDto"];
export type PluginActionsResponse =
	components["schemas"]["PluginActionsResponse"];
export type ExecutePluginRequest =
	components["schemas"]["ExecutePluginRequest"];
export type ExecutePluginResponse =
	components["schemas"]["ExecutePluginResponse"];
export type MetadataContentType = components["schemas"]["MetadataContentType"];
export type MetadataAction = components["schemas"]["MetadataAction"];
export type PluginActionRequest = components["schemas"]["PluginActionRequest"];
export type PluginSearchResultDto =
	components["schemas"]["PluginSearchResultDto"];
export type SearchResultPreviewDto =
	components["schemas"]["SearchResultPreviewDto"];
export type PluginSearchResponse =
	components["schemas"]["PluginSearchResponse"];

// Metadata Preview/Apply types
export type MetadataPreviewRequest =
	components["schemas"]["MetadataPreviewRequest"];
export type MetadataPreviewResponse =
	components["schemas"]["MetadataPreviewResponse"];
export type MetadataFieldPreview =
	components["schemas"]["MetadataFieldPreview"];
export type FieldApplyStatus = components["schemas"]["FieldApplyStatus"];
export type PreviewSummary = components["schemas"]["PreviewSummary"];
export type MetadataApplyRequest =
	components["schemas"]["MetadataApplyRequest"];
export type MetadataApplyResponse =
	components["schemas"]["MetadataApplyResponse"];
export type SkippedField = components["schemas"]["SkippedField"];
// Auto-match types (defined manually until OpenAPI types are regenerated)
export interface MetadataAutoMatchRequest {
	pluginId: string;
	query?: string;
}

export interface MetadataAutoMatchResponse {
	success: boolean;
	matchedResult?: PluginSearchResultDto;
	appliedFields: string[];
	skippedFields: SkippedField[];
	message: string;
	externalUrl?: string;
}

// Task-based auto-match response
export interface EnqueueAutoMatchResponse {
	success: boolean;
	tasksEnqueued: number;
	taskIds: string[];
	message: string;
}

// Search title response (preprocessed title for metadata search)
export interface SearchTitleResponse {
	originalTitle: string;
	searchTitle: string;
	rulesApplied: number;
}

// Plugin Failure types
export type PluginFailureDto = components["schemas"]["PluginFailureDto"];
export type PluginFailuresResponse =
	components["schemas"]["PluginFailuresResponse"];

// Plugin types
export type PluginType = "system" | "user";

// Health status values
export type PluginHealthStatus =
	| "unknown"
	| "healthy"
	| "degraded"
	| "unhealthy"
	| "disabled";

// Credential delivery methods
export type CredentialDelivery = "env" | "init_message" | "both";

// Plugin scopes (must match backend PluginScope enum)
export type PluginScope =
	| "series:detail"
	| "series:bulk"
	| "library:detail"
	| "library:scan";

// Plugin permissions
export type PluginPermission =
	| "metadata:read"
	| "metadata:write:title"
	| "metadata:write:summary"
	| "metadata:write:genres"
	| "metadata:write:tags"
	| "metadata:write:covers"
	| "metadata:write:ratings"
	| "metadata:write:links"
	| "metadata:write:year"
	| "metadata:write:status"
	| "metadata:write:publisher"
	| "metadata:write:age_rating"
	| "metadata:write:language"
	| "metadata:write:reading_direction"
	| "metadata:write:total_book_count"
	| "metadata:write:*"
	| "library:read";

// Available options for forms
export const AVAILABLE_SCOPES: { value: PluginScope; label: string }[] = [
	{ value: "series:detail", label: "Series Detail" },
	{ value: "series:bulk", label: "Series Bulk Actions" },
	{ value: "library:detail", label: "Library Detail" },
	{ value: "library:scan", label: "Post-Library Scan" },
];

export const AVAILABLE_PERMISSIONS: {
	value: PluginPermission;
	label: string;
}[] = [
	{ value: "metadata:read", label: "Read Metadata" },
	{ value: "metadata:write:*", label: "Write All Metadata" },
	{ value: "metadata:write:title", label: "Write Title" },
	{ value: "metadata:write:summary", label: "Write Summary" },
	{ value: "metadata:write:genres", label: "Write Genres" },
	{ value: "metadata:write:tags", label: "Write Tags" },
	{ value: "metadata:write:covers", label: "Write Covers" },
	{ value: "metadata:write:ratings", label: "Write Ratings" },
	{ value: "metadata:write:links", label: "Write Links" },
	{ value: "metadata:write:year", label: "Write Year" },
	{ value: "metadata:write:status", label: "Write Status" },
	{ value: "metadata:write:publisher", label: "Write Publisher" },
	{ value: "metadata:write:age_rating", label: "Write Age Rating" },
	{ value: "metadata:write:language", label: "Write Language" },
	{
		value: "metadata:write:reading_direction",
		label: "Write Reading Direction",
	},
	{
		value: "metadata:write:total_book_count",
		label: "Write Total Book Count",
	},
	{ value: "library:read", label: "Read Library" },
];

export const CREDENTIAL_DELIVERY_OPTIONS: {
	value: CredentialDelivery;
	label: string;
}[] = [
	{ value: "env", label: "Environment Variables" },
	{ value: "init_message", label: "Initialize Message" },
	{ value: "both", label: "Both" },
];

export const pluginsApi = {
	/**
	 * Get all plugins (Admin only)
	 */
	getAll: async (): Promise<PluginsListResponse> => {
		const response = await api.get<PluginsListResponse>("/admin/plugins");
		return response.data;
	},

	/**
	 * Get a single plugin by ID (Admin only)
	 */
	getById: async (id: string): Promise<PluginDto> => {
		const response = await api.get<PluginDto>(`/admin/plugins/${id}`);
		return response.data;
	},

	/**
	 * Create a new plugin (Admin only)
	 */
	create: async (request: CreatePluginRequest): Promise<PluginDto> => {
		const response = await api.post<PluginDto>("/admin/plugins", request);
		return response.data;
	},

	/**
	 * Update a plugin (Admin only)
	 */
	update: async (
		id: string,
		request: UpdatePluginRequest,
	): Promise<PluginDto> => {
		const response = await api.patch<PluginDto>(
			`/admin/plugins/${id}`,
			request,
		);
		return response.data;
	},

	/**
	 * Delete a plugin (Admin only)
	 */
	delete: async (id: string): Promise<void> => {
		await api.delete(`/admin/plugins/${id}`);
	},

	/**
	 * Enable a plugin (Admin only)
	 */
	enable: async (id: string): Promise<PluginStatusResponse> => {
		const response = await api.post<PluginStatusResponse>(
			`/admin/plugins/${id}/enable`,
		);
		return response.data;
	},

	/**
	 * Disable a plugin (Admin only)
	 */
	disable: async (id: string): Promise<PluginStatusResponse> => {
		const response = await api.post<PluginStatusResponse>(
			`/admin/plugins/${id}/disable`,
		);
		return response.data;
	},

	/**
	 * Test a plugin connection (Admin only)
	 * Spawns the plugin process, sends an initialize request, and returns the manifest.
	 */
	test: async (id: string): Promise<PluginTestResult> => {
		const response = await api.post<PluginTestResult>(
			`/admin/plugins/${id}/test`,
		);
		return response.data;
	},

	/**
	 * Get plugin health information (Admin only)
	 */
	getHealth: async (id: string): Promise<PluginHealthResponse> => {
		const response = await api.get<PluginHealthResponse>(
			`/admin/plugins/${id}/health`,
		);
		return response.data;
	},

	/**
	 * Reset plugin failure count (Admin only)
	 * Clears the failure count and disabled reason, allowing the plugin to be used again.
	 */
	resetFailures: async (id: string): Promise<PluginStatusResponse> => {
		const response = await api.post<PluginStatusResponse>(
			`/admin/plugins/${id}/reset`,
		);
		return response.data;
	},

	/**
	 * Get plugin failure history (Admin only)
	 * Returns failure events with time-window statistics.
	 */
	getFailures: async (
		id: string,
		limit = 20,
		offset = 0,
	): Promise<PluginFailuresResponse> => {
		const response = await api.get<PluginFailuresResponse>(
			`/admin/plugins/${id}/failures`,
			{ params: { limit, offset } },
		);
		return response.data;
	},

	// ==========================================================================
	// Plugin Actions API (User-facing)
	// ==========================================================================

	/**
	 * Get available plugin actions for a specific scope
	 * @param scope - The scope to filter actions by (e.g., "series:detail")
	 * @param libraryId - Optional library ID to filter plugins by (only plugins that apply to this library)
	 */
	getActions: async (
		scope: PluginScope,
		libraryId?: string,
	): Promise<PluginActionsResponse> => {
		const params = new URLSearchParams({ scope });
		if (libraryId) {
			params.set("libraryId", libraryId);
		}
		const response = await api.get<PluginActionsResponse>(
			`/plugins/actions?${params.toString()}`,
		);
		return response.data;
	},

	/**
	 * Execute a plugin action
	 * Backend maps action + contentType to the appropriate protocol method
	 */
	execute: async (
		pluginId: string,
		request: ExecutePluginRequest,
	): Promise<ExecutePluginResponse> => {
		const response = await api.post<ExecutePluginResponse>(
			`/plugins/${pluginId}/execute`,
			request,
		);
		return response.data;
	},

	/**
	 * Search for metadata using a plugin
	 */
	searchMetadata: async (
		pluginId: string,
		query: string,
		contentType: MetadataContentType = "series",
	): Promise<ExecutePluginResponse> => {
		return pluginsApi.execute(pluginId, {
			action: {
				metadata: {
					action: "search",
					content_type: contentType,
					params: { query },
				},
			},
		});
	},

	/**
	 * Get full metadata from a plugin by external ID
	 */
	getMetadata: async (
		pluginId: string,
		externalId: string,
		contentType: MetadataContentType = "series",
	): Promise<ExecutePluginResponse> => {
		return pluginsApi.execute(pluginId, {
			action: {
				metadata: {
					action: "get",
					content_type: contentType,
					params: { externalId },
				},
			},
		});
	},
};

/**
 * Plugin Actions API for metadata operations on series and books
 */
export const pluginActionsApi = {
	/**
	 * Get the preprocessed search title for a series
	 * Applies plugin and library preprocessing rules to the series title
	 */
	getSearchTitle: async (
		seriesId: string,
		pluginId: string,
	): Promise<SearchTitleResponse> => {
		const response = await api.get<SearchTitleResponse>(
			`/series/${seriesId}/metadata/search-title`,
			{ params: { pluginId } },
		);
		return response.data;
	},

	/**
	 * Preview metadata from a plugin for a series (dry run)
	 * Returns field-by-field diff with status icons
	 */
	previewSeriesMetadata: async (
		seriesId: string,
		pluginId: string,
		externalId: string,
	): Promise<MetadataPreviewResponse> => {
		const response = await api.post<MetadataPreviewResponse>(
			`/series/${seriesId}/metadata/preview`,
			{ pluginId, externalId },
		);
		return response.data;
	},

	/**
	 * Apply metadata from a plugin to a series
	 * Respects RBAC permissions and field locks
	 */
	applySeriesMetadata: async (
		seriesId: string,
		pluginId: string,
		externalId: string,
		fields?: string[],
	): Promise<MetadataApplyResponse> => {
		const response = await api.post<MetadataApplyResponse>(
			`/series/${seriesId}/metadata/apply`,
			{ pluginId, externalId, fields },
		);
		return response.data;
	},

	/**
	 * Auto-match and apply metadata from a plugin to a series
	 * Searches for the best match and applies metadata in one step
	 */
	autoMatchSeriesMetadata: async (
		seriesId: string,
		pluginId: string,
		query?: string,
	): Promise<MetadataAutoMatchResponse> => {
		const response = await api.post<MetadataAutoMatchResponse>(
			`/series/${seriesId}/metadata/auto-match`,
			{ pluginId, query },
		);
		return response.data;
	},

	// ==========================================================================
	// Task-based Auto-Match API (Background Processing)
	// ==========================================================================

	/**
	 * Enqueue an auto-match task for a single series
	 * Runs asynchronously in a worker process
	 */
	enqueueAutoMatchTask: async (
		seriesId: string,
		pluginId: string,
	): Promise<EnqueueAutoMatchResponse> => {
		const response = await api.post<EnqueueAutoMatchResponse>(
			`/series/${seriesId}/metadata/auto-match/task`,
			{ pluginId },
		);
		return response.data;
	},

	/**
	 * Enqueue auto-match tasks for multiple series (bulk operation)
	 * Each series gets its own task
	 */
	enqueueBulkAutoMatchTasks: async (
		pluginId: string,
		seriesIds: string[],
	): Promise<EnqueueAutoMatchResponse> => {
		const response = await api.post<EnqueueAutoMatchResponse>(
			"/series/metadata/auto-match/task/bulk",
			{ pluginId, seriesIds },
		);
		return response.data;
	},

	/**
	 * Enqueue auto-match tasks for all series in a library
	 * Creates a task for each series in the library
	 */
	enqueueLibraryAutoMatchTasks: async (
		libraryId: string,
		pluginId: string,
	): Promise<EnqueueAutoMatchResponse> => {
		const response = await api.post<EnqueueAutoMatchResponse>(
			`/libraries/${libraryId}/metadata/auto-match/task`,
			{ pluginId },
		);
		return response.data;
	},
};
