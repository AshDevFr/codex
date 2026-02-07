import type { components } from "@/types/api.generated";
import { api } from "./client";

// =============================================================================
// Types (from generated OpenAPI types)
// =============================================================================

export type UserPluginDto = components["schemas"]["UserPluginDto"];
export type AvailablePluginDto = components["schemas"]["AvailablePluginDto"];
export type UserPluginCapabilitiesDto =
  components["schemas"]["UserPluginCapabilitiesDto"];
export type UserPluginsListResponse =
  components["schemas"]["UserPluginsListResponse"];
export type OAuthStartResponse = components["schemas"]["OAuthStartResponse"];
export type UpdateUserPluginConfigRequest =
  components["schemas"]["UpdateUserPluginConfigRequest"];

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
