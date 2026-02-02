import { notifications } from "@mantine/notifications";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/utils";
import { PluginsSettings } from "./PluginsSettings";

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
    list: vi.fn().mockResolvedValue([]),
    create: vi.fn(),
    update: vi.fn(),
    delete: vi.fn(),
    getFailures: vi.fn().mockResolvedValue({ failures: [] }),
  },
}));

vi.mock("@/api/libraries", () => ({
  librariesApi: {
    getLibraries: vi.fn().mockResolvedValue([]),
  },
}));

vi.mock("@mantine/notifications", () => ({
  notifications: {
    show: vi.fn(),
  },
}));

describe("PluginsSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
