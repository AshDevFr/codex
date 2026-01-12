import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";
import {
	userIntegrationsApi,
	type AvailableIntegrationDto,
	type ConnectIntegrationRequest,
	type SyncTriggerResponse,
	type UpdateIntegrationSettingsRequest,
	type UserIntegrationDto,
} from "@/api/userIntegrations";

export interface UserIntegrationsState {
	/**
	 * List of connected integrations
	 */
	integrations: UserIntegrationDto[];

	/**
	 * List of available integrations (including ones not yet connected)
	 */
	available: AvailableIntegrationDto[];

	/**
	 * Whether integrations have been loaded from the server
	 */
	isLoaded: boolean;

	/**
	 * Whether a load operation is in progress
	 */
	isLoading: boolean;

	/**
	 * Error message if loading failed
	 */
	loadError: string | null;

	/**
	 * Map of integration names to their current operation status
	 * Used for showing loading spinners during connect/disconnect/sync operations
	 */
	operationStatus: Record<string, "idle" | "loading" | "error">;

	/**
	 * Load all integrations from the server
	 */
	loadFromServer: () => Promise<void>;

	/**
	 * Get a specific integration by name
	 */
	getIntegration: (name: string) => UserIntegrationDto | undefined;

	/**
	 * Get an available integration by name
	 */
	getAvailableIntegration: (name: string) => AvailableIntegrationDto | undefined;

	/**
	 * Check if an integration is connected
	 */
	isConnected: (name: string) => boolean;

	/**
	 * Connect to an integration
	 * For OAuth2 integrations, returns the auth URL to redirect to
	 * For API key integrations, connects immediately
	 */
	connect: (
		request: ConnectIntegrationRequest,
	) => Promise<{ authUrl?: string; connected: boolean }>;

	/**
	 * Complete OAuth callback after user authorization
	 */
	completeOAuth: (
		name: string,
		code: string,
		state: string,
		redirectUri: string,
	) => Promise<UserIntegrationDto>;

	/**
	 * Update integration settings
	 */
	updateSettings: (
		name: string,
		request: UpdateIntegrationSettingsRequest,
	) => Promise<UserIntegrationDto>;

	/**
	 * Enable an integration
	 */
	enable: (name: string) => Promise<UserIntegrationDto>;

	/**
	 * Disable an integration
	 */
	disable: (name: string) => Promise<UserIntegrationDto>;

	/**
	 * Disconnect an integration
	 */
	disconnect: (name: string) => Promise<void>;

	/**
	 * Trigger a manual sync
	 */
	sync: (name: string) => Promise<SyncTriggerResponse>;

	/**
	 * Clear all cached integrations (used on logout)
	 */
	clearCache: () => void;
}

