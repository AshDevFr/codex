import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  type BulkReleaseActionResponse,
  type PaginatedReleases,
  type ReleaseLedgerEntry,
  type ReleaseSource,
  releaseSourcesApi,
  releasesApi,
} from "@/api/releases";
import {
  renderWithProviders,
  screen,
  userEvent,
  waitFor,
  within,
} from "@/test/utils";
import { SeriesReleasesPanel } from "./SeriesReleasesPanel";

vi.mock("@/api/releases", () => ({
  releasesApi: {
    listInbox: vi.fn(),
    listForSeries: vi.fn(),
    patchEntry: vi.fn(),
    dismiss: vi.fn(),
    markAcquired: vi.fn(),
    delete: vi.fn(),
    bulk: vi.fn(),
  },
  releaseSourcesApi: {
    list: vi.fn(),
    update: vi.fn(),
    pollNow: vi.fn(),
  },
}));

const SERIES_ID = "00000000-0000-0000-0000-000000000001";

function entry(over: Partial<ReleaseLedgerEntry> = {}): ReleaseLedgerEntry {
  return {
    id: "ent-1",
    seriesId: SERIES_ID,
    seriesTitle: "Series",
    sourceId: "11111111-1111-1111-1111-111111111111",
    externalReleaseId: "ext-1",
    payloadUrl: "https://example.com/r/1",
    confidence: 0.95,
    state: "announced",
    observedAt: "2026-05-01T00:00:00Z",
    createdAt: "2026-05-01T00:00:00Z",
    chapter: 143,
    volume: null,
    language: "en",
    groupOrUploader: "GroupA",
    ...over,
  };
}

function paginated(entries: ReleaseLedgerEntry[]): PaginatedReleases {
  return {
    data: entries,
    page: 1,
    pageSize: 100,
    total: entries.length,
    totalPages: 1,
    links: {
      self: "/api/v1/series/x/releases",
    },
  } as PaginatedReleases;
}

const list = vi.mocked(releasesApi.listForSeries);
const dismiss = vi.mocked(releasesApi.dismiss);
const markAcquired = vi.mocked(releasesApi.markAcquired);
const deleteRelease = vi.mocked(releasesApi.delete);
const bulk = vi.mocked(releasesApi.bulk);

/** The panel collapses by default; tests expand it once before asserting. */
async function expandPanel() {
  const user = userEvent.setup();
  const toggle = await screen.findByRole("button", {
    name: /expand releases/i,
  });
  await user.click(toggle);
}

