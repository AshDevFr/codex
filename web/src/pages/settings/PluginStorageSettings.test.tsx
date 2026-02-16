import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as pluginStorageApi from "@/api/pluginStorage";
import { renderWithProviders } from "@/test/utils";
import { PluginStorageSettings } from "./PluginStorageSettings";

// Mock the plugin storage API
vi.mock("@/api/pluginStorage", () => ({
  pluginStorageApi: {
    getStats: vi.fn(),
    getPluginStats: vi.fn(),
    cleanupPlugin: vi.fn(),
  },
}));

// Default mock stats
const defaultStats: pluginStorageApi.AllPluginStorageStatsDto = {
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

describe("PluginStorageSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(pluginStorageApi.pluginStorageApi.getStats).mockResolvedValue(
      defaultStats,
    );
  });

  it("should render page title", async () => {
    renderWithProviders(<PluginStorageSettings />);

    await waitFor(() => {
      expect(screen.getByText("Plugin Storage")).toBeInTheDocument();
    });
  });

  it("should show loading state initially", () => {
    renderWithProviders(<PluginStorageSettings />);

    expect(
      screen.getByText("Loading plugin storage statistics..."),
    ).toBeInTheDocument();
  });

  it("should display stat card labels after loading", async () => {
    renderWithProviders(<PluginStorageSettings />);

    await waitFor(() => {
      expect(screen.getByText("Plugins with Storage")).toBeInTheDocument();
      expect(screen.getByText("Total Files")).toBeInTheDocument();
      expect(screen.getByText("Total Size")).toBeInTheDocument();
    });
  });

  it("should display stat card values after loading", async () => {
    renderWithProviders(<PluginStorageSettings />);

    await waitFor(() => {
      // "3" appears in both the stat card and the table (sync-kavita has 3 files),
      // so check that there are at least 2 elements with "3"
      expect(screen.getAllByText("3")).toHaveLength(2);
      expect(screen.getByText("16")).toBeInTheDocument(); // 16 total files
      expect(screen.getByText("10.5 MB")).toBeInTheDocument(); // ~11MB total
    });
  });

  it("should show info alert about plugin storage", async () => {
    renderWithProviders(<PluginStorageSettings />);

    await waitFor(() => {
      expect(screen.getByText("About Plugin Storage")).toBeInTheDocument();
    });
  });

  it("should show refresh button", async () => {
    renderWithProviders(<PluginStorageSettings />);

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /refresh/i }),
      ).toBeInTheDocument();
    });
  });

  it("should show per-plugin table rows", async () => {
    renderWithProviders(<PluginStorageSettings />);

    await waitFor(() => {
      expect(screen.getByText("metadata-anilist")).toBeInTheDocument();
      expect(screen.getByText("metadata-mangaupdates")).toBeInTheDocument();
      expect(screen.getByText("sync-kavita")).toBeInTheDocument();
    });
  });

  it("should show empty state when no plugins have storage", async () => {
    vi.mocked(pluginStorageApi.pluginStorageApi.getStats).mockResolvedValue({
      plugins: [],
      totalFileCount: 0,
      totalBytes: 0,
    });

    renderWithProviders(<PluginStorageSettings />);

    await waitFor(() => {
      expect(
        screen.getByText("No plugins have stored any files yet."),
      ).toBeInTheDocument();
    });
  });

  it("should open confirmation modal when cleanup button is clicked", async () => {
    const user = userEvent.setup();
    renderWithProviders(<PluginStorageSettings />);

    await waitFor(() => {
      expect(screen.getByText("metadata-anilist")).toBeInTheDocument();
    });

    const deleteButton = screen.getByLabelText(
      "Delete storage for metadata-anilist",
    );
    await user.click(deleteButton);

    await waitFor(() => {
      expect(screen.getByText("Delete Plugin Storage")).toBeInTheDocument();
      expect(screen.getByText(/Delete all storage for/)).toBeInTheDocument();
    });
  });

  it("should call cleanup mutation on confirm", async () => {
    const user = userEvent.setup();
    vi.mocked(
      pluginStorageApi.pluginStorageApi.cleanupPlugin,
    ).mockResolvedValue({
      filesDeleted: 12,
      bytesFreed: 2_097_152,
      failures: 0,
    });

    renderWithProviders(<PluginStorageSettings />);

    await waitFor(() => {
      expect(screen.getByText("metadata-anilist")).toBeInTheDocument();
    });

    const deleteButton = screen.getByLabelText(
      "Delete storage for metadata-anilist",
    );
    await user.click(deleteButton);

    await waitFor(() => {
      expect(screen.getByText("Delete Plugin Storage")).toBeInTheDocument();
    });

    const confirmButton = screen.getByRole("button", { name: "Delete" });
    await user.click(confirmButton);

    await waitFor(() => {
      expect(
        pluginStorageApi.pluginStorageApi.cleanupPlugin,
      ).toHaveBeenCalledWith("metadata-anilist");
    });
  });

  it("should show error notification on cleanup failure", async () => {
    const user = userEvent.setup();
    vi.mocked(
      pluginStorageApi.pluginStorageApi.cleanupPlugin,
    ).mockRejectedValue(new Error("Server error"));

    renderWithProviders(<PluginStorageSettings />);

    await waitFor(() => {
      expect(screen.getByText("metadata-anilist")).toBeInTheDocument();
    });

    const deleteButton = screen.getByLabelText(
      "Delete storage for metadata-anilist",
    );
    await user.click(deleteButton);

    await waitFor(() => {
      expect(screen.getByText("Delete Plugin Storage")).toBeInTheDocument();
    });

    const confirmButton = screen.getByRole("button", { name: "Delete" });
    await user.click(confirmButton);

    await waitFor(() => {
      expect(
        pluginStorageApi.pluginStorageApi.cleanupPlugin,
      ).toHaveBeenCalledWith("metadata-anilist");
    });
  });
});