export const useUserIntegrationsStore = create<UserIntegrationsState>()(
	devtools(
		immer((set, get) => ({
			integrations: [],
			available: [],
			isLoaded: false,
			isLoading: false,
			loadError: null,
			operationStatus: {},

			loadFromServer: async () => {
				set((state) => {
					state.isLoading = true;
					state.loadError = null;
				});

				try {
					const response = await userIntegrationsApi.getAll();

					set((state) => {
						state.integrations = response.integrations;
						state.available = response.available;
						state.isLoaded = true;
						state.isLoading = false;
						state.loadError = null;
					});
				} catch (error) {
					const message =
						error instanceof Error
							? error.message
							: "Failed to load integrations";
					set((state) => {
						state.isLoading = false;
						state.loadError = message;
					});
					console.error("Failed to load integrations from server:", error);
					throw error;
				}
			},

			getIntegration: (name: string) => {
				return get().integrations.find((i) => i.integrationName === name);
			},

			getAvailableIntegration: (name: string) => {
				return get().available.find((i) => i.name === name);
			},

			isConnected: (name: string) => {
				return get().integrations.some((i) => i.integrationName === name);
			},

			connect: async (request) => {
				const { integrationName } = request;

				set((state) => {
					state.operationStatus[integrationName] = "loading";
				});

				try {
					const response = await userIntegrationsApi.connect(request);

					if (response.connected && response.integration) {
						// API key integration was connected immediately
						set((state) => {
							state.integrations.push(response.integration!);
							// Update available list to mark as connected
							const availableIdx = state.available.findIndex(
								(a) => a.name === integrationName,
							);
							if (availableIdx >= 0) {
								state.available[availableIdx].connected = true;
							}
							state.operationStatus[integrationName] = "idle";
						});
					} else {
						// OAuth flow - just clear the loading state
						set((state) => {
							state.operationStatus[integrationName] = "idle";
						});
					}

					return {
						authUrl: response.authUrl ?? undefined,
						connected: response.connected,
					};
				} catch (error) {
					set((state) => {
						state.operationStatus[integrationName] = "error";
					});
					throw error;
				}
			},

			completeOAuth: async (name, code, state, redirectUri) => {
				set((s) => {
					s.operationStatus[name] = "loading";
				});

				try {
					const integration = await userIntegrationsApi.oauthCallback(name, {
						code,
						state,
						redirectUri,
					});

					set((s) => {
						// Add to integrations list if not already there
						const existingIdx = s.integrations.findIndex(
							(i) => i.integrationName === name,
						);
						if (existingIdx >= 0) {
							s.integrations[existingIdx] = integration;
						} else {
							s.integrations.push(integration);
						}
						// Update available list to mark as connected
						const availableIdx = s.available.findIndex((a) => a.name === name);
						if (availableIdx >= 0) {
							s.available[availableIdx].connected = true;
						}
						s.operationStatus[name] = "idle";
					});

					return integration;
				} catch (error) {
					set((s) => {
						s.operationStatus[name] = "error";
					});
					throw error;
				}
			},

			updateSettings: async (name, request) => {
				set((state) => {
					state.operationStatus[name] = "loading";
				});

				try {
					const updated = await userIntegrationsApi.update(name, request);

					set((state) => {
						const idx = state.integrations.findIndex(
							(i) => i.integrationName === name,
						);
						if (idx >= 0) {
							state.integrations[idx] = updated;
						}
						state.operationStatus[name] = "idle";
					});

					return updated;
				} catch (error) {
					set((state) => {
						state.operationStatus[name] = "error";
					});
					throw error;
				}
			},

			enable: async (name) => {
				set((state) => {
					state.operationStatus[name] = "loading";
				});

				try {
					const updated = await userIntegrationsApi.enable(name);

					set((state) => {
						const idx = state.integrations.findIndex(
							(i) => i.integrationName === name,
						);
						if (idx >= 0) {
							state.integrations[idx] = updated;
						}
						state.operationStatus[name] = "idle";
					});

					return updated;
				} catch (error) {
					set((state) => {
						state.operationStatus[name] = "error";
					});
					throw error;
				}
			},

			disable: async (name) => {
				set((state) => {
					state.operationStatus[name] = "loading";
				});

				try {
					const updated = await userIntegrationsApi.disable(name);

					set((state) => {
						const idx = state.integrations.findIndex(
							(i) => i.integrationName === name,
						);
						if (idx >= 0) {
							state.integrations[idx] = updated;
						}
						state.operationStatus[name] = "idle";
					});

					return updated;
				} catch (error) {
					set((state) => {
						state.operationStatus[name] = "error";
					});
					throw error;
				}
			},

			disconnect: async (name) => {
				set((state) => {
					state.operationStatus[name] = "loading";
				});

				try {
					await userIntegrationsApi.disconnect(name);

					set((state) => {
						state.integrations = state.integrations.filter(
							(i) => i.integrationName !== name,
						);
						// Update available list to mark as disconnected
						const availableIdx = state.available.findIndex(
							(a) => a.name === name,
						);
						if (availableIdx >= 0) {
							state.available[availableIdx].connected = false;
						}
						state.operationStatus[name] = "idle";
					});
				} catch (error) {
					set((state) => {
						state.operationStatus[name] = "error";
					});
					throw error;
				}
			},

			sync: async (name) => {
				set((state) => {
					state.operationStatus[name] = "loading";
				});

				try {
					const response = await userIntegrationsApi.sync(name);

					set((state) => {
						// Update the integration with the response
						const idx = state.integrations.findIndex(
							(i) => i.integrationName === name,
						);
						if (idx >= 0) {
							state.integrations[idx] = response.integration;
						}
						state.operationStatus[name] = "idle";
					});

					return response;
				} catch (error) {
					set((state) => {
						state.operationStatus[name] = "error";
					});
					throw error;
				}
			},

			clearCache: () => {
				set((state) => {
					state.integrations = [];
					state.available = [];
					state.isLoaded = false;
					state.isLoading = false;
					state.loadError = null;
					state.operationStatus = {};
				});
			},
		})),
		{
			name: "UserIntegrations",
			enabled: import.meta.env.DEV,
		},
	),
);

// =============================================================================
// Performance Selectors
// =============================================================================

/**
 * Select all connected integrations.
 */
export const selectIntegrations = (
	state: UserIntegrationsState,
): UserIntegrationDto[] => state.integrations;

/**
 * Select all available integrations.
 */
export const selectAvailableIntegrations = (
	state: UserIntegrationsState,
): AvailableIntegrationDto[] => state.available;

/**
 * Select a specific integration by name.
 */
export const selectIntegration =
	(name: string) =>
	(state: UserIntegrationsState): UserIntegrationDto | undefined =>
		state.integrations.find((i) => i.integrationName === name);

/**
 * Select whether an integration is connected.
 */
export const selectIsConnected =
	(name: string) =>
	(state: UserIntegrationsState): boolean =>
		state.integrations.some((i) => i.integrationName === name);

/**
 * Select whether integrations have been loaded.
 */
export const selectIsLoaded = (state: UserIntegrationsState): boolean =>
	state.isLoaded;

/**
 * Select whether a load operation is in progress.
 */
export const selectIsLoading = (state: UserIntegrationsState): boolean =>
	state.isLoading;

/**
 * Select the load error, if any.
 */
export const selectLoadError = (state: UserIntegrationsState): string | null =>
	state.loadError;

/**
 * Select the operation status for an integration.
 */
export const selectOperationStatus =
	(name: string) =>
	(state: UserIntegrationsState): "idle" | "loading" | "error" =>
		state.operationStatus[name] ?? "idle";
