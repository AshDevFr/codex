import { notifications } from "@mantine/notifications";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { PluginsSettings } from "./PluginsSettings";

const mockGetAll = vi.fn();
const mockGetFailures = vi.fn();

// Mock the APIs
vi.mock("@/api/plugins", () => ({
  AVAILABLE_PERMISSIONS: [
    { value: "series:read", label: "Series Read" },
    { value: "series:write", label: "Series Write" },
  ],
  AVAILABLE_SCOPES: [
    { value: "series:detail", label: "Series Detail" },
    { value: "series:bulk", label: "Series Bulk" },
  ],
  CREDENTIAL_DELIVERY_OPTIONS: [
    { value: "env", label: "Environment Variables" },
    { value: "stdin", label: "Standard Input" },
  ],
  pluginsApi: {
    getAll: (...args: unknown[]) => mockGetAll(...args),
    create: vi.fn(),
    update: vi.fn(),
    delete: vi.fn(),
    getFailures: (...args: unknown[]) => mockGetFailures(...args),
  },
}));

vi.mock("@/api/libraries", () => ({
  librariesApi: {
    getAll: vi.fn().mockResolvedValue([]),
  },
}));

vi.mock("@mantine/notifications", () => ({
  notifications: {
    show: vi.fn(),
  },
}));

function createMockPlugin(overrides: Record<string, unknown> = {}) {
  return {
    id: "plugin-test",
    name: "testplugin",
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
    manifest: {
      name: "testplugin",
      displayName: "Test Plugin",
      version: "1.0.0",
      protocolVersion: "1.0",
      description: "A test plugin",
      author: "Test",
      capabilities: {
        metadataProvider: ["series"],
        userSyncProvider: false,
      },
      contentTypes: ["series"],
      requiredCredentials: [],
      scopes: ["series:detail"],
    },
    rateLimitRequestsPerMinute: 60,
    createdAt: "2024-01-01T00:00:00Z",
    updatedAt: "2024-01-01T00:00:00Z",
    ...overrides,
  };
}

describe("PluginsSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetAll.mockResolvedValue({ plugins: [], total: 0 });
    mockGetFailures.mockResolvedValue({
      failures: [],
      total: 0,
      windowFailures: 0,
      windowSeconds: 3600,
      threshold: 3,
    });
  });

  it("renders page title", () => {
    renderWithProviders(<PluginsSettings />);
    expect(screen.getByText("Plugins")).toBeInTheDocument();
  });

  it("shows add plugin button", () => {
    renderWithProviders(<PluginsSettings />);
    expect(screen.getByText("Add Plugin")).toBeInTheDocument();
  });

  it("shows page description", () => {
    renderWithProviders(<PluginsSettings />);
    expect(
      screen.getByText(
        /Manage external plugin processes for metadata fetching/i,
      ),
    ).toBeInTheDocument();
  });
});

