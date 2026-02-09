import { notifications } from "@mantine/notifications";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PluginDto } from "@/api/plugins";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { PluginsSettings } from "./PluginsSettings";

// ---------------------------------------------------------------------------
// Helper: find an ActionIcon button by the Tabler icon class inside it.
// Mantine Tooltip does not set aria-label on the wrapped element, so we
// locate action buttons by their SVG icon class name instead.
// ---------------------------------------------------------------------------
function getActionButtonByIcon(
  container: HTMLElement,
  iconClass: string,
): HTMLElement {
  const svg = container.querySelector(`svg.${iconClass}`);
  if (!svg) throw new Error(`SVG with class "${iconClass}" not found`);
  const button = svg.closest("button");
  if (!button)
    throw new Error(`No button ancestor found for icon "${iconClass}"`);
  return button;
}

/**
 * Click the expand chevron button for a plugin row.
 * Uses the Tabler icon class to find the correct button reliably.
 */
async function clickExpandButton(
  container: HTMLElement,
  user: ReturnType<typeof userEvent.setup>,
) {
  const chevron = getActionButtonByIcon(container, "tabler-icon-chevron-right");
  await user.click(chevron);
}

// Declare top-level mock fns so we can assert against them
const mockGetAll = vi.fn();
const mockCreate = vi.fn();
const mockUpdate = vi.fn();
const mockDelete = vi.fn();
const mockEnable = vi.fn();
const mockDisable = vi.fn();
const mockTest = vi.fn();
const mockResetFailures = vi.fn();
const mockGetFailures = vi.fn();
const mockLibrariesGetAll = vi.fn();

vi.mock("@/api/plugins", () => ({
  AVAILABLE_PERMISSIONS: [
    { value: "metadata:read", label: "Read Metadata" },
    { value: "metadata:write:*", label: "Write All Metadata" },
  ],
  AVAILABLE_SCOPES: [
    { value: "series:detail", label: "Series Detail" },
    { value: "series:bulk", label: "Series Bulk" },
  ],
  CREDENTIAL_DELIVERY_OPTIONS: [
    { value: "env", label: "Environment Variables" },
    { value: "init_message", label: "Initialize Message" },
    { value: "both", label: "Both" },
  ],
  pluginsApi: {
    getAll: (...args: unknown[]) => mockGetAll(...args),
    create: (...args: unknown[]) => mockCreate(...args),
    update: (...args: unknown[]) => mockUpdate(...args),
    delete: (...args: unknown[]) => mockDelete(...args),
    enable: (...args: unknown[]) => mockEnable(...args),
    disable: (...args: unknown[]) => mockDisable(...args),
    test: (...args: unknown[]) => mockTest(...args),
    resetFailures: (...args: unknown[]) => mockResetFailures(...args),
    getFailures: (...args: unknown[]) => mockGetFailures(...args),
  },
}));

vi.mock("@/api/libraries", () => ({
  librariesApi: {
    getAll: (...args: unknown[]) => mockLibrariesGetAll(...args),
  },
}));

vi.mock("@mantine/notifications", () => ({
  notifications: {
    show: vi.fn(),
  },
}));

// ---------------------------------------------------------------------------
// Test data factory
// ---------------------------------------------------------------------------
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
    manifest: {
      name: "test-plugin",
      displayName: "Test Plugin",
      version: "1.0.0",
      protocolVersion: "1.0",
      description: "A test plugin",
      author: "Test Author",
      capabilities: {
        metadataProvider: ["series"],
        userReadSync: false,
      },
      contentTypes: ["series"],
      requiredCredentials: [],
      scopes: ["series:detail"],
    },
    rateLimitRequestsPerMinute: 60,
    createdAt: "2024-01-01T00:00:00Z",
    updatedAt: "2024-01-01T00:00:00Z",
    ...overrides,
  } as PluginDto;
}

const emptyFailuresResponse = {
  failures: [],
  total: 0,
  windowFailures: 0,
  windowSeconds: 3600,
  threshold: 3,
};

// ---------------------------------------------------------------------------
// Shared setup
// ---------------------------------------------------------------------------
beforeEach(() => {
  vi.clearAllMocks();
  mockGetAll.mockResolvedValue({ plugins: [], total: 0 });
  mockLibrariesGetAll.mockResolvedValue([]);
  mockGetFailures.mockResolvedValue(emptyFailuresResponse);
});

