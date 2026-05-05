import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  type PaginatedReleases,
  type ReleaseLedgerEntry,
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

const list = vi.mocked(releasesApi.listInbox);

describe("ReleasesInbox", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useReleaseAnnouncementsStore.getState().reset();
    useReleaseAnnouncementsStore.getState().bump();
    void releaseSourcesApi;
  });

  it("renders releases and resets the unseen badge on mount", async () => {
    list.mockResolvedValueOnce(paginated([entry()]));
    expect(useReleaseAnnouncementsStore.getState().unseenCount).toBe(1);
    renderWithProviders(<ReleasesInbox />);
    await waitFor(() => {
      expect(screen.getByText("GroupZ")).toBeInTheDocument();
    });
    expect(useReleaseAnnouncementsStore.getState().unseenCount).toBe(0);
  });

  it("shows empty-state copy when no entries match", async () => {
    list.mockResolvedValueOnce(paginated([]));
    renderWithProviders(<ReleasesInbox />);
    await waitFor(() => {
      expect(screen.getByText(/No releases match/i)).toBeInTheDocument();
    });
  });

  it("typing a series filter triggers a new query", async () => {
    list.mockResolvedValue(paginated([]));
    const user = userEvent.setup();
    renderWithProviders(<ReleasesInbox />);
    await waitFor(() => {
      expect(list).toHaveBeenCalledWith(
        expect.objectContaining({ state: "announced" }),
      );
    });

    const seriesInput = screen.getByPlaceholderText(/Optional UUID/i);
    await user.type(seriesInput, "abc-123");

    await waitFor(() => {
      expect(list).toHaveBeenCalledWith(
        expect.objectContaining({ seriesId: "abc-123" }),
      );
    });
  });
});
