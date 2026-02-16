import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type AllPluginStorageStatsDto =
  components["schemas"]["AllPluginStorageStatsDto"];
export type PluginStorageStatsDto =
  components["schemas"]["PluginStorageStatsDto"];
export type PluginCleanupResultDto =
  components["schemas"]["PluginCleanupResultDto"];

export const pluginStorageApi = {
  /**
   * Get storage statistics for all plugins (admin only)
   *
   * Returns file count and size per plugin, plus totals.
   */
  getStats: async (): Promise<AllPluginStorageStatsDto> => {
    const response = await api.get<AllPluginStorageStatsDto>(
      "/admin/plugin-storage",
    );
    return response.data;
  },

  /**
   * Get storage statistics for a specific plugin (admin only)
   */
  getPluginStats: async (name: string): Promise<PluginStorageStatsDto> => {
    const response = await api.get<PluginStorageStatsDto>(
      `/admin/plugin-storage/${name}`,
    );
    return response.data;
  },

  /**
   * Delete all storage files for a specific plugin (admin only)
   *
   * Removes the plugin's entire data directory. This is irreversible.
   */
  cleanupPlugin: async (name: string): Promise<PluginCleanupResultDto> => {
    const response = await api.delete<PluginCleanupResultDto>(
      `/admin/plugin-storage/${name}`,
    );
    return response.data;
  },
};
