import { beforeEach, describe, expect, it, vi } from "vitest";
import { trackingApi } from "@/api/tracking";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { TrackingPanel } from "./TrackingPanel";

vi.mock("@/api/tracking", () => ({
  trackingApi: {
    getTracking: vi.fn(),
    updateTracking: vi.fn(),
    listAliases: vi.fn(),
    createAlias: vi.fn(),
    deleteAlias: vi.fn(),
  },
}));

const get = vi.mocked(trackingApi.getTracking);
const update = vi.mocked(trackingApi.updateTracking);
const list = vi.mocked(trackingApi.listAliases);
const create = vi.mocked(trackingApi.createAlias);
const del = vi.mocked(trackingApi.deleteAlias);

const SERIES_ID = "00000000-0000-0000-0000-000000000001";

const baseTracking = {
  seriesId: SERIES_ID,
  tracked: false,
  trackChapters: true,
  trackVolumes: true,
  createdAt: "2024-01-01T00:00:00Z",
  updatedAt: "2024-01-01T00:00:00Z",
};

const baseAlias = (
  alias: string,
  source: "manual" | "metadata" = "manual",
) => ({
  id: `alias-${alias}`,
  seriesId: SERIES_ID,
  alias,
  normalized: alias.toLowerCase(),
  source,
  createdAt: "2024-01-01T00:00:00Z",
});

describe("TrackingPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    list.mockResolvedValue([]);
  });

  it("renders the toggle in untracked state", async () => {
    get.mockResolvedValue({ ...baseTracking });

    renderWithProviders(<TrackingPanel seriesId={SERIES_ID} canEdit={true} />);

    await waitFor(() => {
      expect(
        screen.getByRole("switch", { name: /Toggle release tracking/i }),
      ).not.toBeChecked();
    });

    // Announce switches are hidden when not tracked.
    expect(screen.queryByText("Announce")).not.toBeInTheDocument();
  });

  it("shows announce flags when tracked", async () => {
    get.mockResolvedValue({
      ...baseTracking,
      tracked: true,
      latestKnownChapter: 142.5,
    });

    renderWithProviders(<TrackingPanel seriesId={SERIES_ID} canEdit={true} />);

    await waitFor(() => {
      expect(screen.getByText("Announce")).toBeInTheDocument();
    });
    expect(screen.getByLabelText("Chapters")).toBeChecked();
    expect(screen.getByLabelText("Volumes")).toBeChecked();
  });

  it("toggles tracked via mutation", async () => {
    const user = userEvent.setup();
    get.mockResolvedValue({ ...baseTracking });
    update.mockResolvedValue({ ...baseTracking, tracked: true });

    renderWithProviders(<TrackingPanel seriesId={SERIES_ID} canEdit={true} />);

    const toggle = await screen.findByRole("switch", {
      name: /Toggle release tracking/i,
    });
    await user.click(toggle);

    await waitFor(() => {
      expect(update).toHaveBeenCalledWith(SERIES_ID, { tracked: true });
    });
  });

  it("renders aliases and supports add (after expanding the collapsed panel)", async () => {
    const user = userEvent.setup();
    get.mockResolvedValue({ ...baseTracking, tracked: true });
    list.mockResolvedValue([baseAlias("Existing")]);
    create.mockImplementation(async (_id, req) => baseAlias(req.alias));

    renderWithProviders(<TrackingPanel seriesId={SERIES_ID} canEdit={true} />);

    // The panel is collapsed by default — expand to reach the alias UI.
    await user.click(
      await screen.findByRole("button", { name: /Expand release tracking/i }),
    );

    await screen.findByText("Existing");

    const input = screen.getByPlaceholderText(/Add an alias/i);
    await user.type(input, "New Alias");
    await user.click(screen.getByRole("button", { name: /^Add$/i }));

    await waitFor(() => {
      expect(create).toHaveBeenCalledWith(SERIES_ID, { alias: "New Alias" });
    });
  });

  it("hides edit affordances when canEdit=false", async () => {
    get.mockResolvedValue({ ...baseTracking, tracked: true });
    list.mockResolvedValue([baseAlias("Read Only")]);

    renderWithProviders(<TrackingPanel seriesId={SERIES_ID} canEdit={false} />);

    await screen.findByText("Read Only");

    expect(
      screen.queryByPlaceholderText(/Add an alias/i),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /^Add$/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("switch", { name: /Toggle release tracking/i }),
    ).toBeDisabled();
  });

  it("calls deleteAlias when remove is clicked", async () => {
    const user = userEvent.setup();
    get.mockResolvedValue({ ...baseTracking, tracked: true });
    const alias = baseAlias("Delete Me");
    list.mockResolvedValue([alias]);
    del.mockResolvedValue(undefined);

    renderWithProviders(<TrackingPanel seriesId={SERIES_ID} canEdit={true} />);

    // Expand to reveal the alias list.
    await user.click(
      await screen.findByRole("button", { name: /Expand release tracking/i }),
    );

    // findByRole waits past Mantine's Collapse animation into the
    // accessibility tree; getByRole here would race against it.
    const removeButton = await screen.findByRole("button", {
      name: /Remove alias Delete Me/i,
    });
    await user.click(removeButton);

    await waitFor(() => {
      expect(del).toHaveBeenCalledWith(SERIES_ID, alias.id);
    });
  });
});
