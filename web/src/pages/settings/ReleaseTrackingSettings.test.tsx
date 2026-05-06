import { beforeEach, describe, expect, it, vi } from "vitest";
import { pluginsApi } from "@/api/plugins";
import {
  type ReleaseSource,
  releaseSourcesApi,
  releasesApi,
} from "@/api/releases";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { ReleaseTrackingSettings } from "./ReleaseTrackingSettings";

vi.mock("@/api/releases", () => ({
  releasesApi: {
    listInbox: vi.fn(),
    listForSeries: vi.fn(),
    patchEntry: vi.fn(),
    dismiss: vi.fn(),
    markAcquired: vi.fn(),
  },
  releaseSourcesApi: {
    list: vi.fn(),
    update: vi.fn(),
    pollNow: vi.fn(),
  },
}));

vi.mock("@/api/plugins", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/api/plugins")>();
  return {
    ...actual,
    pluginsApi: {
      ...actual.pluginsApi,
      getAll: vi.fn(),
    },
  };
});

const list = vi.mocked(releaseSourcesApi.list);
const update = vi.mocked(releaseSourcesApi.update);
const pollNow = vi.mocked(releaseSourcesApi.pollNow);
const getAllPlugins = vi.mocked(pluginsApi.getAll);

function source(over: Partial<ReleaseSource> = {}): ReleaseSource {
  return {
    id: "11111111-1111-1111-1111-111111111111",
    pluginId: "release-mangaupdates",
    sourceKey: "mu:batch",
    displayName: "MangaUpdates batch",
    kind: "rss-series",
    enabled: true,
    pollIntervalS: 21600,
    lastPolledAt: "2026-05-01T00:00:00Z",
    lastError: null,
    lastErrorAt: null,
    etag: null,
    config: null,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-05-01T00:00:00Z",
    ...over,
  };
}

