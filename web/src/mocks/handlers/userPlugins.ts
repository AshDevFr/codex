/**
 * User Plugins API mock handlers
 *
 * Provides mock data for:
 * - GET /api/v1/user/plugins (list user's plugins)
 * - GET /api/v1/user/plugins/:id (get single plugin)
 * - POST /api/v1/user/plugins/:id/enable (enable plugin)
 * - POST /api/v1/user/plugins/:id/disable (disable plugin)
 * - DELETE /api/v1/user/plugins/:id (disconnect plugin)
 */

import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";

type UserPluginDto = components["schemas"]["UserPluginDto"];
type AvailablePluginDto = components["schemas"]["AvailablePluginDto"];
type UserPluginsListResponse = components["schemas"]["UserPluginsListResponse"];

// =============================================================================
// Mock Data
// =============================================================================

/** AniList recommendation plugin - enabled and connected */
const anilistPlugin: UserPluginDto = {
  id: "00000000-0000-0000-0000-000000000010",
  pluginId: "00000000-0000-0000-0000-000000000001",
  pluginName: "anilist",
  pluginDisplayName: "AniList",
  pluginType: "system",
  description:
    "Sync your reading progress and get personalized manga recommendations powered by AniList.",
  enabled: true,
  connected: true,
  healthStatus: "healthy",
  oauthConfigured: true,
  requiresOauth: true,
  externalUsername: "manga_fan_42",
  externalAvatarUrl: "https://s4.anilist.co/file/anilistcdn/user/avatar.png",
  lastSuccessAt: "2026-02-11T20:00:00Z",
  lastSyncAt: "2026-02-11T18:30:00Z",
  lastSyncResult: {
    entriesSynced: 42,
    entriesSkipped: 3,
    errors: 0,
  },
  config: {},
  userConfigSchema: null,
  userSetupInstructions: null,
  capabilities: {
    readSync: true,
    userRecommendationProvider: true,
  },
  createdAt: "2026-01-15T00:00:00Z",
};

/** MangaBaka metadata plugin - available but not enabled by user */
const mangabakaAvailable: AvailablePluginDto = {
  pluginId: "plugin-mangabaka",
  name: "mangabaka",
  displayName: "MangaBaka",
  description: "Fetches manga metadata from MangaUpdates/Baka-Updates",
  oauthConfigured: false,
  requiresOauth: false,
  capabilities: {
    readSync: false,
    userRecommendationProvider: false,
  },
};

let enabledPlugins: UserPluginDto[] = [anilistPlugin];
const availablePlugins: AvailablePluginDto[] = [mangabakaAvailable];

// =============================================================================
// Handlers
// =============================================================================

export const userPluginsHandlers = [
  // GET /api/v1/user/plugins
  http.get("/api/v1/user/plugins", async () => {
    await delay(150);

    const response: UserPluginsListResponse = {
      enabled: enabledPlugins,
      available: availablePlugins,
    };

    return HttpResponse.json(response);
  }),

  // GET /api/v1/user/plugins/:id
  http.get("/api/v1/user/plugins/:id", async ({ params }) => {
    await delay(100);

    const plugin = enabledPlugins.find((p) => p.pluginId === params.id);
    if (!plugin) {
      return HttpResponse.json({ error: "Plugin not found" }, { status: 404 });
    }

    return HttpResponse.json(plugin);
  }),

  // POST /api/v1/user/plugins/:id/enable
  http.post("/api/v1/user/plugins/:id/enable", async ({ params }) => {
    await delay(200);

    const pluginId = params.id as string;
    const existing = enabledPlugins.find((p) => p.pluginId === pluginId);
    if (existing) {
      return HttpResponse.json({ ...existing, enabled: true });
    }

    // Create a new enabled instance
    const newPlugin: UserPluginDto = {
      id: crypto.randomUUID(),
      pluginId,
      pluginName: pluginId,
      pluginDisplayName: pluginId,
      pluginType: "system",
      description: null,
      enabled: true,
      connected: false,
      healthStatus: "unknown",
      oauthConfigured: false,
      requiresOauth: false,
      externalUsername: null,
      externalAvatarUrl: null,
      lastSuccessAt: null,
      lastSyncAt: null,
      lastSyncResult: null,
      config: {},
      userConfigSchema: null,
      userSetupInstructions: null,
      capabilities: {
        readSync: false,
        userRecommendationProvider: false,
      },
      createdAt: new Date().toISOString(),
    };

    enabledPlugins.push(newPlugin);
    return HttpResponse.json(newPlugin);
  }),

  // POST /api/v1/user/plugins/:id/disable
  http.post("/api/v1/user/plugins/:id/disable", async ({ params }) => {
    await delay(150);

    enabledPlugins = enabledPlugins.filter((p) => p.pluginId !== params.id);
    return HttpResponse.json({ success: true });
  }),

  // DELETE /api/v1/user/plugins/:id (disconnect)
  http.delete("/api/v1/user/plugins/:id", async ({ params }) => {
    await delay(200);

    enabledPlugins = enabledPlugins.filter((p) => p.pluginId !== params.id);
    return HttpResponse.json({ success: true });
  }),
];
