import { api } from "./client";

// =============================================================================
// Types
// =============================================================================

// Types defined manually until OpenAPI types are regenerated.
// These match the backend DTOs in src/api/routes/v1/dto/user_plugins.rs

/** User plugin instance (enabled by user) */
export interface UserPluginDto {
  id: string;
  pluginId: string;
  pluginName: string;
  pluginDisplayName: string;
  pluginType: string;
  enabled: boolean;
  connected: boolean;
  healthStatus: string;
  externalUsername?: string;
  externalAvatarUrl?: string;
  lastSyncAt?: string;
  lastSuccessAt?: string;
  requiresOauth: boolean;
  description?: string;
  config: Record<string, unknown>;
  createdAt: string;
}

/** Available plugin (not yet enabled by user) */
export interface AvailablePluginDto {
  pluginId: string;
  name: string;
  displayName: string;
  description?: string;
  requiresOauth: boolean;
  capabilities: UserPluginCapabilitiesDto;
}

/** Plugin capabilities (user plugin context) */
export interface UserPluginCapabilitiesDto {
  userSyncProvider: boolean;
  recommendationProvider: boolean;
}

/** List response with enabled and available plugins */
export interface UserPluginsListResponse {
  enabled: UserPluginDto[];
  available: AvailablePluginDto[];
}

/** OAuth start response */
export interface OAuthStartResponse {
  redirectUrl: string;
}

/** Update config request */
export interface UpdateUserPluginConfigRequest {
  config: Record<string, unknown>;
}

// =============================================================================
// API Client
// =============================================================================

export const userPluginsApi = {
  /**
   * List the current user's plugins (enabled and available)
   */
  list: async (): Promise<UserPluginsListResponse> => {
    const response = await api.get<UserPluginsListResponse>("/user/plugins");
    return response.data;
  },

  /**
   * Get a single user plugin instance
   */
  get: async (pluginId: string): Promise<UserPluginDto> => {
    const response = await api.get<UserPluginDto>(`/user/plugins/${pluginId}`);
    return response.data;
  },

  /**
   * Enable a plugin for the current user
   */
  enable: async (pluginId: string): Promise<UserPluginDto> => {
    const response = await api.post<UserPluginDto>(
      `/user/plugins/${pluginId}/enable`,
    );
    return response.data;
  },

  /**
   * Disable a plugin for the current user
   */
  disable: async (pluginId: string): Promise<{ success: boolean }> => {
    const response = await api.post<{ success: boolean }>(
      `/user/plugins/${pluginId}/disable`,
    );
    return response.data;
  },

  /**
   * Update user-specific plugin configuration
   */
  updateConfig: async (
    pluginId: string,
    config: Record<string, unknown>,
  ): Promise<UserPluginDto> => {
    const response = await api.patch<UserPluginDto>(
      `/user/plugins/${pluginId}/config`,
      { config } satisfies UpdateUserPluginConfigRequest,
    );
    return response.data;
  },

  /**
   * Disconnect a plugin (remove all data and credentials)
   */
  disconnect: async (pluginId: string): Promise<{ success: boolean }> => {
    const response = await api.delete<{ success: boolean }>(
      `/user/plugins/${pluginId}`,
    );
    return response.data;
  },

  /**
   * Start OAuth flow for a plugin
   * Returns a redirect URL to open in a popup window
   */
  startOAuth: async (pluginId: string): Promise<OAuthStartResponse> => {
    const response = await api.post<OAuthStartResponse>(
      `/user/plugins/${pluginId}/oauth/start`,
    );
    return response.data;
  },
};
