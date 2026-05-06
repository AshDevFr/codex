import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  type PaginatedReleases,
  type ReleaseFacets,
  type ReleaseLedgerEntry,
  type ReleaseSource,
  releaseSourcesApi,
  releasesApi,
} from "@/api/releases";
import { useReleaseAnnouncementsStore } from "@/store/releaseAnnouncementsStore";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { ReleasesInbox } from "./ReleasesInbox";

vi.mock("@/api/releases", () => ({
  releasesApi: {
    listInbox: vi.fn(),
    listForSeries: vi.fn(),
    patchEntry: vi.fn(),
    dismiss: vi.fn(),
    markAcquired: vi.fn(),
    delete: vi.fn(),
    bulk: vi.fn(),
    facets: vi.fn(),
  },
  releaseSourcesApi: {
    list: vi.fn(),
    update: vi.fn(),
    pollNow: vi.fn(),
  },
}));

function entry(over: Partial<ReleaseLedgerEntry> = {}): ReleaseLedgerEntry {
  return {
    id: "ent-1",
    seriesId: "00000000-0000-0000-0000-000000000001",
    seriesTitle: "Solo Leveling",
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
    groupOrUploader: "GroupZ",
    ...over,
  };
}

function paginated(entries: ReleaseLedgerEntry[]): PaginatedReleases {
  return {
    data: entries,
    page: 1,
    pageSize: 50,
    total: entries.length,
    totalPages: 1,
    links: {
      self: "/api/v1/releases",
    },
  } as PaginatedReleases;
}

function emptyFacets(): ReleaseFacets {
  return { languages: [], libraries: [], series: [] };
}

function source(over: Partial<ReleaseSource> = {}): ReleaseSource {
  return {
    id: "11111111-1111-1111-1111-111111111111",
    displayName: "MangaUpdates Releases",
    sourceKey: "default",
    pluginId: "release-mangaupdates",
    kind: "metadata-feed",
    enabled: true,
    cronSchedule: null,
    effectiveCronSchedule: "0 0 * * *",
    createdAt: "2026-05-01T00:00:00Z",
    updatedAt: "2026-05-01T00:00:00Z",
    ...over,
  } as ReleaseSource;
}

const list = vi.mocked(releasesApi.listInbox);
const facets = vi.mocked(releasesApi.facets);
const bulk = vi.mocked(releasesApi.bulk);
const remove = vi.mocked(releasesApi.delete);
const sourcesList = vi.mocked(releaseSourcesApi.list);

