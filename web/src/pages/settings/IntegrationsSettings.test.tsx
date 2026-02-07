import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { UserPluginsListResponse } from "@/api/userPlugins";
import { renderWithProviders, userEvent } from "@/test/utils";
import { IntegrationsSettings } from "./IntegrationsSettings";

// Mock the user plugins API
vi.mock("@/api/userPlugins", () => ({
  userPluginsApi: {
    list: vi.fn(),
    enable: vi.fn(),
    disable: vi.fn(),
    disconnect: vi.fn(),
    startOAuth: vi.fn(),
    get: vi.fn(),
    updateConfig: vi.fn(),
  },
}));

// Mock the OAuth flow hooks
vi.mock("@/components/plugins/OAuthFlow", () => ({
  useOAuthCallback: vi.fn(),
  useOAuthFlow: () => ({
    startOAuthFlow: vi.fn(),
  }),
}));

// Import mocked module for test manipulation
import { userPluginsApi } from "@/api/userPlugins";

const emptyResponse: UserPluginsListResponse = {
  enabled: [],
  available: [],
};

const responseWithAvailable: UserPluginsListResponse = {
  enabled: [],
  available: [
    {
      pluginId: "plugin-1",
      name: "anilist-sync",
      displayName: "AniList Sync",
      description: "Sync reading progress with AniList",
      requiresOauth: true,
      capabilities: { syncProvider: true, recommendationProvider: false },
    },
    {
      pluginId: "plugin-2",
      name: "smart-recs",
      displayName: "Smart Recommendations",
      description: "Get personalized recommendations",
      requiresOauth: false,
      capabilities: { syncProvider: false, recommendationProvider: true },
    },
  ],
};

const responseWithEnabled: UserPluginsListResponse = {
  enabled: [
    {
      id: "inst-1",
      pluginId: "plugin-1",
      pluginName: "anilist-sync",
      pluginDisplayName: "AniList Sync",
      pluginType: "user",
      enabled: true,
      connected: true,
      healthStatus: "healthy",
      externalUsername: "@testuser",
      lastSyncAt: new Date().toISOString(),
      lastSuccessAt: new Date().toISOString(),
      requiresOauth: true,
      description: "Sync reading progress with AniList",
      config: {},
      createdAt: new Date().toISOString(),
    },
  ],
  available: [],
};

describe("IntegrationsSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(userPluginsApi.list).mockResolvedValue(emptyResponse);
  });

  it("renders page title", async () => {
    renderWithProviders(<IntegrationsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Integrations")).toBeInTheDocument();
    });
  });

  it("shows loading state initially", () => {
    renderWithProviders(<IntegrationsSettings />);
    expect(screen.getByText("Loading integrations...")).toBeInTheDocument();
  });

  it("shows empty state when no plugins available", async () => {
    renderWithProviders(<IntegrationsSettings />);

    await waitFor(() => {
      expect(screen.getByText("No integrations available")).toBeInTheDocument();
    });
  });

  it("shows available plugins section", async () => {
    vi.mocked(userPluginsApi.list).mockResolvedValue(responseWithAvailable);

    renderWithProviders(<IntegrationsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Available Plugins")).toBeInTheDocument();
      expect(screen.getByText("AniList Sync")).toBeInTheDocument();
      expect(screen.getByText("Smart Recommendations")).toBeInTheDocument();
    });
  });

  it("shows connected services section", async () => {
    vi.mocked(userPluginsApi.list).mockResolvedValue(responseWithEnabled);

    renderWithProviders(<IntegrationsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Connected Services")).toBeInTheDocument();
      expect(screen.getByText("AniList Sync")).toBeInTheDocument();
      expect(screen.getByText("Connected")).toBeInTheDocument();
    });
  });

  it("shows external username for connected plugins", async () => {
    vi.mocked(userPluginsApi.list).mockResolvedValue(responseWithEnabled);

    renderWithProviders(<IntegrationsSettings />);

    await waitFor(() => {
      expect(screen.getByText("@testuser")).toBeInTheDocument();
    });
  });

  it("shows disconnect confirmation modal", async () => {
    vi.mocked(userPluginsApi.list).mockResolvedValue(responseWithEnabled);
    const user = userEvent.setup();

    renderWithProviders(<IntegrationsSettings />);

    await waitFor(() => {
      expect(screen.getByText("AniList Sync")).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: /disconnect/i }));

    await waitFor(() => {
      expect(screen.getByText("Disconnect Plugin")).toBeInTheDocument();
      expect(
        screen.getByText(/are you sure you want to disconnect/i),
      ).toBeInTheDocument();
    });
  });

  it("shows error state when API fails", async () => {
    vi.mocked(userPluginsApi.list).mockRejectedValue(
      new Error("Network error"),
    );

    renderWithProviders(<IntegrationsSettings />);

    await waitFor(() => {
      expect(
        screen.getByText("Error loading integrations"),
      ).toBeInTheDocument();
    });
  });
});
