import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type SystemIntegrationDto =
	components["schemas"]["SystemIntegrationDto"];
export type SystemIntegrationsListResponse =
	components["schemas"]["SystemIntegrationsListResponse"];
export type CreateSystemIntegrationRequest =
	components["schemas"]["CreateSystemIntegrationRequest"];
export type UpdateSystemIntegrationRequest =
	components["schemas"]["UpdateSystemIntegrationRequest"];
export type IntegrationTestResult =
	components["schemas"]["IntegrationTestResult"];
export type IntegrationStatusResponse =
	components["schemas"]["IntegrationStatusResponse"];

// Integration types for typed operations
export type IntegrationType =
	| "metadata_provider"
	| "notification"
	| "storage"
	| "sync";

// Health status values
export type HealthStatus =
	| "unknown"
	| "healthy"
	| "degraded"
	| "unhealthy"
	| "disabled";

export const systemIntegrationsApi = {
	/**
	 * Get all system integrations (Admin only)
	 */
	getAll: async (): Promise<SystemIntegrationsListResponse> => {
		const response = await api.get<SystemIntegrationsListResponse>(
			"/admin/integrations",
		);
		return response.data;
	},

	/**
	 * Get a single system integration by ID (Admin only)
	 */
	getById: async (id: string): Promise<SystemIntegrationDto> => {
		const response = await api.get<SystemIntegrationDto>(
			`/admin/integrations/${id}`,
		);
		return response.data;
	},

	/**
	 * Create a new system integration (Admin only)
	 */
	create: async (
		request: CreateSystemIntegrationRequest,
	): Promise<SystemIntegrationDto> => {
		const response = await api.post<SystemIntegrationDto>(
			"/admin/integrations",
			request,
		);
		return response.data;
	},

	/**
	 * Update a system integration (Admin only)
	 */
	update: async (
		id: string,
		request: UpdateSystemIntegrationRequest,
	): Promise<SystemIntegrationDto> => {
		const response = await api.patch<SystemIntegrationDto>(
			`/admin/integrations/${id}`,
			request,
		);
		return response.data;
	},

	/**
	 * Delete a system integration (Admin only)
	 */
	delete: async (id: string): Promise<void> => {
		await api.delete(`/admin/integrations/${id}`);
	},

	/**
	 * Enable a system integration (Admin only)
	 */
	enable: async (id: string): Promise<IntegrationStatusResponse> => {
		const response = await api.post<IntegrationStatusResponse>(
			`/admin/integrations/${id}/enable`,
		);
		return response.data;
	},

	/**
	 * Disable a system integration (Admin only)
	 */
	disable: async (id: string): Promise<IntegrationStatusResponse> => {
		const response = await api.post<IntegrationStatusResponse>(
			`/admin/integrations/${id}/disable`,
		);
		return response.data;
	},

	/**
	 * Test a system integration connection (Admin only)
	 */
	test: async (id: string): Promise<IntegrationTestResult> => {
		const response = await api.post<IntegrationTestResult>(
			`/admin/integrations/${id}/test`,
		);
		return response.data;
	},
};