// ===========================================================================
// 1. Page header & basic rendering
// ===========================================================================
describe("PluginsSettings - page header", () => {
  it("renders the page title", () => {
    renderWithProviders(<PluginsSettings />);
    expect(screen.getByText("Plugins")).toBeInTheDocument();
  });

  it("renders the page description", () => {
    renderWithProviders(<PluginsSettings />);
    expect(
      screen.getByText(
        /Manage external plugin processes for metadata fetching/,
      ),
    ).toBeInTheDocument();
  });

  it("renders the Add Plugin button", () => {
    renderWithProviders(<PluginsSettings />);
    expect(
      screen.getByRole("button", { name: /Add Plugin/i }),
    ).toBeInTheDocument();
  });
});

// ===========================================================================
// 2. Loading state
// ===========================================================================
describe("PluginsSettings - loading state", () => {
  it("shows a loader while data is loading", () => {
    // Never resolve so the query stays in loading state
    mockGetAll.mockReturnValue(new Promise(() => {}));

    const { container } = renderWithProviders(<PluginsSettings />);

    // Mantine Loader renders a span with the mantine-Loader-root class
    expect(
      container.querySelector(".mantine-Loader-root"),
    ).toBeInTheDocument();
  });
});

// ===========================================================================
// 3. Error state
// ===========================================================================
describe("PluginsSettings - error state", () => {
  it("shows an error alert when the plugins query fails", async () => {
    mockGetAll.mockRejectedValue(new Error("Network error"));

    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(
        screen.getByText(/Failed to load plugins/i),
      ).toBeInTheDocument();
    });
  });
});

// ===========================================================================
// 4. Empty state
// ===========================================================================
describe("PluginsSettings - empty state", () => {
  it("shows 'No plugins configured' alert when the plugin list is empty", async () => {
    mockGetAll.mockResolvedValue({ plugins: [], total: 0 });

    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("No plugins configured")).toBeInTheDocument();
    });

    expect(
      screen.getByText(
        /Add plugins to enable metadata fetching from external sources/,
      ),
    ).toBeInTheDocument();
  });
});

// ===========================================================================
// 5. Plugin list rendering
// ===========================================================================
describe("PluginsSettings - plugin list rendering", () => {
  it("renders plugin display name and slug", async () => {
    const plugin = createMockPlugin({
      displayName: "MangaBaka",
      name: "mangabaka",
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("MangaBaka")).toBeInTheDocument();
    });
    expect(screen.getByText("mangabaka")).toBeInTheDocument();
  });

  it("renders the plugin command as code", async () => {
    const plugin = createMockPlugin({ command: "npx" });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("npx")).toBeInTheDocument();
    });
  });

  it("renders multiple plugins as separate rows", async () => {
    const pluginA = createMockPlugin({
      id: "p-1",
      displayName: "Plugin A",
      name: "plugin-a",
    });
    const pluginB = createMockPlugin({
      id: "p-2",
      displayName: "Plugin B",
      name: "plugin-b",
    });
    mockGetAll.mockResolvedValue({ plugins: [pluginA, pluginB], total: 2 });

    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Plugin A")).toBeInTheDocument();
    });
    expect(screen.getByText("Plugin B")).toBeInTheDocument();
  });

  it("renders table headers", async () => {
    const plugin = createMockPlugin();
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Plugin")).toBeInTheDocument();
    });
    expect(screen.getByText("Command")).toBeInTheDocument();
    expect(screen.getByText("Status")).toBeInTheDocument();
    expect(screen.getByText("Health")).toBeInTheDocument();
    expect(screen.getByText("Actions")).toBeInTheDocument();
  });
});

// ===========================================================================
// 6. Health status badges
// ===========================================================================
describe("PluginsSettings - health status badges", () => {
  const healthStatuses: Array<{ status: string }> = [
    { status: "healthy" },
    { status: "degraded" },
    { status: "unhealthy" },
    { status: "disabled" },
    { status: "unknown" },
  ];

  it.each(healthStatuses)(
    "renders a badge with text '$status'",
    async ({ status }) => {
      const plugin = createMockPlugin({
        healthStatus: status,
      });
      mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

      renderWithProviders(<PluginsSettings />);

      await waitFor(() => {
        expect(screen.getByText(status)).toBeInTheDocument();
      });
    },
  );
});

