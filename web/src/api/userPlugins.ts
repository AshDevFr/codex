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
export type SyncTriggerResponse = components["schemas"]["SyncTriggerResponse"];
export type SyncStatusDto = components["schemas"]["SyncStatusDto"];
export type ConfigSchemaDto = components["schemas"]["ConfigSchemaDto"];
export type UserPluginTaskDto = components["schemas"]["UserPluginTaskDto"];

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

  /**
   * Trigger a sync operation for a plugin
   */
  triggerSync: async (pluginId: string): Promise<SyncTriggerResponse> => {
    const response = await api.post<SyncTriggerResponse>(
      `/user/plugins/${pluginId}/sync`,
    );
    return response.data;
  },

  /**
   * Get sync status for a plugin
   * Pass live=true to query the plugin process for real-time counts (more expensive)
   */
  getSyncStatus: async (
    pluginId: string,
    live = false,
  ): Promise<SyncStatusDto> => {
    const response = await api.get<SyncStatusDto>(
      `/user/plugins/${pluginId}/sync/status`,
      { params: live ? { live: true } : undefined },
    );
    return response.data;
  },

  /**
   * Set user credentials (personal access token)
   * Used when OAuth is not configured by admin
   */
  setCredentials: async (
    pluginId: string,
    accessToken: string,
  ): Promise<UserPluginDto> => {
    const response = await api.post<UserPluginDto>(
      `/user/plugins/${pluginId}/credentials`,
      { accessToken },
    );
    return response.data;
  },

  /**
   * Get the latest task for a plugin (user-scoped, no TasksRead permission needed)
   * Pass taskType to filter by type (e.g., "user_plugin_sync")
   */
  getPluginTask: async (
    pluginId: string,
    taskType?: string,
  ): Promise<UserPluginTaskDto> => {
    const response = await api.get<UserPluginTaskDto>(
      `/user/plugins/${pluginId}/tasks`,
      { params: taskType ? { type: taskType } : undefined },
    );
    return response.data;
  },
};
