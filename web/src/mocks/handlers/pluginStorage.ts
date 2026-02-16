/**
 * MSW handlers for plugin storage API endpoints
 */

import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";

type AllPluginStorageStatsDto =
  components["schemas"]["AllPluginStorageStatsDto"];
type PluginCleanupResultDto = components["schemas"]["PluginCleanupResultDto"];

// Mock data for plugin storage stats
let mockPluginStats: AllPluginStorageStatsDto = {
  plugins: [
    { pluginName: "metadata-anilist", fileCount: 12, totalBytes: 2_097_152 },
    {
      pluginName: "metadata-mangaupdates",
      fileCount: 1,
      totalBytes: 8_388_608,
    },
    { pluginName: "sync-kavita", fileCount: 3, totalBytes: 524_288 },
  ],
  totalFileCount: 16,
  totalBytes: 11_010_048,
};

export const pluginStorageHandlers = [
  // Get all plugin storage stats
  http.get("/api/v1/admin/plugin-storage", async () => {
    await delay(150);
    return HttpResponse.json(mockPluginStats);
  }),

  // Get specific plugin storage stats
  http.get("/api/v1/admin/plugin-storage/:name", async ({ params }) => {
    await delay(100);
    const { name } = params;
    const plugin = mockPluginStats.plugins.find((p) => p.pluginName === name);
    if (!plugin) {
      return new HttpResponse(null, { status: 404 });
    }
    return HttpResponse.json(plugin);
  }),

  // Delete plugin storage
  http.delete("/api/v1/admin/plugin-storage/:name", async ({ params }) => {
    await delay(300);
    const { name } = params;
    const pluginIndex = mockPluginStats.plugins.findIndex(
      (p) => p.pluginName === name,
    );

    if (pluginIndex === -1) {
      return new HttpResponse(null, { status: 404 });
    }

    const plugin = mockPluginStats.plugins[pluginIndex];
    const result: PluginCleanupResultDto = {
      filesDeleted: plugin.fileCount,
      bytesFreed: plugin.totalBytes,
      failures: 0,
    };

    // Remove from mock stats
    mockPluginStats = {
      ...mockPluginStats,
      plugins: mockPluginStats.plugins.filter((_, i) => i !== pluginIndex),
      totalFileCount: mockPluginStats.totalFileCount - plugin.fileCount,
      totalBytes: mockPluginStats.totalBytes - plugin.totalBytes,
    };

    return HttpResponse.json(result);
  }),
];

// Helper to reset mock state (useful for tests)
export function resetPluginStorageMockState() {
  mockPluginStats = {
    plugins: [
      { pluginName: "metadata-anilist", fileCount: 12, totalBytes: 2_097_152 },
      {
        pluginName: "metadata-mangaupdates",
        fileCount: 1,
        totalBytes: 8_388_608,
      },
      { pluginName: "sync-kavita", fileCount: 3, totalBytes: 524_288 },
    ],
    totalFileCount: 16,
    totalBytes: 11_010_048,
  };
}