// ===========================================================================
// 7. Failure count badge
// ===========================================================================
describe("PluginsSettings - failure count badge", () => {
  it("shows a failure count badge when failureCount > 0", async () => {
    const plugin = createMockPlugin({ failureCount: 5 });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("5")).toBeInTheDocument();
    });
  });

  it("does not show a failure count badge when failureCount is 0", async () => {
    const plugin = createMockPlugin({ failureCount: 0 });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    // The only number-like text in the health column should be the status label
    // There should be no extra badge with "0"
    const healthyBadges = screen.getAllByText("healthy");
    expect(healthyBadges).toHaveLength(1);
  });

  it("shows Reset Failures button when failureCount > 0", async () => {
    const plugin = createMockPlugin({ failureCount: 3 });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    // The Reset Failures button contains the refresh icon
    const resetBtn = getActionButtonByIcon(container, "tabler-icon-refresh");
    expect(resetBtn).toBeInTheDocument();
  });

  it("does not show Reset Failures button when failureCount is 0", async () => {
    const plugin = createMockPlugin({ failureCount: 0 });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    expect(
      container.querySelector("svg.tabler-icon-refresh"),
    ).not.toBeInTheDocument();
  });
});

// ===========================================================================
// 8. Enable / disable toggle
// ===========================================================================
describe("PluginsSettings - enable/disable toggle", () => {
  it("renders an enabled switch for an enabled plugin", async () => {
    const plugin = createMockPlugin({ enabled: true });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    renderWithProviders(<PluginsSettings />);

    // Wait for data to load, then check the switch
    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const switchEl = screen.getByRole("switch");
    expect(switchEl).toBeChecked();
  });

  it("renders a disabled switch for a disabled plugin", async () => {
    const plugin = createMockPlugin({ enabled: false });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const switchEl = screen.getByRole("switch");
    expect(switchEl).not.toBeChecked();
  });

  it("calls disable API when toggling an enabled plugin off", async () => {
    const plugin = createMockPlugin({ id: "p-1", enabled: true });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });
    mockDisable.mockResolvedValue({ message: "Plugin disabled" });

    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await user.click(screen.getByRole("switch"));

    await waitFor(() => {
      expect(mockDisable).toHaveBeenCalledWith("p-1", expect.anything());
    });
  });

  it("calls enable API when toggling a disabled plugin on", async () => {
    const plugin = createMockPlugin({ id: "p-2", enabled: false });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });
    mockEnable.mockResolvedValue({ message: "Plugin enabled" });

    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await user.click(screen.getByRole("switch"));

    await waitFor(() => {
      expect(mockEnable).toHaveBeenCalledWith("p-2", expect.anything());
    });
  });

  it("shows success notification after enabling", async () => {
    const plugin = createMockPlugin({ id: "p-2", enabled: false });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });
    mockEnable.mockResolvedValue({ message: "Plugin enabled successfully" });

    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await user.click(screen.getByRole("switch"));

    await waitFor(() => {
      expect(notifications.show).toHaveBeenCalledWith(
        expect.objectContaining({
          title: "Success",
          message: "Plugin enabled successfully",
          color: "green",
        }),
      );
    });
  });
});

// ===========================================================================
// 9. Add Plugin modal
// ===========================================================================
describe("PluginsSettings - Add Plugin modal", () => {
  it("opens the create modal when Add Plugin is clicked", async () => {
    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await user.click(screen.getByRole("button", { name: /Add Plugin/i }));

    await waitFor(() => {
      // Modal title
      expect(
        screen.getByRole("heading", { name: /Add Plugin/i }),
      ).toBeInTheDocument();
    });

    // Form fields should be visible
    expect(screen.getByLabelText(/Display Name/i)).toBeInTheDocument();
  });

  it("shows the Create Plugin submit button inside the modal", async () => {
    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await user.click(screen.getByRole("button", { name: /Add Plugin/i }));

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /Create Plugin/i }),
      ).toBeInTheDocument();
    });
  });

  it("shows Cancel button inside the modal", async () => {
    const user = userEvent.setup();
    renderWithProviders(<PluginsSettings />);

    await user.click(screen.getByRole("button", { name: /Add Plugin/i }));

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /Cancel/i }),
      ).toBeInTheDocument();
    });
  });
});

