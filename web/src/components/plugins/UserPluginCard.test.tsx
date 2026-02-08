import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type {
  AvailablePluginDto,
  SyncStatusDto,
  UserPluginDto,
} from "@/api/userPlugins";
import { renderWithProviders, userEvent } from "@/test/utils";
import { AvailablePluginCard, ConnectedPluginCard } from "./UserPluginCard";

// =============================================================================
// Test Data
// =============================================================================

const connectedPlugin: UserPluginDto = {
  id: "inst-1",
  pluginId: "plugin-1",
  pluginName: "sync-anilist",
  pluginDisplayName: "AniList Sync",
  pluginType: "user",
  enabled: true,
  connected: true,
  healthStatus: "healthy",
  externalUsername: "@testuser",
  externalAvatarUrl: "https://example.com/avatar.png",
  lastSyncAt: new Date(Date.now() - 3600000).toISOString(), // 1 hour ago
  lastSuccessAt: new Date(Date.now() - 3600000).toISOString(),
  requiresOauth: true,
  oauthConfigured: true,
  description: "Sync your reading progress with AniList",
  config: {},
  capabilities: { readSync: true, userRecommendationProvider: false },
  createdAt: new Date().toISOString(),
} as UserPluginDto;

const enabledNotConnected: UserPluginDto = {
  id: "inst-2",
  pluginId: "plugin-2",
  pluginName: "sync-mal",
  pluginDisplayName: "MyAnimeList",
  pluginType: "user",
  enabled: true,
  connected: false,
  healthStatus: "unknown",
  requiresOauth: true,
  oauthConfigured: true,
  description: "Sync with MyAnimeList",
  config: {},
  capabilities: { readSync: true, userRecommendationProvider: false },
  createdAt: new Date().toISOString(),
} as UserPluginDto;

const availablePlugin: AvailablePluginDto = {
  pluginId: "plugin-3",
  name: "smart-recs",
  displayName: "Smart Recommendations",
  description: "Get personalized manga recommendations",
  requiresOauth: false,
  oauthConfigured: false,
  capabilities: {
    readSync: false,
    userRecommendationProvider: true,
  },
} as AvailablePluginDto;

const availableOAuthPlugin: AvailablePluginDto = {
  pluginId: "plugin-4",
  name: "sync-anilist",
  displayName: "AniList Sync",
  description: "Sync reading progress with AniList",
  requiresOauth: true,
  oauthConfigured: false,
  capabilities: {
    readSync: true,
    userRecommendationProvider: false,
  },
} as AvailablePluginDto;

// =============================================================================
// ConnectedPluginCard Tests
// =============================================================================

