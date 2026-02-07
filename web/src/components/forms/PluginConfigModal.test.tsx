import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PluginDto } from "@/api/plugins";
import { renderWithProviders, screen, waitFor } from "@/test/utils";
import { PluginConfigModal } from "./PluginConfigModal";

// Mock the APIs
vi.mock("@/api/plugins", async () => {
  const actual = await vi.importActual("@/api/plugins");
  return {
    ...actual,
    pluginsApi: {
      update: vi.fn(),
    },
  };
});

vi.mock("@mantine/notifications", () => ({
  notifications: {
    show: vi.fn(),
  },
}));

vi.mock("@/utils/templateUtils", () => ({
  SAMPLE_SERIES_CONTEXT: {
    seriesId: "test-id",
    bookCount: 10,
    metadata: {
      title: "One Piece (Digital)",
      titleSort: "One Piece",
      year: 1999,
      publisher: "Shueisha",
      language: "en",
      status: "ongoing",
      ageRating: null,
      genres: ["Action", "Adventure"],
      tags: ["pirates"],
    },
  },
}));

// Minimal mock for sub-editors to avoid deep dependency issues
vi.mock("./PreprocessingRulesEditor", () => ({
  PreprocessingRulesEditor: () => (
    <div data-testid="preprocessing-rules-editor" />
  ),
}));

vi.mock("./ConditionsEditor", () => ({
  ConditionsEditor: () => <div data-testid="conditions-editor" />,
}));

function createMockPlugin(overrides: Partial<PluginDto> = {}): PluginDto {
  return {
    id: "plugin-1",
    name: "test-plugin",
    displayName: "Test Plugin",
    description: "A test plugin",
    pluginType: "system",
    command: "node",
    args: ["index.js"],
    workingDirectory: null,
    env: {},
    permissions: ["metadata:read"],
    scopes: ["series:detail"],
    libraryIds: [],
    credentialDelivery: "env",
    hasCredentials: false,
    config: {},
    searchPreprocessingRules: null,
    autoMatchConditions: null,
    metadataTargets: null,
    searchQueryTemplate: null,
    useExistingExternalId: false,
    enabled: true,
    healthStatus: "healthy",
    failureCount: 0,
    lastFailureAt: null,
    lastSuccessAt: null,
    disabledReason: null,
    manifest: null,
    rateLimitRequestsPerMinute: 60,
    createdAt: "2024-01-01T00:00:00Z",
    updatedAt: "2024-01-01T00:00:00Z",
    ...overrides,
  } as PluginDto;
}

const mockLibraries = [
  { id: "lib-1", name: "Comics" },
  { id: "lib-2", name: "Manga" },
];

describe("PluginConfigModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders with Permissions tab for any plugin", () => {
    const plugin = createMockPlugin();
    renderWithProviders(
      <PluginConfigModal
        plugin={plugin}
        opened={true}
        onClose={vi.fn()}
        libraries={mockLibraries}
      />,
    );

    expect(
      screen.getByRole("tab", { name: /Permissions/ }),
    ).toBeInTheDocument();
  });

  it("shows search tabs for metadata provider plugins", () => {
    const plugin = createMockPlugin({
      manifest: {
        name: "metadata-plugin",
        displayName: "Metadata Plugin",
        version: "1.0.0",
        protocolVersion: "1.0",
        description: "A metadata plugin",
        capabilities: {
          metadataProvider: ["series"],
          userSyncProvider: false,
        },
        contentTypes: ["series"],
        requiredCredentials: [],
        scopes: ["series:detail"],
      },
    });

    renderWithProviders(
      <PluginConfigModal
        plugin={plugin}
        opened={true}
        onClose={vi.fn()}
        libraries={mockLibraries}
      />,
    );

    expect(
      screen.getByRole("tab", { name: /Permissions/ }),
    ).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Template/ })).toBeInTheDocument();
    expect(
      screen.getByRole("tab", { name: /Preprocessing/ }),
    ).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Conditions/ })).toBeInTheDocument();
  });

  it("hides search tabs for sync-only plugins", () => {
    const plugin = createMockPlugin({
      manifest: {
        name: "sync-plugin",
        displayName: "Sync Plugin",
        version: "1.0.0",
        protocolVersion: "1.0",
        description: "A sync plugin",
        capabilities: {
          metadataProvider: [],
          userSyncProvider: true,
        },
        contentTypes: [],
        requiredCredentials: [],
        scopes: [],
      },
    });

    renderWithProviders(
      <PluginConfigModal
        plugin={plugin}
        opened={true}
        onClose={vi.fn()}
        libraries={mockLibraries}
      />,
    );

    expect(
      screen.getByRole("tab", { name: /Permissions/ }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("tab", { name: /Template/ }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("tab", { name: /Preprocessing/ }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("tab", { name: /Conditions/ }),
    ).not.toBeInTheDocument();
  });

  it("hides search tabs for plugins with no manifest", () => {
    const plugin = createMockPlugin({ manifest: null });

    renderWithProviders(
      <PluginConfigModal
        plugin={plugin}
        opened={true}
        onClose={vi.fn()}
        libraries={mockLibraries}
      />,
    );

    expect(
      screen.getByRole("tab", { name: /Permissions/ }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("tab", { name: /Template/ }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("tab", { name: /Preprocessing/ }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("tab", { name: /Conditions/ }),
    ).not.toBeInTheDocument();
  });

  it("shows no-manifest warning when plugin has no manifest", async () => {
    const plugin = createMockPlugin({ manifest: null });

    renderWithProviders(
      <PluginConfigModal
        plugin={plugin}
        opened={true}
        onClose={vi.fn()}
        libraries={mockLibraries}
      />,
    );

    await waitFor(() => {
      expect(
        screen.getByText(/This plugin has not been tested yet/),
      ).toBeInTheDocument();
    });
  });

  it("does not show no-manifest warning when plugin has a manifest", () => {
    const plugin = createMockPlugin({
      manifest: {
        name: "metadata-plugin",
        displayName: "Metadata Plugin",
        version: "1.0.0",
        protocolVersion: "1.0",
        description: "A metadata plugin",
        capabilities: {
          metadataProvider: ["series"],
          userSyncProvider: false,
        },
        contentTypes: ["series"],
        requiredCredentials: [],
        scopes: ["series:detail"],
      },
    });

    renderWithProviders(
      <PluginConfigModal
        plugin={plugin}
        opened={true}
        onClose={vi.fn()}
        libraries={mockLibraries}
      />,
    );

    expect(
      screen.queryByText(/This plugin has not been tested yet/),
    ).not.toBeInTheDocument();
  });

  it("shows modal title with plugin display name", () => {
    const plugin = createMockPlugin({ displayName: "MangaBaka" });

    renderWithProviders(
      <PluginConfigModal
        plugin={plugin}
        opened={true}
        onClose={vi.fn()}
        libraries={mockLibraries}
      />,
    );

    expect(screen.getByText("Configure: MangaBaka")).toBeInTheDocument();
  });

  it("does not render when opened is false", () => {
    const plugin = createMockPlugin();

    renderWithProviders(
      <PluginConfigModal
        plugin={plugin}
        opened={false}
        onClose={vi.fn()}
        libraries={mockLibraries}
      />,
    );

    expect(
      screen.queryByText("Configure: Test Plugin"),
    ).not.toBeInTheDocument();
  });
});