describe("SeriesReleasesPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(releaseSourcesApi.list).mockResolvedValue([]);
  });

  it("collapses by default and only renders the body after the user expands", async () => {
    list.mockResolvedValue(
      paginated([entry({ id: "a", groupOrUploader: "Group-A" })]),
    );
    renderWithProviders(<SeriesReleasesPanel seriesId={SERIES_ID} />);
    // Header carries the expand affordance; while collapsed, body content sits
    // in an aria-hidden subtree (Mantine's Collapse) so the toggle's a11y name
    // is "Expand releases" and row buttons are hidden from the a11y tree.
    await screen.findByRole("button", { name: /expand releases/i });
    expect(
      screen.queryByRole("button", { name: /dismiss/i }),
    ).not.toBeInTheDocument();
    await expandPanel();
    await screen.findByRole("button", { name: /collapse releases/i });
    await screen.findByRole("button", { name: /dismiss/i, hidden: true });
  });

  it("renders an empty-state message when no releases exist", async () => {
    list.mockResolvedValueOnce(paginated([]));
    renderWithProviders(<SeriesReleasesPanel seriesId={SERIES_ID} />);
    await expandPanel();
    await waitFor(() => {
      expect(screen.getByText(/no releases yet/i)).toBeInTheDocument();
    });
  });

  it("renders one row per ledger entry with the chapter label repeated", async () => {
    list.mockResolvedValueOnce(
      paginated([
        entry({ id: "a", chapter: 143, groupOrUploader: "Group-A" }),
        entry({
          id: "b",
          chapter: 143,
          groupOrUploader: "Group-B",
          externalReleaseId: "ext-2",
        }),
        entry({
          id: "c",
          chapter: 142,
          groupOrUploader: "Group-C",
          externalReleaseId: "ext-3",
        }),
      ]),
    );
    renderWithProviders(<SeriesReleasesPanel seriesId={SERIES_ID} />);
    await expandPanel();
    await waitFor(() => {
      expect(screen.getByText("Group-A")).toBeInTheDocument();
    });
    expect(screen.getByText("Group-B")).toBeInTheDocument();
    expect(screen.getByText("Group-C")).toBeInTheDocument();
    // Flat rows: each row carries its own chapter label. Two rows for Ch 143
    // (Group-A and Group-B), one row for Ch 142.
    expect(screen.getAllByText(/Ch 143/)).toHaveLength(2);
    expect(screen.getAllByText(/Ch 142/)).toHaveLength(1);
  });

  it("renders the source display name from the sources list", async () => {
    list.mockResolvedValueOnce(
      paginated([
        entry({
          id: "a",
          sourceId: "11111111-1111-1111-1111-111111111111",
          groupOrUploader: "tsuna69",
        }),
      ]),
    );
    vi.mocked(releaseSourcesApi.list).mockResolvedValue([
      {
        id: "11111111-1111-1111-1111-111111111111",
        pluginId: "release-nyaa",
        sourceKey: "nyaa:user:tsuna69",
        displayName: "Nyaa - tsuna69",
        kind: "rss_uploader",
        cronSchedule: null,
        effectiveCronSchedule: "0 * * * *",
        enabled: true,
        config: null,
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-01T00:00:00Z",
      } as ReleaseSource,
    ]);
    renderWithProviders(<SeriesReleasesPanel seriesId={SERIES_ID} />);
    await expandPanel();
    await waitFor(() => {
      expect(screen.getByText("Nyaa - tsuna69")).toBeInTheDocument();
    });
    // The UUID-prefix fallback should not appear once the join resolves.
    expect(screen.queryByText(/source: 11111111…/)).not.toBeInTheDocument();
  });

  it("dismisses an announced entry via the dismiss action", async () => {
    list.mockResolvedValue(
      paginated([entry({ id: "a", groupOrUploader: "OnlyGroup" })]),
    );
    dismiss.mockResolvedValueOnce(entry({ id: "a", state: "dismissed" }));
    const user = userEvent.setup();
    renderWithProviders(<SeriesReleasesPanel seriesId={SERIES_ID} />);
    await expandPanel();
    await screen.findByText("OnlyGroup");
    const dismissButton = await screen.findByRole("button", {
      name: /dismiss/i,
      hidden: true,
    });
    await user.click(dismissButton);
    await waitFor(() => {
      expect(dismiss).toHaveBeenCalledWith("a");
    });
  });

  it("marks an announced entry acquired via the action", async () => {
    list.mockResolvedValue(
      paginated([entry({ id: "a", groupOrUploader: "OnlyGroup" })]),
    );
    markAcquired.mockResolvedValueOnce(
      entry({ id: "a", state: "marked_acquired" }),
    );
    const user = userEvent.setup();
    renderWithProviders(<SeriesReleasesPanel seriesId={SERIES_ID} />);
    await expandPanel();
    await screen.findByText("OnlyGroup");
    const acquireButton = await screen.findByRole("button", {
      name: /mark acquired/i,
      hidden: true,
    });
    await user.click(acquireButton);
    await waitFor(() => {
      expect(markAcquired).toHaveBeenCalledWith("a");
    });
  });

  it("hard-deletes a row via the delete action", async () => {
    list.mockResolvedValue(
      paginated([entry({ id: "a", groupOrUploader: "OnlyGroup" })]),
    );
    deleteRelease.mockResolvedValueOnce({
      affectedReleaseIds: ["a"],
      affectedSeriesIds: [SERIES_ID],
      affectedSourceIds: [],
    } as BulkReleaseActionResponse);
    const user = userEvent.setup();
    renderWithProviders(<SeriesReleasesPanel seriesId={SERIES_ID} />);
    await expandPanel();
    await screen.findByText("OnlyGroup");
    const deleteButton = await screen.findByRole("button", {
      name: /delete/i,
      hidden: true,
    });
    await user.click(deleteButton);
    await waitFor(() => {
      expect(deleteRelease).toHaveBeenCalledWith("a");
    });
  });

  it("bulk-marks selected entries as acquired", async () => {
    list.mockResolvedValue(
      paginated([
        entry({ id: "a", chapter: 200, groupOrUploader: "Group-A" }),
        entry({ id: "b", chapter: 199, groupOrUploader: "Group-B" }),
        entry({ id: "c", chapter: 198, groupOrUploader: "Group-C" }),
      ]),
    );
    bulk.mockResolvedValueOnce({
      affectedReleaseIds: ["a", "b"],
      affectedSeriesIds: [SERIES_ID],
      affectedSourceIds: [],
    } as BulkReleaseActionResponse);
    const user = userEvent.setup();
    renderWithProviders(<SeriesReleasesPanel seriesId={SERIES_ID} />);
    await expandPanel();
    await screen.findByText("Group-A");
    // Select rows a and b individually.
    await user.click(
      await screen.findByRole("checkbox", {
        name: "Select release a",
        hidden: true,
      }),
    );
    await user.click(
      await screen.findByRole("checkbox", {
        name: "Select release b",
        hidden: true,
      }),
    );
    // Action bar appears with the count. Find the bulk action by walking up
    // from the "2 selected" label — the per-row "Mark acquired" buttons share
    // the same accessible name, so role-by-name returns multiple.
    const banner = screen
      .getByText("2 selected")
      .closest("div.mantine-Card-root");
    if (!banner) throw new Error("bulk banner not found");
    const bulkButton = within(banner as HTMLElement).getByRole("button", {
      name: /mark acquired/i,
    });
    await user.click(bulkButton);
    await waitFor(() => {
      expect(bulk).toHaveBeenCalledWith({
        ids: ["a", "b"],
        action: "mark-acquired",
      });
    });
  });

  it("bulk-deletes via the Delete button after the modal confirm", async () => {
    list.mockResolvedValue(
      paginated([
        entry({ id: "a", chapter: 200, groupOrUploader: "Group-A" }),
        entry({ id: "b", chapter: 199, groupOrUploader: "Group-B" }),
      ]),
    );
    bulk.mockResolvedValueOnce({
      affectedReleaseIds: ["a", "b"],
      affectedSeriesIds: [SERIES_ID],
      affectedSourceIds: [],
    } as BulkReleaseActionResponse);
    const user = userEvent.setup();
    renderWithProviders(<SeriesReleasesPanel seriesId={SERIES_ID} />);
    await expandPanel();
    await screen.findByText("Group-A");
    await user.click(
      await screen.findByRole("checkbox", {
        name: "Select release a",
        hidden: true,
      }),
    );
    await user.click(
      await screen.findByRole("checkbox", {
        name: "Select release b",
        hidden: true,
      }),
    );
    // Open the bulk-delete modal from the action bar (scoped to the banner
    // because per-row Delete buttons share the accessible name).
    const banner = screen
      .getByText("2 selected")
      .closest("div.mantine-Card-root");
    if (!banner) throw new Error("bulk banner not found");
    await user.click(
      within(banner as HTMLElement).getByRole("button", { name: /^delete$/i }),
    );
    // Confirm in the modal — its button label includes the count.
    const confirmButton = await screen.findByRole("button", {
      name: /delete 2 releases/i,
    });
    await user.click(confirmButton);
    await waitFor(() => {
      expect(bulk).toHaveBeenCalledWith({
        ids: ["a", "b"],
        action: "delete",
      });
    });
  });
});