describe("ConnectedPluginCard", () => {
  const defaultProps = {
    plugin: connectedPlugin,
    onDisconnect: vi.fn(),
    onDisable: vi.fn(),
    onConnect: vi.fn(),
  };

  it("renders plugin display name", () => {
    renderWithProviders(<ConnectedPluginCard {...defaultProps} />);
    expect(screen.getByText("AniList Sync")).toBeInTheDocument();
  });

  it("renders description", () => {
    renderWithProviders(<ConnectedPluginCard {...defaultProps} />);
    expect(
      screen.getByText("Sync your reading progress with AniList"),
    ).toBeInTheDocument();
  });

  it("shows Connected badge when connected", () => {
    renderWithProviders(<ConnectedPluginCard {...defaultProps} />);
    expect(screen.getByText("Connected")).toBeInTheDocument();
  });

  it("shows external username when connected", () => {
    renderWithProviders(<ConnectedPluginCard {...defaultProps} />);
    expect(screen.getByText("@testuser")).toBeInTheDocument();
  });

  it("shows last sync time", () => {
    renderWithProviders(<ConnectedPluginCard {...defaultProps} />);
    expect(screen.getByText(/Last sync:/)).toBeInTheDocument();
  });

  it("shows Not Connected badge when not connected with OAuth", () => {
    renderWithProviders(
      <ConnectedPluginCard {...defaultProps} plugin={enabledNotConnected} />,
    );
    expect(screen.getByText("Not Connected")).toBeInTheDocument();
  });

  it("shows Connect button when not connected with OAuth configured", () => {
    renderWithProviders(
      <ConnectedPluginCard {...defaultProps} plugin={enabledNotConnected} />,
    );
    expect(
      screen.getByRole("button", { name: /connect with oauth/i }),
    ).toBeInTheDocument();
  });

  it("shows Disconnect button", () => {
    renderWithProviders(<ConnectedPluginCard {...defaultProps} />);
    expect(
      screen.getByRole("button", { name: /disconnect/i }),
    ).toBeInTheDocument();
  });

  it("calls onDisconnect when Disconnect is clicked", async () => {
    const user = userEvent.setup();
    const onDisconnect = vi.fn();
    renderWithProviders(
      <ConnectedPluginCard {...defaultProps} onDisconnect={onDisconnect} />,
    );

    await user.click(screen.getByRole("button", { name: /disconnect/i }));
    expect(onDisconnect).toHaveBeenCalledWith("plugin-1");
  });

  it("shows Sync Now button when connected with onSync handler", () => {
    const onSync = vi.fn();
    renderWithProviders(
      <ConnectedPluginCard {...defaultProps} onSync={onSync} />,
    );
    expect(
      screen.getByRole("button", { name: /sync now/i }),
    ).toBeInTheDocument();
  });

  it("calls onSync when Sync Now is clicked", async () => {
    const user = userEvent.setup();
    const onSync = vi.fn();
    renderWithProviders(
      <ConnectedPluginCard {...defaultProps} onSync={onSync} />,
    );

    await user.click(screen.getByRole("button", { name: /sync now/i }));
    expect(onSync).toHaveBeenCalledWith("plugin-1");
  });

  it("shows health badge", () => {
    renderWithProviders(<ConnectedPluginCard {...defaultProps} />);
    expect(screen.getByText("Healthy")).toBeInTheDocument();
  });

  // ---------------------------------------------------------------------------
  // Sync metrics tests
  // ---------------------------------------------------------------------------

  it("shows failure badge when syncStatus has failures", () => {
    const syncStatus: SyncStatusDto = {
      pluginId: "plugin-1",
      pluginName: "sync-anilist",
      connected: true,
      healthStatus: "healthy",
      failureCount: 3,
      enabled: true,
    } as SyncStatusDto;

    renderWithProviders(
      <ConnectedPluginCard {...defaultProps} syncStatus={syncStatus} />,
    );
    expect(screen.getByText(/3 failures/)).toBeInTheDocument();
  });

  it("shows external entry count from live status", () => {
    const syncStatus: SyncStatusDto = {
      pluginId: "plugin-1",
      pluginName: "sync-anilist",
      connected: true,
      healthStatus: "healthy",
      failureCount: 0,
      enabled: true,
      externalCount: 150,
    } as SyncStatusDto;

    renderWithProviders(
      <ConnectedPluginCard {...defaultProps} syncStatus={syncStatus} />,
    );
    expect(screen.getByText("150 external entries")).toBeInTheDocument();
  });

  it("shows pending pull and push counts", () => {
    const syncStatus: SyncStatusDto = {
      pluginId: "plugin-1",
      pluginName: "sync-anilist",
      connected: true,
      healthStatus: "healthy",
      failureCount: 0,
      enabled: true,
      pendingPull: 5,
      pendingPush: 3,
    } as SyncStatusDto;

    renderWithProviders(
      <ConnectedPluginCard {...defaultProps} syncStatus={syncStatus} />,
    );
    expect(screen.getByText("5 to pull")).toBeInTheDocument();
    expect(screen.getByText("3 to push")).toBeInTheDocument();
  });

  it("does not show sync metrics for non-sync plugins", () => {
    const nonSyncPlugin: UserPluginDto = {
      ...connectedPlugin,
      capabilities: { readSync: false, userRecommendationProvider: true },
    } as UserPluginDto;

    const syncStatus: SyncStatusDto = {
      pluginId: "plugin-1",
      pluginName: "sync-anilist",
      connected: true,
      healthStatus: "healthy",
      failureCount: 0,
      enabled: true,
      externalCount: 100,
    } as SyncStatusDto;

    renderWithProviders(
      <ConnectedPluginCard
        {...defaultProps}
        plugin={nonSyncPlugin}
        syncStatus={syncStatus}
      />,
    );
    expect(screen.queryByText("100 external entries")).not.toBeInTheDocument();
  });

  it("does not show sync metrics when plugin is not connected", () => {
    const syncStatus: SyncStatusDto = {
      pluginId: "plugin-2",
      pluginName: "sync-mal",
      connected: false,
      healthStatus: "unknown",
      failureCount: 0,
      enabled: true,
      externalCount: 100,
    } as SyncStatusDto;

    renderWithProviders(
      <ConnectedPluginCard
        {...defaultProps}
        plugin={enabledNotConnected}
        syncStatus={syncStatus}
      />,
    );
    expect(screen.queryByText("100 external entries")).not.toBeInTheDocument();
  });

  it("shows Settings button when onSettings provided", () => {
    const onSettings = vi.fn();
    renderWithProviders(
      <ConnectedPluginCard {...defaultProps} onSettings={onSettings} />,
    );
    expect(
      screen.getByRole("button", { name: /settings/i }),
    ).toBeInTheDocument();
  });

  it("calls onSettings when Settings is clicked", async () => {
    const user = userEvent.setup();
    const onSettings = vi.fn();
    renderWithProviders(
      <ConnectedPluginCard {...defaultProps} onSettings={onSettings} />,
    );

    await user.click(screen.getByRole("button", { name: /settings/i }));
    expect(onSettings).toHaveBeenCalledWith("plugin-1");
  });

  // ---------------------------------------------------------------------------
  // Last sync result display
  // ---------------------------------------------------------------------------

  it("shows last sync result summary", () => {
    const plugin: UserPluginDto = {
      ...connectedPlugin,
      lastSyncResult: {
        pulled: 10,
        matched: 8,
        applied: 6,
        pushed: 5,
        pushFailures: 0,
      },
    } as UserPluginDto;

    renderWithProviders(
      <ConnectedPluginCard {...defaultProps} plugin={plugin} />,
    );
    expect(
      screen.getByText("Pulled 10 (8 matched, 6 applied), pushed 5"),
    ).toBeInTheDocument();
  });

  it("shows skipped reason when sync was skipped", () => {
    const plugin: UserPluginDto = {
      ...connectedPlugin,
      lastSyncResult: {
        skippedReason: "Plugin not connected",
      },
    } as UserPluginDto;

    renderWithProviders(
      <ConnectedPluginCard {...defaultProps} plugin={plugin} />,
    );
    expect(
      screen.getByText("Skipped: Plugin not connected"),
    ).toBeInTheDocument();
  });

  it("does not show sync result when absent", () => {
    renderWithProviders(<ConnectedPluginCard {...defaultProps} />);
    expect(screen.queryByText(/Pulled/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Skipped/)).not.toBeInTheDocument();
  });
});

