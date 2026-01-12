import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type UserIntegrationDto = components["schemas"]["UserIntegrationDto"];
export type UserIntegrationsListResponse =
	components["schemas"]["UserIntegrationsListResponse"];
export type AvailableIntegrationDto =
	components["schemas"]["AvailableIntegrationDto"];
export type ConnectIntegrationRequest =
	components["schemas"]["ConnectIntegrationRequest"];
export type ConnectIntegrationResponse =
	components["schemas"]["ConnectIntegrationResponse"];
export type UpdateIntegrationSettingsRequest =
	components["schemas"]["UpdateIntegrationSettingsRequest"];
export type OAuthCallbackRequest =
	components["schemas"]["OAuthCallbackRequest"];
export type SyncTriggerResponse = components["schemas"]["SyncTriggerResponse"];

// Sync status values
export type SyncStatus = "idle" | "syncing" | "error" | "rate_limited";

// Integration provider names
export type IntegrationProvider =
	| "anilist"
	| "myanimelist"
	| "kitsu"
	| "mangadex"
	| "kavita";

// Auth type values
export type AuthType = "oauth2" | "api_key" | "none";

export const userIntegrationsApi = {
	/**
	 * Get all user integrations (connected and available)
	 */
	getAll: async (): Promise<UserIntegrationsListResponse> => {
		const response = await api.get<UserIntegrationsListResponse>(
			"/user/integrations",
		);
		return response.data;
	},

	/**
	 * Get a specific integration by name
	 */
	getByName: async (name: string): Promise<UserIntegrationDto> => {
		const response = await api.get<UserIntegrationDto>(
			`/user/integrations/${encodeURIComponent(name)}`,
		);
		return response.data;
	},

	/**
	 * Connect to an integration
	 * For OAuth2 integrations, this returns an auth URL to redirect the user to
	 * For API key integrations, this creates the connection immediately
	 */
	connect: async (
		request: ConnectIntegrationRequest,
	): Promise<ConnectIntegrationResponse> => {
		const response = await api.post<ConnectIntegrationResponse>(
			"/user/integrations",
			request,
		);
		return response.data;
	},

	/**
	 * Complete OAuth callback after user authorization
	 */
	oauthCallback: async (
		name: string,
		request: OAuthCallbackRequest,
	): Promise<UserIntegrationDto> => {
		const response = await api.post<UserIntegrationDto>(
			`/user/integrations/${encodeURIComponent(name)}/callback`,
			request,
		);
		return response.data;
	},

	/**
	 * Update integration settings
	 */
	update: async (
		name: string,
		request: UpdateIntegrationSettingsRequest,
	): Promise<UserIntegrationDto> => {
		const response = await api.patch<UserIntegrationDto>(
			`/user/integrations/${encodeURIComponent(name)}`,
			request,
		);
		return response.data;
	},

	/**
	 * Enable an integration
	 */
	enable: async (name: string): Promise<UserIntegrationDto> => {
		const response = await api.patch<UserIntegrationDto>(
			`/user/integrations/${encodeURIComponent(name)}`,
			{ enabled: true },
		);
		return response.data;
	},

	/**
	 * Disable an integration
	 */
	disable: async (name: string): Promise<UserIntegrationDto> => {
		const response = await api.patch<UserIntegrationDto>(
			`/user/integrations/${encodeURIComponent(name)}`,
			{ enabled: false },
		);
		return response.data;
	},

	/**
	 * Disconnect (delete) an integration
	 */
	disconnect: async (name: string): Promise<void> => {
		await api.delete(`/user/integrations/${encodeURIComponent(name)}`);
	},

	/**
	 * Trigger a manual sync for an integration
	 */
	sync: async (name: string): Promise<SyncTriggerResponse> => {
		const response = await api.post<SyncTriggerResponse>(
			`/user/integrations/${encodeURIComponent(name)}/sync`,
		);
		return response.data;
	},
};