describe("ReleasesInbox", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useReleaseAnnouncementsStore.getState().reset();
    useReleaseAnnouncementsStore.getState().bump();
    facets.mockResolvedValue(emptyFacets());
    sourcesList.mockResolvedValue([source()]);
  });

  it("renders releases and resets the unseen badge on mount", async () => {
    list.mockResolvedValueOnce(paginated([entry()]));
    expect(useReleaseAnnouncementsStore.getState().unseenCount).toBe(1);
    renderWithProviders(<ReleasesInbox />);
    await waitFor(() => {
      expect(screen.getByText("GroupZ")).toBeInTheDocument();
    });
    // Series column should show the human title, not a sliced UUID.
    expect(screen.getByText("Solo Leveling")).toBeInTheDocument();
    expect(useReleaseAnnouncementsStore.getState().unseenCount).toBe(0);
  });

  it("falls back to a truncated UUID when the series title is empty", async () => {
    list.mockResolvedValueOnce(paginated([entry({ seriesTitle: "" })]));
    renderWithProviders(<ReleasesInbox />);
    await waitFor(() => {
      expect(screen.getByText(/^00000000…$/)).toBeInTheDocument();
    });
  });

  it("renders the source's display name instead of a UUID", async () => {
    list.mockResolvedValueOnce(paginated([entry()]));
    renderWithProviders(<ReleasesInbox />);
    expect(
      await screen.findByText("MangaUpdates Releases"),
    ).toBeInTheDocument();
    // The bare UUID slice should no longer appear in the row.
    expect(screen.queryByText(/^source: 11111111…$/)).not.toBeInTheDocument();
  });

  it("falls back to a truncated source UUID when the source is unknown", async () => {
    sourcesList.mockResolvedValue([]);
    list.mockResolvedValueOnce(paginated([entry()]));
    renderWithProviders(<ReleasesInbox />);
    expect(await screen.findByText(/^11111111…$/)).toBeInTheDocument();
  });

  it("shows empty-state copy when no entries match", async () => {
    list.mockResolvedValueOnce(paginated([]));
    renderWithProviders(<ReleasesInbox />);
    await waitFor(() => {
      expect(screen.getByText(/No releases match/i)).toBeInTheDocument();
    });
  });

  it("renders a kind-specific media-url icon when mediaUrl is set", async () => {
    list.mockResolvedValueOnce(
      paginated([
        entry({
          mediaUrl: "https://nyaa.si/download/1.torrent",
          mediaUrlKind: "torrent",
        }),
      ]),
    );
    renderWithProviders(<ReleasesInbox />);
    const payloadLink = await screen.findByLabelText("Open payload URL");
    expect(payloadLink).toHaveAttribute("href", "https://example.com/r/1");
    const torrentLink = screen.getByLabelText("Download .torrent");
    expect(torrentLink).toHaveAttribute(
      "href",
      "https://nyaa.si/download/1.torrent",
    );
  });

  it("does not render a media-url icon when mediaUrl is absent", async () => {
    list.mockResolvedValueOnce(paginated([entry()]));
    renderWithProviders(<ReleasesInbox />);
    await screen.findByLabelText("Open payload URL");
    expect(screen.queryByLabelText("Download .torrent")).toBeNull();
    expect(screen.queryByLabelText("Open magnet link")).toBeNull();
    expect(screen.queryByLabelText("Direct download")).toBeNull();
  });

  it("loads facets with the active filter context", async () => {
    list.mockResolvedValue(paginated([]));
    facets.mockResolvedValue({
      languages: [{ language: "en", count: 7 }],
      libraries: [
        { libraryId: "lib-a", libraryName: "Manga", count: 5 },
        { libraryId: "lib-b", libraryName: "Books", count: 2 },
      ],
      series: [
        {
          seriesId: "s-1",
          seriesTitle: "Solo Leveling",
          libraryId: "lib-a",
          libraryName: "Manga",
          count: 5,
        },
      ],
    });
    renderWithProviders(<ReleasesInbox />);
    await waitFor(() => {
      expect(facets).toHaveBeenCalledWith(
        expect.objectContaining({ state: "announced" }),
      );
    });
  });

  it("supports state=all by passing 'all' to the inbox query", async () => {
    list.mockResolvedValue(paginated([]));
    const user = userEvent.setup();
    renderWithProviders(<ReleasesInbox />);
    await waitFor(() => {
      expect(list).toHaveBeenCalledWith(
        expect.objectContaining({ state: "announced" }),
      );
    });
    const stateInput = screen.getByTestId(
      "releases-state-filter",
    ) as HTMLInputElement;
    await user.click(stateInput);
    const allOption = await screen.findByText("All", {
      selector: "[role=option] *, [role=option]",
    });
    await user.click(allOption);
    await waitFor(() => {
      expect(list).toHaveBeenCalledWith(
        expect.objectContaining({ state: "all" }),
      );
    });
  });

  it("bulk-dismisses the selected rows", async () => {
    list.mockResolvedValue(paginated([entry({ id: "a" }), entry({ id: "b" })]));
    bulk.mockResolvedValue({ affected: 2, action: "dismiss" });
    const user = userEvent.setup();
    renderWithProviders(<ReleasesInbox />);
    await screen.findAllByText("GroupZ");

    await user.click(screen.getByLabelText("Select release a"));
    await user.click(screen.getByLabelText("Select release b"));
    // The bulk action bar's Dismiss button has the IconX icon; the
    // per-row dismiss button has aria-label "Dismiss" but no visible
    // text. The bar's button has visible "Dismiss" text inside it.
    const dismissButtons = await screen.findAllByRole("button", {
      name: /Dismiss/,
    });
    // Bar button is the only "button" tagged with the visible word.
    const barButton = dismissButtons.find((b) =>
      b.textContent?.includes("Dismiss"),
    );
    await user.click(barButton!);
    await waitFor(() => {
      expect(bulk).toHaveBeenCalledWith({
        ids: ["a", "b"],
        action: "dismiss",
      });
    });
  });

  it("requires confirmation before bulk-deleting", async () => {
    list.mockResolvedValue(paginated([entry({ id: "a" })]));
    bulk.mockResolvedValue({ affected: 1, action: "delete" });
    const user = userEvent.setup();
    renderWithProviders(<ReleasesInbox />);
    await screen.findAllByText("GroupZ");

    await user.click(screen.getByLabelText("Select release a"));
    // The bulk-bar Delete button has visible "Delete" text; the per-row
    // delete has aria-label only.
    const deleteButtons = await screen.findAllByRole("button", {
      name: /Delete/,
    });
    const barButton = deleteButtons.find((b) =>
      b.textContent?.includes("Delete"),
    );
    await user.click(barButton!);
    // Confirmation modal opens — bulk hasn't fired yet.
    expect(bulk).not.toHaveBeenCalled();
    await user.click(
      await screen.findByRole("button", { name: /Delete 1 release/ }),
    );
    await waitFor(() => {
      expect(bulk).toHaveBeenCalledWith({ ids: ["a"], action: "delete" });
    });
  });

  it("per-row delete fires the delete API", async () => {
    list.mockResolvedValue(paginated([entry({ id: "a" })]));
    remove.mockResolvedValue({ deleted: true });
    const user = userEvent.setup();
    renderWithProviders(<ReleasesInbox />);
    await screen.findByText("GroupZ");

    await user.click(screen.getByLabelText("Delete"));
    await waitFor(() => {
      expect(remove).toHaveBeenCalledWith("a");
    });
  });
});