describe("ReleaseTrackingSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    void releasesApi;
    // Default: no plugins installed. Individual tests override as needed.
    getAllPlugins.mockResolvedValue({ plugins: [], total: 0 });
  });

  it("renders sources and the OK status when last poll is fresh", async () => {
    list.mockResolvedValueOnce([source()]);
    renderWithProviders(<ReleaseTrackingSettings />);
    await waitFor(() => {
      expect(screen.getByText("MangaUpdates batch")).toBeInTheDocument();
    });
    expect(screen.getByText("OK")).toBeInTheDocument();
  });

  it("shows an Errored badge when last_error is populated", async () => {
    list.mockResolvedValueOnce([
      source({ lastError: "upstream returned 503" }),
    ]);
    renderWithProviders(<ReleaseTrackingSettings />);
    await waitFor(() => {
      expect(screen.getByText("Errored")).toBeInTheDocument();
    });
  });

  it("toggling enabled calls update with the new value", async () => {
    list.mockResolvedValue([source()]);
    update.mockResolvedValueOnce(source({ enabled: false }));
    const user = userEvent.setup();
    renderWithProviders(<ReleaseTrackingSettings />);
    await waitFor(() => {
      expect(screen.getByText("MangaUpdates batch")).toBeInTheDocument();
    });
    const toggle = screen.getByRole("switch", { name: "Enable source" });
    await user.click(toggle);
    await waitFor(() => {
      expect(update).toHaveBeenCalledWith(
        "11111111-1111-1111-1111-111111111111",
        expect.objectContaining({ enabled: false }),
      );
    });
  });

  it("Poll now button is disabled when source is disabled", async () => {
    list.mockResolvedValueOnce([source({ enabled: false })]);
    renderWithProviders(<ReleaseTrackingSettings />);
    await waitFor(() => {
      expect(screen.getByText("MangaUpdates batch")).toBeInTheDocument();
    });
    const pollButton = screen.getByLabelText("Poll now");
    expect(pollButton).toBeDisabled();
  });

  it("clicking Poll now triggers the API call when source is enabled", async () => {
    list.mockResolvedValue([source()]);
    pollNow.mockResolvedValueOnce({ status: "enqueued", message: "ok" });
    const user = userEvent.setup();
    renderWithProviders(<ReleaseTrackingSettings />);
    await waitFor(() => {
      expect(screen.getByText("MangaUpdates batch")).toBeInTheDocument();
    });
    const pollButton = screen.getByLabelText("Poll now");
    await user.click(pollButton);
    await waitFor(() => {
      expect(pollNow).toHaveBeenCalledWith(
        "11111111-1111-1111-1111-111111111111",
      );
    });
  });

  it("Poll now spinner is per-row, not shared across rows", async () => {
    // Two sources: the first poll is held in flight while we click the
    // second. Only the first row should show a loading spinner.
    list.mockResolvedValue([
      source({
        id: "11111111-1111-1111-1111-111111111111",
        displayName: "Source A",
      }),
      source({
        id: "22222222-2222-2222-2222-222222222222",
        displayName: "Source B",
        sourceKey: "mu:other",
      }),
    ]);

    let resolveFirst:
      | ((v: { status: string; message: string }) => void)
      | null = null;
    pollNow.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
    );

    const user = userEvent.setup();
    renderWithProviders(<ReleaseTrackingSettings />);
    await waitFor(() => {
      expect(screen.getByText("Source A")).toBeInTheDocument();
      expect(screen.getByText("Source B")).toBeInTheDocument();
    });

    const pollButtons = screen.getAllByLabelText("Poll now");
    expect(pollButtons).toHaveLength(2);

    await user.click(pollButtons[0]);

    await waitFor(() => {
      expect(pollButtons[0]).toHaveAttribute("data-loading", "true");
    });
    // Crucially, the other row's button must NOT be in a loading state while
    // row A's poll is in flight.
    expect(pollButtons[1]).not.toHaveAttribute("data-loading", "true");
    expect(pollButtons[1]).not.toBeDisabled();

    // Resolve the first request and verify the spinner clears.
    resolveFirst?.({ status: "enqueued", message: "ok" });
    await waitFor(() => {
      expect(pollButtons[0]).not.toHaveAttribute("data-loading", "true");
    });
  });

  it("plugin-sources dropdown lists release-source plugins by display name", async () => {
    list.mockResolvedValue([]);
    // One release-source plugin + one metadata plugin to confirm filtering.
    getAllPlugins.mockResolvedValue({
      plugins: [
        {
          id: "p1",
          name: "release-mangaupdates",
          displayName: "MangaUpdates Releases",
          manifest: {
            name: "release-mangaupdates",
            displayName: "MangaUpdates Releases",
            capabilities: { releaseSource: true },
          },
          // The remaining PluginDto fields don't matter for this test.
        } as never,
        {
          id: "p2",
          name: "metadata-mangabaka",
          displayName: "MangaBaka",
          manifest: {
            name: "metadata-mangabaka",
            displayName: "MangaBaka",
            capabilities: { metadataProvider: ["series"] },
          },
        } as never,
      ],
      total: 2,
    });

    const user = userEvent.setup();
    renderWithProviders(<ReleaseTrackingSettings />);
    // Wait for the plugins query to settle (the dropdown only renders the
    // release-source options once `pluginsApi.getAll` resolves).
    await waitFor(() => {
      expect(getAllPlugins).toHaveBeenCalled();
    });
    // Mantine MultiSelect renders an input with role=textbox associated with
    // the label; clicking it opens the dropdown and shows the options.
    const select = screen.getByRole("textbox", { name: "Plugin sources" });
    await user.click(select);
    await waitFor(() => {
      expect(screen.getByText("MangaUpdates Releases")).toBeInTheDocument();
    });
    // Metadata-only plugin is filtered out — should not appear as an option.
    expect(screen.queryByText("MangaBaka")).not.toBeInTheDocument();
  });
});
