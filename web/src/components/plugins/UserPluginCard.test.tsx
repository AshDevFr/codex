import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AvailablePluginDto, UserPluginDto } from "@/api/userPlugins";
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
  description: "Sync your reading progress with AniList",
  config: {},
  createdAt: new Date().toISOString(),
};

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
  description: "Sync with MyAnimeList",
  config: {},
  createdAt: new Date().toISOString(),
};

const availablePlugin: AvailablePluginDto = {
  pluginId: "plugin-3",
  name: "smart-recs",
  displayName: "Smart Recommendations",
  description: "Get personalized manga recommendations",
  requiresOauth: false,
  capabilities: {
    syncProvider: false,
    recommendationProvider: true,
  },
};

const availableOAuthPlugin: AvailablePluginDto = {
  pluginId: "plugin-4",
  name: "sync-anilist",
  displayName: "AniList Sync",
  description: "Sync reading progress with AniList",
  requiresOauth: true,
  capabilities: {
    syncProvider: true,
    recommendationProvider: false,
  },
};

// =============================================================================
// ConnectedPluginCard Tests
// =============================================================================

describe("ConnectedPluginCard", () => {
  const defaultProps = {
    plugin: connectedPlugin,
    onDisconnect: vi.fn(),
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

  it("shows Connect button when not connected with OAuth", () => {
    renderWithProviders(
      <ConnectedPluginCard {...defaultProps} plugin={enabledNotConnected} />,
    );
    expect(
      screen.getByRole("button", { name: /^connect$/i }),
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
});

// =============================================================================
// AvailablePluginCard Tests
// =============================================================================

describe("AvailablePluginCard", () => {
  const defaultProps = {
    plugin: availablePlugin,
    onEnable: vi.fn(),
    onConnect: vi.fn(),
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

  it("shows Enable button for non-OAuth plugins", () => {
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

  it("shows Connect button for OAuth plugins", () => {
    renderWithProviders(
      <AvailablePluginCard {...defaultProps} plugin={availableOAuthPlugin} />,
    );
    expect(
      screen.getByRole("button", { name: /connect with/i }),
    ).toBeInTheDocument();
  });

  it("shows Sync badge for sync providers", () => {
    renderWithProviders(
      <AvailablePluginCard {...defaultProps} plugin={availableOAuthPlugin} />,
    );
    expect(screen.getByText("Sync")).toBeInTheDocument();
  });

  it("calls onConnect for OAuth plugin", async () => {
    const user = userEvent.setup();
    const onConnect = vi.fn();
    renderWithProviders(
      <AvailablePluginCard
        {...defaultProps}
        plugin={availableOAuthPlugin}
        onConnect={onConnect}
      />,
    );

    await user.click(screen.getByRole("button", { name: /connect with/i }));
    expect(onConnect).toHaveBeenCalledWith("plugin-4");
  });
});