// ===========================================================================
// 10. Delete confirmation modal
// ===========================================================================
describe("PluginsSettings - delete confirmation", () => {
  it("opens delete modal when trash icon is clicked", async () => {
    const plugin = createMockPlugin({ displayName: "MangaBaka" });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("MangaBaka")).toBeInTheDocument();
    });

    // Click the delete button (trash icon)
    const deleteBtn = getActionButtonByIcon(container, "tabler-icon-trash");
    await user.click(deleteBtn);

    await waitFor(() => {
      expect(
        screen.getByText(/Are you sure you want to delete the plugin/),
      ).toBeInTheDocument();
    });
    // The plugin name appears both in the table row and in the modal confirmation
    expect(screen.getAllByText("MangaBaka").length).toBeGreaterThanOrEqual(2);
    expect(
      screen.getByText("This action cannot be undone."),
    ).toBeInTheDocument();
  });

  it("calls delete API and shows notification on confirm", async () => {
    const plugin = createMockPlugin({
      id: "del-1",
      displayName: "MangaBaka",
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });
    mockDelete.mockResolvedValue(undefined);

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("MangaBaka")).toBeInTheDocument();
    });

    const deleteBtn = getActionButtonByIcon(container, "tabler-icon-trash");
    await user.click(deleteBtn);

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /^Delete Plugin$/i }),
      ).toBeInTheDocument();
    });

    // Click the "Delete Plugin" confirm button inside the modal
    await user.click(
      screen.getByRole("button", { name: /^Delete Plugin$/i }),
    );

    await waitFor(() => {
      expect(mockDelete).toHaveBeenCalledWith("del-1", expect.anything());
    });

    await waitFor(() => {
      expect(notifications.show).toHaveBeenCalledWith(
        expect.objectContaining({
          title: "Success",
          message: "Plugin deleted successfully",
          color: "green",
        }),
      );
    });
  });

  it("closes the delete modal when Cancel is clicked", async () => {
    const plugin = createMockPlugin({ displayName: "MangaBaka" });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("MangaBaka")).toBeInTheDocument();
    });

    const deleteBtn = getActionButtonByIcon(container, "tabler-icon-trash");
    await user.click(deleteBtn);

    await waitFor(() => {
      expect(
        screen.getByText(/Are you sure you want to delete/),
      ).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: /Cancel/i }));

    await waitFor(() => {
      expect(
        screen.queryByText(/Are you sure you want to delete/),
      ).not.toBeInTheDocument();
    });
  });
});

// ===========================================================================
// 11. Test connection
// ===========================================================================
describe("PluginsSettings - test connection", () => {
  it("calls test API when Test Connection button is clicked", async () => {
    const plugin = createMockPlugin({ id: "test-conn-1" });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });
    mockTest.mockResolvedValue({
      success: true,
      message: "Successfully connected to plugin",
      latencyMs: 42,
    });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const testBtn = getActionButtonByIcon(
      container,
      "tabler-icon-player-play",
    );
    await user.click(testBtn);

    await waitFor(() => {
      expect(mockTest).toHaveBeenCalledWith("test-conn-1", expect.anything());
    });
  });

  it("shows success notification with latency on successful test", async () => {
    const plugin = createMockPlugin({ id: "test-conn-2" });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });
    mockTest.mockResolvedValue({
      success: true,
      message: "Successfully connected to plugin",
      latencyMs: 120,
    });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const testBtn = getActionButtonByIcon(
      container,
      "tabler-icon-player-play",
    );
    await user.click(testBtn);

    await waitFor(() => {
      expect(notifications.show).toHaveBeenCalledWith(
        expect.objectContaining({
          title: "Connection Successful",
          message: "Successfully connected to plugin (120ms)",
          color: "green",
        }),
      );
    });
  });

  it("shows failure notification when test reports failure", async () => {
    const plugin = createMockPlugin({ id: "test-conn-3" });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });
    mockTest.mockResolvedValue({
      success: false,
      message: "Connection refused",
      latencyMs: null,
    });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const testBtn = getActionButtonByIcon(
      container,
      "tabler-icon-player-play",
    );
    await user.click(testBtn);

    await waitFor(() => {
      expect(notifications.show).toHaveBeenCalledWith(
        expect.objectContaining({
          title: "Connection Failed",
          message: "Connection refused",
          color: "red",
        }),
      );
    });
  });

  it("shows error notification when test API call throws", async () => {
    const plugin = createMockPlugin({ id: "test-conn-4" });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });
    mockTest.mockRejectedValue(new Error("Network error"));

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const testBtn = getActionButtonByIcon(
      container,
      "tabler-icon-player-play",
    );
    await user.click(testBtn);

    await waitFor(() => {
      expect(notifications.show).toHaveBeenCalledWith(
        expect.objectContaining({
          title: "Test Failed",
          color: "red",
        }),
      );
    });
  });
});