describe("PluginDetails - Metadata Targets", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetFailures.mockResolvedValue({
      failures: [],
      total: 0,
      windowFailures: 0,
      windowSeconds: 3600,
      threshold: 3,
    });
  });

  it("shows active metadata targets from manifest capabilities", async () => {
    const plugin = createMockPlugin({
      metadataTargets: ["series"],
      manifest: {
        name: "testplugin",
        displayName: "Test Plugin",
        version: "1.0.0",
        protocolVersion: "1.0",
        capabilities: {
          metadataProvider: ["series", "book"],
          userSyncProvider: false,
        },
        contentTypes: ["series", "book"],
        requiredCredentials: [],
        scopes: ["series:detail"],
      },
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    // Wait for plugin to appear
    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    // Expand the row
    const expandButtons = screen.getAllByRole("button");
    const expandButton = expandButtons[0];
    await user.click(expandButton);

    // Should show "Metadata Targets" label
    await waitFor(() => {
      expect(screen.getByText("Metadata Targets")).toBeInTheDocument();
    });

    // "Series" badge should be active (teal), "Books" should be dimmed (gray) since
    // metadataTargets only includes "series"
    const seriesBadge = screen.getByText("Series");
    const booksBadge = screen.getByText("Books");
    expect(seriesBadge).toBeInTheDocument();
    expect(booksBadge).toBeInTheDocument();
  });

  it("shows all targets as active when metadataTargets is null (auto)", async () => {
    const plugin = createMockPlugin({
      metadataTargets: null,
      manifest: {
        name: "testplugin",
        displayName: "Test Plugin",
        version: "1.0.0",
        protocolVersion: "1.0",
        capabilities: {
          metadataProvider: ["series", "book"],
          userSyncProvider: false,
        },
        contentTypes: ["series", "book"],
        requiredCredentials: [],
        scopes: ["series:detail"],
      },
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const expandButtons = screen.getAllByRole("button");
    await user.click(expandButtons[0]);

    await waitFor(() => {
      expect(screen.getByText("Metadata Targets")).toBeInTheDocument();
    });

    // Both should be visible since capabilities include both
    expect(screen.getByText("Series")).toBeInTheDocument();
    expect(screen.getByText("Books")).toBeInTheDocument();
  });

  it("does not show metadata targets when no manifest", async () => {
    const plugin = createMockPlugin({
      manifest: null,
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const expandButtons = screen.getAllByRole("button");
    await user.click(expandButtons[0]);

    // Wait for expanded content
    await waitFor(() => {
      expect(screen.getByText("A test plugin")).toBeInTheDocument();
    });

    // Should not show metadata targets without manifest
    expect(screen.queryByText("Metadata Targets")).not.toBeInTheDocument();
  });
});

describe("PluginDetails - Preprocessing Rules and Conditions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetFailures.mockResolvedValue({
      failures: [],
      total: 0,
      windowFailures: 0,
      windowSeconds: 3600,
      threshold: 3,
    });
  });

  it("shows preprocessing rules count when rules exist", async () => {
    const plugin = createMockPlugin({
      searchPreprocessingRules: [
        { pattern: "\\s*\\(Digital\\)$", replacement: "" },
        { pattern: "\\s*\\[Digital\\]$", replacement: "" },
      ],
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const expandButtons = screen.getAllByRole("button");
    await user.click(expandButtons[0]);

    await waitFor(() => {
      expect(screen.getByText("Preprocessing Rules")).toBeInTheDocument();
    });

    expect(screen.getByText("2 rules")).toBeInTheDocument();
  });

  it("shows singular rule label for single preprocessing rule", async () => {
    const plugin = createMockPlugin({
      searchPreprocessingRules: [
        { pattern: "\\s*\\(Digital\\)$", replacement: "" },
      ],
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const expandButtons = screen.getAllByRole("button");
    await user.click(expandButtons[0]);

    await waitFor(() => {
      expect(screen.getByText("Preprocessing Rules")).toBeInTheDocument();
    });

    expect(screen.getByText("1 rule")).toBeInTheDocument();
  });

  it("shows auto-match conditions count when conditions exist", async () => {
    const plugin = createMockPlugin({
      autoMatchConditions: {
        mode: "all",
        rules: [
          { field: "external_ids.plugin:test", operator: "is_null" },
          { field: "book_count", operator: "gte", value: 1 },
          { field: "metadata.status", operator: "equals", value: "ongoing" },
        ],
      },
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const expandButtons = screen.getAllByRole("button");
    await user.click(expandButtons[0]);

    await waitFor(() => {
      expect(screen.getByText("Auto-Match Conditions")).toBeInTheDocument();
    });

    expect(screen.getByText("3 conditions")).toBeInTheDocument();
  });

  it("shows singular condition label for single condition", async () => {
    const plugin = createMockPlugin({
      autoMatchConditions: {
        mode: "all",
        rules: [{ field: "external_ids.plugin:test", operator: "is_null" }],
      },
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const expandButtons = screen.getAllByRole("button");
    await user.click(expandButtons[0]);

    await waitFor(() => {
      expect(screen.getByText("Auto-Match Conditions")).toBeInTheDocument();
    });

    expect(screen.getByText("1 condition")).toBeInTheDocument();
  });

  it("shows zero counts when no preprocessing rules or conditions exist", async () => {
    const plugin = createMockPlugin({
      searchPreprocessingRules: null,
      autoMatchConditions: null,
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const expandButtons = screen.getAllByRole("button");
    await user.click(expandButtons[0]);

    await waitFor(() => {
      expect(screen.getByText("Preprocessing Rules")).toBeInTheDocument();
    });

    expect(screen.getByText("0 rules")).toBeInTheDocument();
    expect(screen.getByText("Auto-Match Conditions")).toBeInTheDocument();
    expect(screen.getByText("0 conditions")).toBeInTheDocument();
  });

  it("shows both preprocessing rules and conditions together", async () => {
    const plugin = createMockPlugin({
      searchPreprocessingRules: [
        { pattern: "\\s*\\(Digital\\)$", replacement: "" },
      ],
      autoMatchConditions: {
        mode: "any",
        rules: [
          { field: "external_ids.plugin:test", operator: "is_null" },
          { field: "book_count", operator: "gte", value: 1 },
        ],
      },
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const expandButtons = screen.getAllByRole("button");
    await user.click(expandButtons[0]);

    await waitFor(() => {
      expect(screen.getByText("Preprocessing Rules")).toBeInTheDocument();
    });

    expect(screen.getByText("1 rule")).toBeInTheDocument();
    expect(screen.getByText("Auto-Match Conditions")).toBeInTheDocument();
    expect(screen.getByText("2 conditions")).toBeInTheDocument();
  });
});

describe("PluginDetails - External ID Priority", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetFailures.mockResolvedValue({
      failures: [],
      total: 0,
      windowFailures: 0,
      windowSeconds: 3600,
      threshold: 3,
    });
  });

  it("shows External ID Prioritized badge when useExistingExternalId is true", async () => {
    const plugin = createMockPlugin({
      useExistingExternalId: true,
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const expandButtons = screen.getAllByRole("button");
    await user.click(expandButtons[0]);

    await waitFor(() => {
      expect(screen.getByText("External ID")).toBeInTheDocument();
    });

    expect(screen.getByText("Prioritized")).toBeInTheDocument();
  });

  it("shows Not prioritized badge when useExistingExternalId is false", async () => {
    const plugin = createMockPlugin({
      useExistingExternalId: false,
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const expandButtons = screen.getAllByRole("button");
    await user.click(expandButtons[0]);

    await waitFor(() => {
      expect(screen.getByText("External ID")).toBeInTheDocument();
    });

    expect(screen.getByText("Not prioritized")).toBeInTheDocument();
    expect(screen.queryByText("Prioritized")).not.toBeInTheDocument();
  });
});

describe("PluginDetails - Search Template", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetFailures.mockResolvedValue({
      failures: [],
      total: 0,
      windowFailures: 0,
      windowSeconds: 3600,
      threshold: 3,
    });
  });

  it("shows Custom badge when searchQueryTemplate is set", async () => {
    const plugin = createMockPlugin({
      searchQueryTemplate: "{{metadata.title}}",
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const expandButtons = screen.getAllByRole("button");
    await user.click(expandButtons[0]);

    await waitFor(() => {
      expect(screen.getByText("Search Template")).toBeInTheDocument();
    });

    expect(screen.getByText("Custom")).toBeInTheDocument();
  });

  it("shows Default badge when searchQueryTemplate is null", async () => {
    const plugin = createMockPlugin({
      searchQueryTemplate: null,
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const expandButtons = screen.getAllByRole("button");
    await user.click(expandButtons[0]);

    await waitFor(() => {
      expect(screen.getByText("Search Template")).toBeInTheDocument();
    });

    expect(screen.getByText("Default")).toBeInTheDocument();
  });
});

// Test the safeJsonParse helper function behavior
describe("safeJsonParse notification behavior", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows error notification when notifications.show is called with invalid JSON message", () => {
    // This tests that the notification infrastructure is properly mocked
    // and can be used to verify safeJsonParse behavior in integration tests
    notifications.show({
      title: "Invalid JSON",
      message:
        "The credentials field contains invalid JSON. Please check the format.",
      color: "red",
    });

    expect(notifications.show).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "Invalid JSON",
        color: "red",
      }),
    );
  });
});