// =============================================================================
// AvailablePluginCard Tests
// =============================================================================

describe("AvailablePluginCard", () => {
  const defaultProps = {
    plugin: availablePlugin,
    onEnable: vi.fn(),
  };

  it("renders plugin display name", () => {
    renderWithProviders(<AvailablePluginCard {...defaultProps} />);
    expect(screen.getByText("Smart Recommendations")).toBeInTheDocument();
  });

  it("renders description", () => {
    renderWithProviders(<AvailablePluginCard {...defaultProps} />);
    expect(
      screen.getByText("Get personalized manga recommendations"),
    ).toBeInTheDocument();
  });

  it("shows Recommendations badge for recommendation plugins", () => {
    renderWithProviders(<AvailablePluginCard {...defaultProps} />);
    expect(screen.getByText("Recommendations")).toBeInTheDocument();
  });

  it("shows Enable button", () => {
    renderWithProviders(<AvailablePluginCard {...defaultProps} />);
    expect(screen.getByRole("button", { name: /enable/i })).toBeInTheDocument();
  });

  it("calls onEnable when Enable is clicked", async () => {
    const user = userEvent.setup();
    const onEnable = vi.fn();
    renderWithProviders(
      <AvailablePluginCard {...defaultProps} onEnable={onEnable} />,
    );

    await user.click(screen.getByRole("button", { name: /enable/i }));
    expect(onEnable).toHaveBeenCalledWith("plugin-3");
  });

  it("shows Sync badge for sync providers", () => {
    renderWithProviders(
      <AvailablePluginCard {...defaultProps} plugin={availableOAuthPlugin} />,
    );
    expect(screen.getByText("Sync")).toBeInTheDocument();
  });
});