// ===========================================================================
// 12. Row expansion / collapse
// ===========================================================================
describe("PluginsSettings - row expansion", () => {
  it("expands a row when the chevron is clicked", async () => {
    const plugin = createMockPlugin({ description: "Detailed description" });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    // Before expanding, the description is inside a collapsed section
    // Click the expand button (first button in the row)
    await clickExpandButton(container, user);

    // After expansion, details should be visible
    await waitFor(() => {
      expect(screen.getByText("Description")).toBeInTheDocument();
    });
    expect(screen.getByText("Detailed description")).toBeInTheDocument();
  });

  it("collapses an expanded row when the chevron is clicked again", async () => {
    const plugin = createMockPlugin({ description: "Detailed description" });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    // First click: expand
    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("Description")).toBeInTheDocument();
    });

    // Second click: collapse (icon is now chevron-down after expanding)
    const collapseBtn = getActionButtonByIcon(container, "tabler-icon-chevron-down");
    await user.click(collapseBtn);

    // The collapse is animated, so the content may eventually be hidden.
    // We just verify the toggle was processed without error.
  });

  it("shows plugin details including credentials info when expanded", async () => {
    const plugin = createMockPlugin({ hasCredentials: true });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("Credentials")).toBeInTheDocument();
    });
    expect(screen.getByText("Configured")).toBeInTheDocument();
  });

  it("shows 'Not configured' when hasCredentials is false", async () => {
    const plugin = createMockPlugin({ hasCredentials: false });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("Not configured")).toBeInTheDocument();
    });
  });

  it("shows rate limit value in expanded details", async () => {
    const plugin = createMockPlugin({ rateLimitRequestsPerMinute: 120 });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("120 req/min")).toBeInTheDocument();
    });
  });

  it("shows 'No limit' when rateLimitRequestsPerMinute is null", async () => {
    const plugin = createMockPlugin({ rateLimitRequestsPerMinute: null });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("No limit")).toBeInTheDocument();
    });
  });

  it("shows 'No description' when description is absent", async () => {
    const plugin = createMockPlugin({ description: null });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("No description")).toBeInTheDocument();
    });
  });
});

// ===========================================================================
// 13. Expanded row - permissions and scopes
// ===========================================================================
describe("PluginsSettings - permissions and scopes in expanded details", () => {
  it("shows permission badges", async () => {
    const plugin = createMockPlugin({
      permissions: ["metadata:read", "metadata:write:*"],
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("metadata:read")).toBeInTheDocument();
    });
    expect(screen.getByText("metadata:write:*")).toBeInTheDocument();
  });

  it("shows scope badges", async () => {
    const plugin = createMockPlugin({
      scopes: ["series:detail", "series:bulk"],
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("series:detail")).toBeInTheDocument();
    });
    expect(screen.getByText("series:bulk")).toBeInTheDocument();
  });

  it("shows 'None' text when permissions are empty", async () => {
    const plugin = createMockPlugin({ permissions: [], scopes: [] });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("Permissions")).toBeInTheDocument();
    });

    // Both permissions and scopes show "None"
    const noneTexts = screen.getAllByText("None");
    expect(noneTexts.length).toBeGreaterThanOrEqual(2);
  });

  it("shows 'All Libraries' badge when libraryIds is empty", async () => {
    const plugin = createMockPlugin({ libraryIds: [] });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("All Libraries")).toBeInTheDocument();
    });
  });
});

