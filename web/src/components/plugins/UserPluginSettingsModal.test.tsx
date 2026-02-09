import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { UserPluginDto } from "@/api/userPlugins";
import { renderWithProviders, userEvent } from "@/test/utils";
import { UserPluginSettingsModal } from "./UserPluginSettingsModal";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makePlugin(overrides: Partial<UserPluginDto> = {}): UserPluginDto {
  return {
    id: "00000000-0000-0000-0000-000000000001",
    pluginId: "00000000-0000-0000-0000-000000000002",
    pluginName: "sync-anilist",
    pluginDisplayName: "AniList Sync",
    pluginType: "user",
    enabled: true,
    connected: true,
    healthStatus: "healthy",
    requiresOauth: true,
    oauthConfigured: true,
    config: {},
    capabilities: { readSync: true, userRecommendationProvider: false },
    createdAt: new Date().toISOString(),
    ...overrides,
  } as UserPluginDto;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("UserPluginSettingsModal", () => {
  it("shows syncMode select for sync plugins", () => {
    const plugin = makePlugin({
      capabilities: { readSync: true, userRecommendationProvider: false },
    });

    renderWithProviders(
      <UserPluginSettingsModal
        plugin={plugin}
        opened={true}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByText("Sync Mode")).toBeInTheDocument();
  });

  it("does not show syncMode select for non-sync plugins", () => {
    const plugin = makePlugin({
      capabilities: { readSync: false, userRecommendationProvider: true },
    });

    renderWithProviders(
      <UserPluginSettingsModal
        plugin={plugin}
        opened={true}
        onClose={vi.fn()}
      />,
    );

    expect(screen.queryByText("Sync Mode")).not.toBeInTheDocument();
  });

  it("renders dynamic fields from userConfigSchema", () => {
    const plugin = makePlugin({
      userConfigSchema: {
        description: "User preferences",
        fields: [
          {
            key: "scoreFormat",
            label: "Score Format",
            description: "How scores are mapped",
            type: "string",
            required: false,
          },
        ],
      },
    });

    renderWithProviders(
      <UserPluginSettingsModal
        plugin={plugin}
        opened={true}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByText("Score Format")).toBeInTheDocument();
    expect(screen.getByText("How scores are mapped")).toBeInTheDocument();
  });

  it("renders boolean fields as switches", () => {
    const plugin = makePlugin({
      capabilities: { readSync: false, userRecommendationProvider: false },
      userConfigSchema: {
        fields: [
          {
            key: "includeNsfw",
            label: "Include NSFW",
            description: "Include adult content",
            type: "boolean",
            required: false,
            default: false,
          },
        ],
      },
    });

    renderWithProviders(
      <UserPluginSettingsModal
        plugin={plugin}
        opened={true}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByText("Include NSFW")).toBeInTheDocument();
    // Boolean fields render as Mantine Switch which uses role="switch"
    expect(screen.getByRole("switch")).toBeInTheDocument();
  });

  it("shows 'no configurable settings' when no fields and not sync", () => {
    const plugin = makePlugin({
      capabilities: { readSync: false, userRecommendationProvider: false },
      userConfigSchema: undefined,
    });

    renderWithProviders(
      <UserPluginSettingsModal
        plugin={plugin}
        opened={true}
        onClose={vi.fn()}
      />,
    );

    expect(
      screen.getByText("This plugin has no configurable settings."),
    ).toBeInTheDocument();
  });

  it("initialises syncMode from existing config", () => {
    const plugin = makePlugin({
      config: { syncMode: "pull" },
      capabilities: { readSync: true, userRecommendationProvider: false },
    });

    renderWithProviders(
      <UserPluginSettingsModal
        plugin={plugin}
        opened={true}
        onClose={vi.fn()}
      />,
    );

    // The select should show the current value
    expect(screen.getByText("Pull Only")).toBeInTheDocument();
  });

  it("shows Codex sync settings for sync plugins", () => {
    const plugin = makePlugin({
      capabilities: { readSync: true, userRecommendationProvider: false },
    });

    renderWithProviders(
      <UserPluginSettingsModal
        plugin={plugin}
        opened={true}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByText("Include completed series")).toBeInTheDocument();
    expect(screen.getByText("Include in-progress series")).toBeInTheDocument();
    expect(screen.getByText("Count partially-read books")).toBeInTheDocument();
    expect(screen.getByText("Sync ratings & notes")).toBeInTheDocument();
    expect(screen.getByText("Sync Settings")).toBeInTheDocument();
  });

  it("does not show Codex sync settings for non-sync plugins", () => {
    const plugin = makePlugin({
      capabilities: { readSync: false, userRecommendationProvider: true },
    });

    renderWithProviders(
      <UserPluginSettingsModal
        plugin={plugin}
        opened={true}
        onClose={vi.fn()}
      />,
    );

    expect(
      screen.queryByText("Include completed series"),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Sync Settings")).not.toBeInTheDocument();
  });

  it("initialises Codex sync settings from _codex namespace", () => {
    const plugin = makePlugin({
      config: {
        _codex: {
          includeCompleted: false,
          includeInProgress: true,
          countPartialProgress: true,
          syncRatings: false,
        },
      },
      capabilities: { readSync: true, userRecommendationProvider: false },
    });

    renderWithProviders(
      <UserPluginSettingsModal
        plugin={plugin}
        opened={true}
        onClose={vi.fn()}
      />,
    );

    // Check that the switches reflect the stored values
    const switches = screen.getAllByRole("switch");
    // Order: includeCompleted, includeInProgress, countPartialProgress, syncRatings
    expect(switches[0]).not.toBeChecked(); // includeCompleted = false
    expect(switches[1]).toBeChecked(); // includeInProgress = true
    expect(switches[2]).toBeChecked(); // countPartialProgress = true
    expect(switches[3]).not.toBeChecked(); // syncRatings = false
  });

  it("shows Plugin Settings divider when plugin has config fields", () => {
    const plugin = makePlugin({
      capabilities: { readSync: true, userRecommendationProvider: false },
      userConfigSchema: {
        fields: [
          {
            key: "progressUnit",
            label: "Progress Unit",
            type: "string",
            required: false,
          },
        ],
      },
    });

    renderWithProviders(
      <UserPluginSettingsModal
        plugin={plugin}
        opened={true}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByText("Plugin Settings")).toBeInTheDocument();
    expect(screen.getByText("Progress Unit")).toBeInTheDocument();
  });

  it("calls onClose when Cancel is clicked", async () => {
    const onClose = vi.fn();
    const plugin = makePlugin();

    renderWithProviders(
      <UserPluginSettingsModal
        plugin={plugin}
        opened={true}
        onClose={onClose}
      />,
    );

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(onClose).toHaveBeenCalledOnce();
  });
});
