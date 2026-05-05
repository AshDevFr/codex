import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  type PaginatedReleases,
  type ReleaseLedgerEntry,
  releaseSourcesApi,
  releasesApi,
} from "@/api/releases";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { SeriesReleasesPanel } from "./SeriesReleasesPanel";

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

const SERIES_ID = "00000000-0000-0000-0000-000000000001";

function entry(over: Partial<ReleaseLedgerEntry> = {}): ReleaseLedgerEntry {
  return {
    id: "ent-1",
    seriesId: SERIES_ID,
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

describe("SeriesReleasesPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Avoid an unused-import warning while keeping the api mocked.
    void releaseSourcesApi;
  });

  it("renders an empty-state message when no releases exist", async () => {
    list.mockResolvedValueOnce(paginated([]));
    renderWithProviders(<SeriesReleasesPanel seriesId={SERIES_ID} />);
    await waitFor(() => {
      expect(screen.getByText(/no releases yet/i)).toBeInTheDocument();
    });
  });

  it("groups entries by chapter/volume and renders source rows", async () => {
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
    await waitFor(() => {
      expect(screen.getByText("Group-A")).toBeInTheDocument();
    });
    expect(screen.getByText("Group-B")).toBeInTheDocument();
    expect(screen.getByText("Group-C")).toBeInTheDocument();
    // Ch 143 is shared by Group-A and Group-B but only renders the cell label
    // on the first row of the group (others get an empty cell).
    expect(screen.getAllByText(/Ch 143/)).toHaveLength(1);
    expect(screen.getAllByText(/Ch 142/)).toHaveLength(1);
  });

  it("dismisses an announced entry via the dismiss action", async () => {
    list.mockResolvedValue(
      paginated([entry({ id: "a", groupOrUploader: "OnlyGroup" })]),
    );
    dismiss.mockResolvedValueOnce(entry({ id: "a", state: "dismissed" }));
    const user = userEvent.setup();
    renderWithProviders(<SeriesReleasesPanel seriesId={SERIES_ID} />);
    await screen.findByText("OnlyGroup");
    const dismissButton = screen.getByRole("button", { name: /dismiss/i });
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
    await screen.findByText("OnlyGroup");
    const acquireButton = screen.getByRole("button", {
      name: /mark acquired/i,
    });
    await user.click(acquireButton);
    await waitFor(() => {
      expect(markAcquired).toHaveBeenCalledWith("a");
    });
  });
});