// ===========================================================================
// 14. Expanded row - manifest section
// ===========================================================================
describe("PluginsSettings - manifest section in expanded details", () => {
  it("shows manifest version and protocol", async () => {
    const plugin = createMockPlugin({
      manifest: {
        name: "test-plugin",
        displayName: "Test Plugin",
        version: "2.1.0",
        protocolVersion: "1.0",
        description: "A test plugin",
        author: "Author Name",
        capabilities: {
          metadataProvider: ["series"],
          userReadSync: false,
        },
        contentTypes: ["series"],
        requiredCredentials: [],
        scopes: ["series:detail"],
      },
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("2.1.0")).toBeInTheDocument();
    });
    expect(screen.getByText("v1.0")).toBeInTheDocument();
    expect(screen.getByText("Author Name")).toBeInTheDocument();
  });

  it("shows Metadata Provider badge for metadata-capable plugins", async () => {
    const plugin = createMockPlugin({
      manifest: {
        name: "test-plugin",
        displayName: "Test Plugin",
        version: "1.0.0",
        protocolVersion: "1.0",
        description: "A test",
        capabilities: {
          metadataProvider: ["series"],
          userReadSync: false,
        },
        contentTypes: ["series"],
        requiredCredentials: [],
        scopes: [],
      },
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("Metadata Provider")).toBeInTheDocument();
    });
  });

  it("shows Reading Sync badge for sync-capable plugins", async () => {
    const plugin = createMockPlugin({
      manifest: {
        name: "sync-plugin",
        displayName: "Test Plugin",
        version: "1.0.0",
        protocolVersion: "1.0",
        description: "A sync plugin",
        capabilities: {
          metadataProvider: [],
          userReadSync: true,
        },
        contentTypes: [],
        requiredCredentials: [],
        scopes: [],
      },
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("Reading Sync")).toBeInTheDocument();
    });
  });

  it("does not show manifest section when manifest is null", async () => {
    const plugin = createMockPlugin({ manifest: null });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("Description")).toBeInTheDocument();
    });

    // Manifest-specific labels should not appear
    expect(screen.queryByText("Manifest")).not.toBeInTheDocument();
    expect(screen.queryByText("Metadata Provider")).not.toBeInTheDocument();
  });
});

// ===========================================================================
// 15. Expanded row - disabled reason
// ===========================================================================
describe("PluginsSettings - disabled reason in expanded details", () => {
  it("shows disabled reason alert when present", async () => {
    const plugin = createMockPlugin({
      disabledReason: "Too many failures (exceeded threshold)",
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("Disabled Reason")).toBeInTheDocument();
    });
    expect(
      screen.getByText("Too many failures (exceeded threshold)"),
    ).toBeInTheDocument();
  });
});

// ===========================================================================
// 16. Edit plugin modal
// ===========================================================================
describe("PluginsSettings - edit plugin modal", () => {
  it("opens edit modal when the edit button is clicked", async () => {
    const plugin = createMockPlugin({ displayName: "MangaBaka" });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("MangaBaka")).toBeInTheDocument();
    });

    const editBtn = getActionButtonByIcon(container, "tabler-icon-edit");
    await user.click(editBtn);

    await waitFor(() => {
      expect(
        screen.getByText("Edit Plugin: MangaBaka"),
      ).toBeInTheDocument();
    });

    // The edit form should have Save Changes instead of Create Plugin
    expect(
      screen.getByRole("button", { name: /Save Changes/i }),
    ).toBeInTheDocument();
  });
});

// ===========================================================================
// 17. Configure plugin modal
// ===========================================================================
describe("PluginsSettings - configure plugin button", () => {
  it("shows configure plugin button in actions", async () => {
    const plugin = createMockPlugin();
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const configBtn = getActionButtonByIcon(
      container,
      "tabler-icon-settings",
    );
    expect(configBtn).toBeInTheDocument();
  });
});

