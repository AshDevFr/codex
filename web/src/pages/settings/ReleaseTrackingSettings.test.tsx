import { beforeEach, describe, expect, it, vi } from "vitest";
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

const list = vi.mocked(releaseSourcesApi.list);
const update = vi.mocked(releaseSourcesApi.update);
const pollNow = vi.mocked(releaseSourcesApi.pollNow);

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
});