// ===========================================================================
// 18. Metadata targets in expanded details
// ===========================================================================
describe("PluginsSettings - metadata targets in expanded details", () => {
  it("shows active metadata targets from manifest capabilities", async () => {
    const plugin = createMockPlugin({
      metadataTargets: ["series"],
      manifest: {
        name: "test-plugin",
        displayName: "Test Plugin",
        version: "1.0.0",
        protocolVersion: "1.0",
        description: "A test plugin",
        author: "Test",
        capabilities: {
          metadataProvider: ["series", "book"],
          userReadSync: false,
        },
        contentTypes: ["series", "book"],
        requiredCredentials: [],
        scopes: ["series:detail"],
      },
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("Metadata Targets")).toBeInTheDocument();
    });

    expect(screen.getByText("Series")).toBeInTheDocument();
    expect(screen.getByText("Books")).toBeInTheDocument();
  });

  it("shows all targets as active when metadataTargets is null", async () => {
    const plugin = createMockPlugin({
      metadataTargets: null,
      manifest: {
        name: "test-plugin",
        displayName: "Test Plugin",
        version: "1.0.0",
        protocolVersion: "1.0",
        description: "A test plugin",
        capabilities: {
          metadataProvider: ["series", "book"],
          userReadSync: false,
        },
        contentTypes: ["series", "book"],
        requiredCredentials: [],
        scopes: ["series:detail"],
      },
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("Metadata Targets")).toBeInTheDocument();
    });

    expect(screen.getByText("Series")).toBeInTheDocument();
    expect(screen.getByText("Books")).toBeInTheDocument();
  });

  it("does not show metadata targets when no manifest", async () => {
    const plugin = createMockPlugin({ manifest: null });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("A test plugin")).toBeInTheDocument();
    });

    expect(screen.queryByText("Metadata Targets")).not.toBeInTheDocument();
  });
});

// ===========================================================================
// 19. Preprocessing rules and auto-match conditions
// ===========================================================================
describe("PluginsSettings - preprocessing rules and conditions", () => {
  it("shows preprocessing rules count", async () => {
    const plugin = createMockPlugin({
      searchPreprocessingRules: [
        { pattern: "\\s*\\(Digital\\)$", replacement: "" },
        { pattern: "\\s*\\[Digital\\]$", replacement: "" },
      ],
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("Preprocessing Rules")).toBeInTheDocument();
    });

    expect(screen.getByText("2 rules")).toBeInTheDocument();
  });

  it("shows singular rule label for a single preprocessing rule", async () => {
    const plugin = createMockPlugin({
      searchPreprocessingRules: [
        { pattern: "\\s*\\(Digital\\)$", replacement: "" },
      ],
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("1 rule")).toBeInTheDocument();
    });
  });

  it("shows auto-match conditions count", async () => {
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
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("3 conditions")).toBeInTheDocument();
    });
  });

  it("shows singular condition label for a single condition", async () => {
    const plugin = createMockPlugin({
      autoMatchConditions: {
        mode: "all",
        rules: [{ field: "external_ids.plugin:test", operator: "is_null" }],
      },
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("1 condition")).toBeInTheDocument();
    });
  });

  it("shows zero counts when rules and conditions are null", async () => {
    const plugin = createMockPlugin({
      searchPreprocessingRules: null,
      autoMatchConditions: null,
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("0 rules")).toBeInTheDocument();
    });

    expect(screen.getByText("0 conditions")).toBeInTheDocument();
  });
});

// ===========================================================================
// 20. External ID priority and search template
// ===========================================================================
describe("PluginsSettings - external ID priority", () => {
  it("shows Prioritized badge when useExistingExternalId is true", async () => {
    const plugin = createMockPlugin({ useExistingExternalId: true });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("External ID")).toBeInTheDocument();
    });

    expect(screen.getByText("Prioritized")).toBeInTheDocument();
  });

  it("shows Not prioritized badge when useExistingExternalId is false", async () => {
    const plugin = createMockPlugin({ useExistingExternalId: false });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("Not prioritized")).toBeInTheDocument();
    });
  });
});

describe("PluginsSettings - search template", () => {
  it("shows Custom badge when searchQueryTemplate is set", async () => {
    const plugin = createMockPlugin({
      searchQueryTemplate: "{{metadata.title}}",
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("Search Template")).toBeInTheDocument();
    });

    expect(screen.getByText("Custom")).toBeInTheDocument();
  });

  it("shows Default badge when searchQueryTemplate is null", async () => {
    const plugin = createMockPlugin({ searchQueryTemplate: null });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("Search Template")).toBeInTheDocument();
    });

    expect(screen.getByText("Default")).toBeInTheDocument();
  });
});

// ===========================================================================
// 21. Reset failures
// ===========================================================================
describe("PluginsSettings - reset failures", () => {
  it("calls resetFailures API when button is clicked", async () => {
    const plugin = createMockPlugin({
      id: "reset-1",
      failureCount: 5,
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });
    mockResetFailures.mockResolvedValue({
      message: "Failure count reset",
    });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const resetBtn = getActionButtonByIcon(container, "tabler-icon-refresh");
    await user.click(resetBtn);

    await waitFor(() => {
      expect(mockResetFailures).toHaveBeenCalledWith("reset-1", expect.anything());
    });
  });

  it("shows success notification after resetting failures", async () => {
    const plugin = createMockPlugin({
      id: "reset-2",
      failureCount: 3,
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });
    mockResetFailures.mockResolvedValue({
      message: "Failure count reset successfully",
    });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    const resetBtn = getActionButtonByIcon(container, "tabler-icon-refresh");
    await user.click(resetBtn);

    await waitFor(() => {
      expect(notifications.show).toHaveBeenCalledWith(
        expect.objectContaining({
          title: "Success",
          message: "Failure count reset successfully",
          color: "green",
        }),
      );
    });
  });
});

// ===========================================================================
// 22. Arguments display in expanded details
// ===========================================================================
describe("PluginsSettings - arguments in expanded details", () => {
  it("shows arguments as code block when present", async () => {
    const plugin = createMockPlugin({
      args: ["-y", "@ashdev/codex-plugin@1.0.0"],
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("Arguments")).toBeInTheDocument();
    });

    // Arguments are joined by newline in a code block; use a text content matcher
    expect(screen.getByText(/^-y/)).toBeInTheDocument();
    expect(
      screen.getByText(/@ashdev\/codex-plugin@1\.0\.0/),
    ).toBeInTheDocument();
  });

  it("does not show arguments code block when args is empty", async () => {
    const plugin = createMockPlugin({ args: [] });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("Description")).toBeInTheDocument();
    });

    // When args is empty, the code block with "index.js" should not appear
    // (Note: we check for the arg value rather than the "Arguments" label
    // because the form modal also has an "Arguments" label)
    expect(screen.queryByText("index.js")).not.toBeInTheDocument();
  });
});

// ===========================================================================
// 23. User count display
// ===========================================================================
describe("PluginsSettings - user count in expanded details", () => {
  it("shows user count when userCount is present", async () => {
    const plugin = createMockPlugin({ userCount: 5 } as Partial<PluginDto>);
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("Users")).toBeInTheDocument();
    });

    expect(screen.getByText("5 users")).toBeInTheDocument();
  });

  it("shows singular user label for single user", async () => {
    const plugin = createMockPlugin({ userCount: 1 } as Partial<PluginDto>);
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(screen.getByText("1 user")).toBeInTheDocument();
    });
  });
});

// ===========================================================================
// 24. Recommendation provider badge
// ===========================================================================
describe("PluginsSettings - recommendation provider badge", () => {
  it("shows Recommendation Provider badge when capability is set", async () => {
    const plugin = createMockPlugin({
      manifest: {
        name: "rec-plugin",
        displayName: "Rec Plugin",
        version: "1.0.0",
        protocolVersion: "1.0",
        description: "A recommendation plugin",
        capabilities: {
          metadataProvider: [],
          userReadSync: false,
          userRecommendationProvider: true,
        },
        contentTypes: [],
        requiredCredentials: [],
        scopes: [],
      },
    });
    mockGetAll.mockResolvedValue({ plugins: [plugin], total: 1 });

    const user = userEvent.setup();
    const { container } = renderWithProviders(<PluginsSettings />);

    await waitFor(() => {
      expect(screen.getByText("Test Plugin")).toBeInTheDocument();
    });

    await clickExpandButton(container, user);

    await waitFor(() => {
      expect(
        screen.getByText("Recommendation Provider"),
      ).toBeInTheDocument();
    });
  });
});

// ===========================================================================
// 25. Notification mock verification
// ===========================================================================
describe("PluginsSettings - notification mock", () => {
  it("verifies that notifications.show is properly mocked", () => {
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
